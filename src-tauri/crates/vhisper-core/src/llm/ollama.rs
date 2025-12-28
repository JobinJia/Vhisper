use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::traits::{LlmError, LlmService, REFINE_PROMPT};

/// Ollama 本地 LLM 服务
pub struct OllamaLlm {
    endpoint: String,
    model: String,
    client: Client,
}

impl OllamaLlm {
    pub fn new(endpoint: String, model: String) -> Self {
        Self {
            endpoint,
            model,
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: Option<Message>,
    error: Option<String>,
}

#[async_trait]
impl LlmService for OllamaLlm {
    async fn refine_text(&self, text: &str) -> Result<String, LlmError> {
        let url = format!("{}/api/chat", self.endpoint.trim_end_matches('/'));

        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: REFINE_PROMPT.to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: text.to_string(),
                },
            ],
            stream: false,
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        if !status.is_success() {
            return Err(LlmError::Api(format!("HTTP {}: {}", status, body)));
        }

        let result: OllamaChatResponse =
            serde_json::from_str(&body).map_err(|e| LlmError::Api(e.to_string()))?;

        if let Some(error) = result.error {
            return Err(LlmError::Api(error));
        }

        let output_text = result
            .message
            .map(|m| m.content)
            .unwrap_or_else(|| text.to_string());

        Ok(output_text.trim().to_string())
    }
}

/// 测试 Ollama 服务连接
pub async fn test_api(endpoint: &str, model: &str) -> Result<String, LlmError> {
    let client = Client::new();
    let url = format!("{}/api/tags", endpoint.trim_end_matches('/'));

    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| LlmError::Network(format!("无法连接到 Ollama: {}", e)))?;

    if !response.status().is_success() {
        return Err(LlmError::Api(format!(
            "Ollama 服务错误: HTTP {}",
            response.status()
        )));
    }

    // 检查模型是否存在
    let body = response
        .text()
        .await
        .map_err(|e| LlmError::Network(e.to_string()))?;

    #[derive(Deserialize)]
    struct TagsResponse {
        models: Option<Vec<ModelInfo>>,
    }

    #[derive(Deserialize)]
    struct ModelInfo {
        name: String,
    }

    let tags: TagsResponse =
        serde_json::from_str(&body).map_err(|e| LlmError::Api(e.to_string()))?;

    if let Some(models) = tags.models {
        let model_exists = models.iter().any(|m| m.name.starts_with(model));
        if model_exists {
            Ok(format!("Ollama 连接成功，模型 {} 可用", model))
        } else {
            let available: Vec<_> = models.iter().map(|m| m.name.as_str()).collect();
            Err(LlmError::Api(format!(
                "模型 {} 未找到。可用模型: {}",
                model,
                available.join(", ")
            )))
        }
    } else {
        Ok("Ollama 连接成功".to_string())
    }
}
