mod dashscope;
mod ollama;
mod openai;
mod traits;

pub use dashscope::DashScopeLlm;
pub use ollama::OllamaLlm;
pub use openai::OpenAiLlm;
pub use traits::{LlmError, LlmService};

use crate::config::settings::LlmConfig;

/// 根据配置创建 LLM 服务
pub fn create_llm_service(config: &LlmConfig) -> Result<Option<Box<dyn LlmService>>, LlmError> {
    if !config.enabled {
        return Ok(None);
    }

    match config.provider.as_str() {
        "DashScope" => {
            let dashscope_config = config
                .dashscope
                .as_ref()
                .ok_or_else(|| LlmError::Config("DashScope LLM 配置缺失".to_string()))?;
            Ok(Some(Box::new(DashScopeLlm::new(
                dashscope_config.api_key.clone(),
                dashscope_config.model.clone(),
            ))))
        }
        "OpenAI" => {
            let openai_config = config
                .openai
                .as_ref()
                .ok_or_else(|| LlmError::Config("OpenAI LLM 配置缺失".to_string()))?;
            Ok(Some(Box::new(OpenAiLlm::new(
                openai_config.api_key.clone(),
                openai_config.model.clone(),
                openai_config.temperature,
                openai_config.max_tokens,
            ))))
        }
        "Ollama" => {
            let ollama_config = config
                .ollama
                .as_ref()
                .ok_or_else(|| LlmError::Config("Ollama 配置缺失".to_string()))?;
            Ok(Some(Box::new(OllamaLlm::new(
                ollama_config.endpoint.clone(),
                ollama_config.model.clone(),
            ))))
        }
        _ => Err(LlmError::Config(format!(
            "未知的 LLM 服务商: {}",
            config.provider
        ))),
    }
}

/// 测试 Ollama API
pub async fn test_ollama_api(endpoint: &str, model: &str) -> Result<String, LlmError> {
    ollama::test_api(endpoint, model).await
}
