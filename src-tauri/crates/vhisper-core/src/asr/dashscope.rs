use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

use super::traits::{AsrError, AsrResult, AsrService};

/// DashScope ASR 服务 (WebSocket 实时语音识别)
pub struct DashScopeAsr {
    api_key: String,
    model: String,
}

impl DashScopeAsr {
    pub fn new(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }
}

// WebSocket 请求结构
#[derive(Serialize)]
struct WsRequest {
    header: WsHeader,
    payload: WsPayload,
}

#[derive(Serialize)]
struct WsHeader {
    action: String,
    task_id: String,
    streaming: String,
}

#[derive(Serialize)]
struct WsPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    task_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    task: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<WsParameters>,
    input: serde_json::Value,
}

#[derive(Serialize)]
struct WsParameters {
    format: String,
    sample_rate: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_hints: Option<Vec<String>>,
}

// WebSocket 响应结构
#[derive(Deserialize, Debug)]
struct WsResponse {
    header: WsResponseHeader,
    payload: Option<WsResponsePayload>,
}

#[derive(Deserialize, Debug)]
struct WsResponseHeader {
    #[allow(dead_code)]
    task_id: String,
    event: String,
    #[serde(default)]
    error_code: Option<String>,
    #[serde(default)]
    error_message: Option<String>,
}

#[derive(Deserialize, Debug)]
struct WsResponsePayload {
    output: Option<WsOutput>,
}

#[derive(Deserialize, Debug)]
struct WsOutput {
    sentence: Option<WsSentence>,
}

#[derive(Deserialize, Debug)]
struct WsSentence {
    text: Option<String>,
    #[serde(default)]
    sentence_end: bool,
}

#[async_trait]
impl AsrService for DashScopeAsr {
    async fn recognize(&self, audio_data: &[u8], sample_rate: u32) -> Result<AsrResult, AsrError> {
        let task_id = Uuid::new_v4().to_string().replace("-", "");

        // 构建 WebSocket URL 和请求
        let url = "wss://dashscope.aliyuncs.com/api-ws/v1/inference";

        // 创建带认证头的请求
        let request = http::Request::builder()
            .uri(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Sec-WebSocket-Key", tokio_tungstenite::tungstenite::handshake::client::generate_key())
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

        // 发送 run-task 指令
        let run_task = WsRequest {
            header: WsHeader {
                action: "run-task".to_string(),
                task_id: task_id.clone(),
                streaming: "duplex".to_string(),
            },
            payload: WsPayload {
                task_group: Some("audio".to_string()),
                task: Some("asr".to_string()),
                function: Some("recognition".to_string()),
                model: Some(self.model.clone()),
                parameters: Some(WsParameters {
                    format: "pcm".to_string(),
                    sample_rate,
                    language_hints: Some(vec!["zh".to_string(), "en".to_string()]),
                }),
                input: serde_json::json!({}),
            },
        };

        let run_task_json = serde_json::to_string(&run_task)
            .map_err(|e| AsrError::Encoding(e.to_string()))?;

        write
            .send(Message::Text(run_task_json.into()))
            .await
            .map_err(|e| AsrError::Network(e.to_string()))?;

        // 等待 task-started 事件
        let mut task_started = false;
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let response: WsResponse = serde_json::from_str(&text)
                        .map_err(|e| AsrError::Api(format!("解析响应失败: {}", e)))?;

                    if let Some(error_code) = &response.header.error_code {
                        return Err(AsrError::Api(format!(
                            "{}: {}",
                            error_code,
                            response.header.error_message.unwrap_or_default()
                        )));
                    }

                    if response.header.event == "task-started" {
                        task_started = true;
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

        if !task_started {
            return Err(AsrError::Api("未收到 task-started 事件".to_string()));
        }

        // 分块发送音频数据（每块约 3200 字节，对应 100ms @ 16kHz 16bit）
        let chunk_size = (sample_rate as usize) * 2 / 10; // 100ms 的数据量
        for chunk in audio_data.chunks(chunk_size) {
            write
                .send(Message::Binary(chunk.to_vec().into()))
                .await
                .map_err(|e| AsrError::Network(e.to_string()))?;
        }

        // 发送 finish-task 指令
        let finish_task = WsRequest {
            header: WsHeader {
                action: "finish-task".to_string(),
                task_id: task_id.clone(),
                streaming: "duplex".to_string(),
            },
            payload: WsPayload {
                task_group: None,
                task: None,
                function: None,
                model: None,
                parameters: None,
                input: serde_json::json!({}),
            },
        };

        let finish_task_json = serde_json::to_string(&finish_task)
            .map_err(|e| AsrError::Encoding(e.to_string()))?;

        write
            .send(Message::Text(finish_task_json.into()))
            .await
            .map_err(|e| AsrError::Network(e.to_string()))?;

        // 收集识别结果
        let mut final_text = String::new();

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let response: WsResponse = serde_json::from_str(&text)
                        .map_err(|e| AsrError::Api(format!("解析响应失败: {}", e)))?;

                    if let Some(error_code) = &response.header.error_code {
                        return Err(AsrError::Api(format!(
                            "{}: {}",
                            error_code,
                            response.header.error_message.unwrap_or_default()
                        )));
                    }

                    match response.header.event.as_str() {
                        "result-generated" => {
                            if let Some(payload) = response.payload {
                                if let Some(output) = payload.output {
                                    if let Some(sentence) = output.sentence {
                                        if let Some(text) = &sentence.text {
                                            tracing::debug!("ASR partial: {} (end={})", text, sentence.sentence_end);
                                            // 收集所有结果，不只是 sentence_end
                                            if sentence.sentence_end {
                                                final_text = text.clone();
                                            } else if final_text.is_empty() {
                                                // 如果还没有最终结果，先保存中间结果
                                                final_text = text.clone();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        "task-finished" => {
                            break;
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

/// 测试 DashScope API 连接
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
