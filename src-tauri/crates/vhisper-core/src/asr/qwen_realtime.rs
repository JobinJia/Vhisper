//! 通义千问实时流式 ASR 服务
//!
//! 基于 WebSocket 的实时语音识别，支持边说边识别

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

/// WebSocket 连接超时时间
const WS_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Session 确认超时时间
const SESSION_CONFIRM_TIMEOUT: Duration = Duration::from_secs(5);

use super::traits::{AsrError, StreamingAsrEvent, StreamingAsrService, StreamingControl};

fn generate_event_id() -> String {
    format!(
        "event_{}",
        Uuid::new_v4().to_string().replace("-", "")[..20].to_string()
    )
}

/// 通义千问实时流式 ASR 服务
pub struct QwenRealtimeAsr {
    api_key: String,
    model: String,
}

impl QwenRealtimeAsr {
    pub fn new(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }
}

// ============================================================================
// 请求事件结构
// ============================================================================

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
    audio: String,
}

#[derive(Serialize)]
struct AudioCommitEvent {
    event_id: String,
    #[serde(rename = "type")]
    event_type: String,
}

// ============================================================================
// 响应事件结构
// ============================================================================

#[derive(Deserialize, Debug)]
struct ResponseEvent {
    #[serde(rename = "type")]
    event_type: String,
    transcript: Option<String>,
    // 流式中间结果字段
    text: Option<String>,
    stash: Option<String>,
    error: Option<ErrorInfo>,
}

#[derive(Deserialize, Debug)]
struct ErrorInfo {
    message: String,
}

// ============================================================================
// 流式服务实现
// ============================================================================

#[async_trait]
impl StreamingAsrService for QwenRealtimeAsr {
    async fn start_streaming(
        &self,
        sample_rate: u32,
    ) -> Result<(mpsc::Sender<StreamingControl>, mpsc::Receiver<StreamingAsrEvent>), AsrError> {
        // 创建通道
        let (control_tx, mut control_rx) = mpsc::channel::<StreamingControl>(32);
        let (event_tx, event_rx) = mpsc::channel::<StreamingAsrEvent>(32);

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

        // 连接 WebSocket（带超时）
        let (ws_stream, _) = timeout(WS_CONNECT_TIMEOUT, connect_async(request))
            .await
            .map_err(|_| AsrError::Network("WebSocket 连接超时".to_string()))?
            .map_err(|e| AsrError::Network(format!("WebSocket 连接失败: {}", e)))?;

        let (mut write, mut read) = ws_stream.split();

        // 发送 session.update 配置（使用 VAD 模式实现实时识别）
        let session_update = SessionUpdateEvent {
            event_id: generate_event_id(),
            event_type: "session.update".to_string(),
            session: SessionConfig {
                modalities: vec!["text".to_string()],
                input_audio_format: "pcm".to_string(),
                sample_rate,
                input_audio_transcription: TranscriptionConfig {
                    language: "zh".to_string(),
                },
                // VAD 模式：服务端自动检测语音边界
                turn_detection: Some(TurnDetection {
                    detection_type: "server_vad".to_string(),
                    threshold: 0.5,
                    silence_duration_ms: 500,
                }),
            },
        };

        let session_json =
            serde_json::to_string(&session_update).map_err(|e| AsrError::Encoding(e.to_string()))?;

        write
            .send(Message::Text(session_json.into()))
            .await
            .map_err(|e| AsrError::Network(e.to_string()))?;

        // 等待 session 确认（带超时）
        let session_confirm_result = timeout(SESSION_CONFIRM_TIMEOUT, async {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(response) = serde_json::from_str::<ResponseEvent>(&text) {
                            if let Some(error) = response.error {
                                return Err(AsrError::Api(error.message));
                            }
                            if response.event_type == "session.created"
                                || response.event_type == "session.updated"
                            {
                                return Ok(());
                            }
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
            Err(AsrError::Api("未收到 session 确认事件".to_string()))
        })
        .await;

        match session_confirm_result {
            Ok(Ok(())) => {} // session 确认成功
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(AsrError::Network("等待 session 确认超时".to_string())),
        }

        // 启动后台任务处理双向通信
        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            let mut accumulated_text = String::new();

            loop {
                tokio::select! {
                    // 处理控制命令
                    Some(control) = control_rx.recv() => {
                        match control {
                            StreamingControl::Audio(data) => {
                                // 发送音频数据
                                let audio_append = AudioAppendEvent {
                                    event_id: generate_event_id(),
                                    event_type: "input_audio_buffer.append".to_string(),
                                    audio: BASE64.encode(&data),
                                };
                                if let Ok(json) = serde_json::to_string(&audio_append) {
                                    if write.send(Message::Text(json.into())).await.is_err() {
                                        let _ = event_tx_clone.send(StreamingAsrEvent::Error(
                                            "发送音频失败".to_string()
                                        )).await;
                                        break;
                                    }
                                }
                            }
                            StreamingControl::Commit => {
                                // 提交音频缓冲区
                                let commit = AudioCommitEvent {
                                    event_id: generate_event_id(),
                                    event_type: "input_audio_buffer.commit".to_string(),
                                };
                                if let Ok(json) = serde_json::to_string(&commit) {
                                    let _ = write.send(Message::Text(json.into())).await;
                                }
                            }
                            StreamingControl::Cancel => {
                                // 取消并关闭连接
                                let _ = write.close().await;
                                break;
                            }
                        }
                    }
                    // 处理服务端响应
                    Some(msg) = read.next() => {
                        match msg {
                            Ok(Message::Text(text)) => {
                                if let Ok(response) = serde_json::from_str::<ResponseEvent>(&text) {
                                    if let Some(error) = response.error {
                                        let _ = event_tx_clone.send(StreamingAsrEvent::Error(
                                            error.message
                                        )).await;
                                        break;
                                    }

                                    match response.event_type.as_str() {
                                        // 中间结果（流式）
                                        "conversation.item.input_audio_transcription.text" => {
                                            let text = response.text.unwrap_or_default();
                                            let stash = response.stash.unwrap_or_default();
                                            // 更新累积文本
                                            if !text.is_empty() {
                                                accumulated_text = text.clone();
                                            }
                                            let _ = event_tx_clone.send(StreamingAsrEvent::Partial {
                                                text,
                                                stash,
                                            }).await;
                                        }
                                        // 最终结果
                                        "conversation.item.input_audio_transcription.completed" => {
                                            let final_text = response.transcript
                                                .or(response.text)
                                                .unwrap_or(accumulated_text.clone());
                                            let _ = event_tx_clone.send(StreamingAsrEvent::Final {
                                                text: final_text,
                                            }).await;
                                            // 重置累积文本，准备下一轮
                                            accumulated_text.clear();
                                        }
                                        "error" => {
                                            if let Some(error) = response.error {
                                                let _ = event_tx_clone.send(StreamingAsrEvent::Error(
                                                    error.message
                                                )).await;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            Ok(Message::Close(_)) => {
                                break;
                            }
                            Err(e) => {
                                let _ = event_tx_clone.send(StreamingAsrEvent::Error(
                                    e.to_string()
                                )).await;
                                break;
                            }
                            _ => {}
                        }
                    }
                    else => break,
                }
            }
        });

        Ok((control_tx, event_rx))
    }
}
