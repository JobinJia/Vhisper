use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::traits::{LlmError, LlmService, REFINE_PROMPT};

/// OpenAI LLM 服务
pub struct OpenAiLlm {
    api_key: String,
    model: String,
    temperature: f32,
    max_tokens: u32,
    client: Client,
}

impl OpenAiLlm {
    pub fn new(api_key: String, model: String, temperature: f32, max_tokens: u32) -> Self {
        Self {
            api_key,
            model,
            temperature,
            max_tokens,
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Option<Vec<Choice>>,
    error: Option<OpenAiError>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: String,
}

#[derive(Deserialize)]
struct OpenAiError {
    message: String,
}

#[async_trait]
impl LlmService for OpenAiLlm {
    async fn refine_text(&self, text: &str) -> Result<String, LlmError> {
        let request = OpenAiRequest {
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
            temperature: self.temperature,
            max_tokens: self.max_tokens,
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
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

        let result: OpenAiResponse =
            serde_json::from_str(&body).map_err(|e| LlmError::Api(e.to_string()))?;

        if let Some(error) = result.error {
            return Err(LlmError::Api(error.message));
        }

        let output_text = result
            .choices
            .and_then(|c| c.into_iter().next().map(|choice| choice.message.content))
            .unwrap_or_else(|| text.to_string());

        Ok(output_text.trim().to_string())
    }
}
