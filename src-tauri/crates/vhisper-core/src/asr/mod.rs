mod dashscope;
mod funasr;
mod openai_whisper;
mod qwen;
mod qwen_realtime;
mod traits;

pub use dashscope::DashScopeAsr;
pub use funasr::FunAsr;
pub use openai_whisper::OpenAiWhisper;
pub use qwen::QwenAsr;
pub use qwen_realtime::QwenRealtimeAsr;
pub use traits::{AsrError, AsrResult, AsrService, StreamingAsrEvent, StreamingAsrService, StreamingControl};

use crate::config::settings::AsrConfig;

/// 根据配置创建 ASR 服务
pub fn create_asr_service(config: &AsrConfig) -> Result<Box<dyn AsrService>, AsrError> {
    match config.provider.as_str() {
        "Qwen" => {
            let qwen_config = config
                .qwen
                .as_ref()
                .ok_or_else(|| AsrError::Config("通义千问 ASR 配置缺失".to_string()))?;
            Ok(Box::new(QwenAsr::new(
                qwen_config.api_key.clone(),
                qwen_config.model.clone(),
            )))
        }
        "DashScope" => {
            let dashscope_config = config
                .dashscope
                .as_ref()
                .ok_or_else(|| AsrError::Config("DashScope 配置缺失".to_string()))?;
            Ok(Box::new(DashScopeAsr::new(
                dashscope_config.api_key.clone(),
                dashscope_config.model.clone(),
            )))
        }
        "OpenAIWhisper" => {
            let openai_config = config
                .openai
                .as_ref()
                .ok_or_else(|| AsrError::Config("OpenAI 配置缺失".to_string()))?;
            Ok(Box::new(OpenAiWhisper::new(
                openai_config.api_key.clone(),
                openai_config.model.clone(),
                openai_config.language.clone(),
            )))
        }
        "FunAsr" => {
            let funasr_config = config
                .funasr
                .as_ref()
                .ok_or_else(|| AsrError::Config("FunASR 配置缺失".to_string()))?;
            Ok(Box::new(FunAsr::new(funasr_config.endpoint.clone())))
        }
        _ => Err(AsrError::Config(format!(
            "未知的 ASR 服务商: {}",
            config.provider
        ))),
    }
}

/// 测试通义千问 ASR API
pub async fn test_qwen_api(api_key: &str) -> Result<String, AsrError> {
    qwen::test_api(api_key).await
}

/// 测试 DashScope API
pub async fn test_dashscope_api(api_key: &str) -> Result<String, AsrError> {
    dashscope::test_api(api_key).await
}

/// 测试 OpenAI API
pub async fn test_openai_api(api_key: &str) -> Result<String, AsrError> {
    openai_whisper::test_api(api_key).await
}

/// 测试 FunASR API
pub async fn test_funasr_api(endpoint: &str) -> Result<String, AsrError> {
    funasr::test_api(endpoint).await
}

/// 根据配置创建流式 ASR 服务
pub fn create_streaming_asr_service(
    config: &AsrConfig,
) -> Result<Box<dyn StreamingAsrService>, AsrError> {
    match config.provider.as_str() {
        "Qwen" => {
            let qwen_config = config
                .qwen
                .as_ref()
                .ok_or_else(|| AsrError::Config("通义千问 ASR 配置缺失".to_string()))?;
            Ok(Box::new(QwenRealtimeAsr::new(
                qwen_config.api_key.clone(),
                qwen_config.model.clone(),
            )))
        }
        _ => Err(AsrError::Config(format!(
            "ASR 服务商 {} 不支持流式识别",
            config.provider
        ))),
    }
}
