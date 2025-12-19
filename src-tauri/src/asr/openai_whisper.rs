use async_trait::async_trait;
use reqwest::{multipart, Client};
use serde::Deserialize;

use super::traits::{AsrError, AsrResult, AsrService};

/// OpenAI Whisper ASR 服务
pub struct OpenAiWhisper {
    api_key: String,
    model: String,
    language: String,
    client: Client,
}

impl OpenAiWhisper {
    pub fn new(api_key: String, model: String, language: String) -> Self {
        Self {
            api_key,
            model,
            language,
            client: Client::new(),
        }
    }
}

#[derive(Deserialize)]
struct WhisperResponse {
    text: String,
}

#[derive(Deserialize)]
struct WhisperError {
    error: WhisperErrorDetail,
}

#[derive(Deserialize)]
struct WhisperErrorDetail {
    message: String,
}

#[async_trait]
impl AsrService for OpenAiWhisper {
    async fn recognize(&self, audio_data: &[u8], _sample_rate: u32) -> Result<AsrResult, AsrError> {
        // OpenAI Whisper API 需要 WAV 格式的文件
        let file_part = multipart::Part::bytes(audio_data.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| AsrError::Encoding(e.to_string()))?;

        let form = multipart::Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("language", self.language.clone())
            .text("response_format", "json");

        let response = self
            .client
            .post("https://api.openai.com/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| AsrError::Network(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| AsrError::Network(e.to_string()))?;

        if !status.is_success() {
            if let Ok(error) = serde_json::from_str::<WhisperError>(&body) {
                return Err(AsrError::Api(error.error.message));
            }
            return Err(AsrError::Api(format!("HTTP {}: {}", status, body)));
        }

        let result: WhisperResponse =
            serde_json::from_str(&body).map_err(|e| AsrError::Api(e.to_string()))?;

        Ok(AsrResult {
            text: result.text,
            is_final: true,
        })
    }
}

/// 测试 OpenAI API 连接
pub async fn test_api(api_key: &str) -> Result<String, AsrError> {
    let client = Client::new();

    let response = client
        .get("https://api.openai.com/v1/models")
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
