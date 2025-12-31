use async_trait::async_trait;
use tokio::sync::mpsc;

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
    #[error("Session error: {0}")]
    Session(String),
    #[error("Cancelled")]
    Cancelled,
}

/// ASR 识别结果
#[derive(Debug, Clone)]
pub struct AsrResult {
    pub text: String,
    pub is_final: bool,
}

/// 流式识别事件
#[derive(Debug, Clone)]
pub enum StreamingAsrEvent {
    /// 中间结果
    /// - text: 已确认的文本（不会再变）
    /// - stash: 暂定文本（可能被后续修正）
    Partial { text: String, stash: String },
    /// 最终结果（会话结束）
    Final { text: String },
    /// 错误
    Error(String),
}

/// 流式会话控制命令
#[derive(Debug)]
pub enum StreamingControl {
    /// 发送音频数据
    Audio(Vec<u8>),
    /// 提交缓冲区（触发识别确认）
    Commit,
    /// 取消会话
    Cancel,
}

/// ASR 服务 trait（批量模式）
#[async_trait]
pub trait AsrService: Send + Sync {
    /// 识别音频数据
    async fn recognize(&self, audio_data: &[u8], sample_rate: u32) -> Result<AsrResult, AsrError>;
}

/// 流式 ASR 服务 trait
#[async_trait]
pub trait StreamingAsrService: Send + Sync {
    /// 开始流式识别会话
    ///
    /// 返回:
    /// - 控制发送器：发送音频数据和控制命令
    /// - 事件接收器：接收识别事件
    ///
    /// 使用方式:
    /// 1. 调用 start_streaming 获取通道
    /// 2. 通过 control_tx 发送 StreamingControl::Audio(data) 推送音频
    /// 3. 通过 event_rx 接收 StreamingAsrEvent 获取识别结果
    /// 4. 发送 StreamingControl::Commit 触发最终确认
    /// 5. 等待 StreamingAsrEvent::Final 获取最终结果
    async fn start_streaming(
        &self,
        sample_rate: u32,
    ) -> Result<(mpsc::Sender<StreamingControl>, mpsc::Receiver<StreamingAsrEvent>), AsrError>;
}
