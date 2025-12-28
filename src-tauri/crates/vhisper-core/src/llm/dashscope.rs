use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::traits::{LlmError, LlmService, REFINE_PROMPT};

/// DashScope LLM 服务 (通义千问)
pub struct DashScopeLlm {
    api_key: String,
    model: String,
    client: Client,
}

impl DashScopeLlm {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct DashScopeRequest {
    model: String,
    input: DashScopeInput,
}

#[derive(Serialize)]
struct DashScopeInput {
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct DashScopeResponse {
    output: Option<DashScopeOutput>,
    message: Option<String>,
}

#[derive(Deserialize)]
struct DashScopeOutput {
    text: Option<String>,
    choices: Option<Vec<Choice>>,
}

#[derive(Deserialize)]
struct Choice {
    message: Option<ChoiceMessage>,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: String,
}

#[async_trait]
impl LlmService for DashScopeLlm {
    async fn refine_text(&self, text: &str) -> Result<String, LlmError> {
        let request = DashScopeRequest {
            model: self.model.clone(),
            input: DashScopeInput {
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
            },
        };

        let response = self
            .client
            .post("https://dashscope.aliyuncs.com/api/v1/services/aigc/text-generation/generation")
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

        let result: DashScopeResponse =
            serde_json::from_str(&body).map_err(|e| LlmError::Api(e.to_string()))?;

        if let Some(message) = &result.message {
            if !message.is_empty() && result.output.is_none() {
                return Err(LlmError::Api(message.clone()));
            }
        }

        let output_text = result
            .output
            .and_then(|o| {
                o.text.or(o.choices.and_then(|c| {
                    c.into_iter()
                        .next()
                        .and_then(|choice| choice.message.map(|m| m.content))
                }))
            })
            .unwrap_or_else(|| text.to_string());

        Ok(output_text.trim().to_string())
    }
}
