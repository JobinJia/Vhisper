use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;

use super::traits::{AsrError, AsrResult, AsrService};

/// FunASR 本地服务 (WebSocket 实时语音识别)
pub struct FunAsr {
    endpoint: String,
}

impl FunAsr {
    pub fn new(endpoint: String) -> Self {
        // 将 HTTP 端点转换为 WebSocket Secure 端点 (FunASR 默认启用 SSL)
        let ws_endpoint = endpoint
            .replace("http://", "wss://")
            .replace("https://", "wss://")
            .replace("ws://", "wss://");
        Self {
            endpoint: ws_endpoint,
        }
    }
}

/// 创建接受自签名证书的 TLS 连接器
fn create_tls_connector() -> Result<tokio_tungstenite::Connector, AsrError> {
    let tls_connector = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
        .map_err(|e| AsrError::Network(format!("TLS 配置失败: {}", e)))?;
    Ok(tokio_tungstenite::Connector::NativeTls(tls_connector))
}

// FunASR WebSocket 请求结构
#[derive(Serialize)]
struct FunAsrStartMessage {
    chunk_size: Vec<i32>,
    chunk_interval: i32,
    wav_name: String,
    wav_format: String,
    audio_fs: u32,
    itn: bool,
    is_speaking: bool,
}

#[derive(Serialize)]
struct FunAsrEndMessage {
    is_speaking: bool,
}

// FunASR WebSocket 响应结构
#[derive(Deserialize, Debug)]
struct FunAsrResponse {
    text: Option<String>,
    #[serde(default)]
    is_final: bool,
    mode: Option<String>,
}

#[async_trait]
impl AsrService for FunAsr {
    async fn recognize(&self, audio_data: &[u8], sample_rate: u32) -> Result<AsrResult, AsrError> {
        // 创建 TLS 连接器（接受自签名证书）
        let connector = create_tls_connector()?;

        // 连接 WebSocket (使用 wss://)
        let (ws_stream, _) = tokio_tungstenite::connect_async_tls_with_config(
            &self.endpoint,
            None,
            false,
            Some(connector),
        )
        .await
        .map_err(|e| AsrError::Network(format!("WebSocket 连接失败: {}", e)))?;

        let (mut write, mut read) = ws_stream.split();

        // 发送开始消息
        let start_msg = FunAsrStartMessage {
            chunk_size: vec![5, 10, 5],
            chunk_interval: 10,
            wav_name: "audio".to_string(),
            wav_format: "pcm".to_string(),
            audio_fs: sample_rate,
            itn: true,
            is_speaking: true,
        };

        let start_json = serde_json::to_string(&start_msg)
            .map_err(|e| AsrError::Encoding(e.to_string()))?;

        write
            .send(Message::Text(start_json.into()))
            .await
            .map_err(|e| AsrError::Network(e.to_string()))?;

        // 分块发送音频数据（每块约 6400 字节，对应 200ms @ 16kHz 16bit）
        let chunk_size = (sample_rate as usize) * 2 / 5; // 200ms 的数据量
        for chunk in audio_data.chunks(chunk_size) {
            write
                .send(Message::Binary(chunk.to_vec().into()))
                .await
                .map_err(|e| AsrError::Network(e.to_string()))?;
        }

        // 发送结束消息
        let end_msg = FunAsrEndMessage { is_speaking: false };
        let end_json = serde_json::to_string(&end_msg)
            .map_err(|e| AsrError::Encoding(e.to_string()))?;

        write
            .send(Message::Text(end_json.into()))
            .await
            .map_err(|e| AsrError::Network(e.to_string()))?;

        // 收集识别结果
        let mut final_text = String::new();

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(response) = serde_json::from_str::<FunAsrResponse>(&text) {
                        if let Some(result_text) = response.text {
                            // FunASR 返回的是累积结果，取最后一个
                            final_text = result_text;
                        }
                        // 如果是最终结果或者模式是 offline，则结束
                        if response.is_final || response.mode.as_deref() == Some("offline") {
                            break;
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    break;
                }
                Err(e) => {
                    // 如果已经有结果，忽略关闭错误
                    if final_text.is_empty() {
                        return Err(AsrError::Network(e.to_string()));
                    }
                    break;
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

/// 测试 FunASR 服务连接
pub async fn test_api(endpoint: &str) -> Result<String, AsrError> {
    // 将 HTTP 端点转换为 WebSocket Secure 端点
    let ws_endpoint = endpoint
        .replace("http://", "wss://")
        .replace("https://", "wss://")
        .replace("ws://", "wss://");

    // 创建 TLS 连接器（接受自签名证书）
    let connector = create_tls_connector()?;

    // 尝试建立 WebSocket 连接
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio_tungstenite::connect_async_tls_with_config(
            &ws_endpoint,
            None,
            false,
            Some(connector),
        ),
    )
    .await;

    match result {
        Ok(Ok(_)) => Ok("FunASR 服务连接成功".to_string()),
        Ok(Err(e)) => Err(AsrError::Network(format!("WebSocket 连接失败: {}", e))),
        Err(_) => Err(AsrError::Network("连接超时".to_string())),
    }
}
