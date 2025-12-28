use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

fn generate_event_id() -> String {
    format!("event_{}", Uuid::new_v4().to_string().replace("-", "")[..20].to_string())
}

use super::traits::{AsrError, AsrResult, AsrService};

/// 通义千问实时语音识别服务
pub struct QwenAsr {
    api_key: String,
    model: String,
}

impl QwenAsr {
    pub fn new(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }
}

// 请求事件结构
#[derive(Serialize)]
struct SessionUpdateEvent {
    event_id: String,
    #[serde(rename = "type")]
    event_type: String,
    session: SessionConfig,
}

#[derive(Serialize)]
struct SessionConfig {
    modalities: Vec<String>,
    input_audio_format: String,
    sample_rate: u32,
    input_audio_transcription: TranscriptionConfig,
    turn_detection: Option<TurnDetection>,
}

#[derive(Serialize)]
struct TranscriptionConfig {
    language: String,
}

#[derive(Serialize)]
struct TurnDetection {
    #[serde(rename = "type")]
    detection_type: String,
    threshold: f32,
    silence_duration_ms: u32,
}

#[derive(Serialize)]
struct AudioAppendEvent {
    event_id: String,
    #[serde(rename = "type")]
    event_type: String,
    audio: String, // base64 encoded
}

#[derive(Serialize)]
struct AudioCommitEvent {
    event_id: String,
    #[serde(rename = "type")]
    event_type: String,
}

// 响应事件结构
#[derive(Deserialize, Debug)]
struct ResponseEvent {
    #[serde(rename = "type")]
    event_type: String,
    transcript: Option<String>,
    error: Option<ErrorInfo>,
}

#[derive(Deserialize, Debug)]
struct ErrorInfo {
    message: String,
}

#[async_trait]
impl AsrService for QwenAsr {
    async fn recognize(&self, audio_data: &[u8], _sample_rate: u32) -> Result<AsrResult, AsrError> {
        // 构建 WebSocket URL
        let url = format!(
            "wss://dashscope.aliyuncs.com/api-ws/v1/realtime?model={}",
            self.model
        );

        // 创建带认证头的请求
        let request = http::Request::builder()
            .uri(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("OpenAI-Beta", "realtime=v1")
            .header(
                "Sec-WebSocket-Key",
                tokio_tungstenite::tungstenite::handshake::client::generate_key(),
            )
            .header("Sec-WebSocket-Version", "13")
            .header("Host", "dashscope.aliyuncs.com")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .body(())
            .map_err(|e| AsrError::Network(e.to_string()))?;

        // 连接 WebSocket
        let (ws_stream, _) = connect_async(request)
            .await
            .map_err(|e| AsrError::Network(format!("WebSocket 连接失败: {}", e)))?;

        let (mut write, mut read) = ws_stream.split();

        // 发送 session.update 配置
        let session_update = SessionUpdateEvent {
            event_id: generate_event_id(),
            event_type: "session.update".to_string(),
            session: SessionConfig {
                modalities: vec!["text".to_string()],
                input_audio_format: "pcm".to_string(),
                sample_rate: 16000,
                input_audio_transcription: TranscriptionConfig {
                    language: "zh".to_string(),
                },
                turn_detection: None, // 手动模式，通过 commit 触发
            },
        };

        let session_json =
            serde_json::to_string(&session_update).map_err(|e| AsrError::Encoding(e.to_string()))?;

        write
            .send(Message::Text(session_json.into()))
            .await
            .map_err(|e| AsrError::Network(e.to_string()))?;

        // 等待 session.created 或 session.updated 事件
        let mut session_ready = false;
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let response: ResponseEvent = serde_json::from_str(&text)
                        .map_err(|e| AsrError::Api(format!("解析响应失败: {}", e)))?;

                    if let Some(error) = response.error {
                        return Err(AsrError::Api(error.message));
                    }

                    if response.event_type == "session.created"
                        || response.event_type == "session.updated"
                    {
                        session_ready = true;
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    return Err(AsrError::Network("WebSocket 连接被关闭".to_string()));
                }
                Err(e) => {
                    return Err(AsrError::Network(e.to_string()));
                }
                _ => {}
            }
        }

        if !session_ready {
            return Err(AsrError::Api("未收到 session 确认事件".to_string()));
        }

        // 检查音频数据是否为空
        if audio_data.is_empty() {
            return Err(AsrError::Encoding("音频数据为空".to_string()));
        }

        tracing::debug!("发送音频数据: {} 字节", audio_data.len());

        // 分块发送音频数据（base64 编码）
        let chunk_size = 3200; // 约 100ms @ 16kHz 16bit
        for chunk in audio_data.chunks(chunk_size) {
            let audio_append = AudioAppendEvent {
                event_id: generate_event_id(),
                event_type: "input_audio_buffer.append".to_string(),
                audio: BASE64.encode(chunk),
            };

            let audio_json = serde_json::to_string(&audio_append)
                .map_err(|e| AsrError::Encoding(e.to_string()))?;

            write
                .send(Message::Text(audio_json.into()))
                .await
                .map_err(|e| AsrError::Network(e.to_string()))?;
        }

        // 发送 commit 信号表示音频结束
        let commit = AudioCommitEvent {
            event_id: generate_event_id(),
            event_type: "input_audio_buffer.commit".to_string(),
        };

        let commit_json =
            serde_json::to_string(&commit).map_err(|e| AsrError::Encoding(e.to_string()))?;

        write
            .send(Message::Text(commit_json.into()))
            .await
            .map_err(|e| AsrError::Network(e.to_string()))?;

        // 收集识别结果
        let mut final_text = String::new();

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let response: ResponseEvent = serde_json::from_str(&text)
                        .map_err(|e| AsrError::Api(format!("解析响应失败: {}", e)))?;

                    if let Some(error) = response.error {
                        return Err(AsrError::Api(error.message));
                    }

                    match response.event_type.as_str() {
                        "conversation.item.input_audio_transcription.completed" => {
                            if let Some(transcript) = response.transcript {
                                final_text = transcript;
                            }
                            break;
                        }
                        "conversation.item.input_audio_transcription.text" => {
                            // 中间结果，可以忽略或更新
                            if let Some(transcript) = response.transcript {
                                final_text = transcript;
                            }
                        }
                        "error" => {
                            if let Some(error) = response.error {
                                return Err(AsrError::Api(error.message));
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Message::Close(_)) => {
                    break;
                }
                Err(e) => {
                    return Err(AsrError::Network(e.to_string()));
                }
                _ => {}
            }
        }

        Ok(AsrResult {
            text: final_text,
            is_final: true,
        })
    }
}

/// 测试通义千问 ASR API 连接
pub async fn test_api(api_key: &str) -> Result<String, AsrError> {
    use reqwest::Client;

    let client = Client::new();
    let response = client
        .get("https://dashscope.aliyuncs.com/api/v1/models")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| AsrError::Network(e.to_string()))?;

    if response.status().is_success() {
        Ok("API Key 验证成功".to_string())
    } else {
        Err(AsrError::Api(format!(
            "API Key 无效: HTTP {}",
            response.status()
        )))
    }
}
