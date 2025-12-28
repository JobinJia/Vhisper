use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum AsrError {
    #[error("API error: {0}")]
    Api(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Audio encoding error: {0}")]
    Encoding(String),
    #[error("Configuration error: {0}")]
    Config(String),
}

/// ASR 识别结果
#[derive(Debug, Clone)]
pub struct AsrResult {
    pub text: String,
    pub is_final: bool,
}

/// ASR 服务 trait
#[async_trait]
pub trait AsrService: Send + Sync {
    /// 识别音频数据
    async fn recognize(&self, audio_data: &[u8], sample_rate: u32) -> Result<AsrResult, AsrError>;
}
