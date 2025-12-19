use std::sync::{Arc, RwLock};
use tokio::sync::RwLock as TokioRwLock;

use crate::asr::create_asr_service;
use crate::audio::{encode_to_pcm, encode_to_wav, AudioRecorder};
use crate::config::AppConfig;
use crate::llm::create_llm_service;
use crate::output;

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Audio error: {0}")]
    Audio(#[from] crate::audio::AudioError),
    #[error("ASR error: {0}")]
    Asr(#[from] crate::asr::AsrError),
    #[error("LLM error: {0}")]
    Llm(#[from] crate::llm::LlmError),
    #[error("Output error: {0}")]
    Output(#[from] crate::output::OutputError),
    #[error("Pipeline error: {0}")]
    Other(String),
}

/// 语音处理管道
pub struct VoicePipeline {
    config: Arc<TokioRwLock<AppConfig>>,
    // 使用 std::sync::RwLock，因为 AudioRecorder 的操作是同步的
    // 这样避免了 blocking_write() 在 async 上下文中可能的问题
    recorder: Arc<RwLock<AudioRecorder>>,
}

impl VoicePipeline {
    /// 创建新的语音管道
    pub fn new(config: Arc<TokioRwLock<AppConfig>>) -> Result<Self, PipelineError> {
        let recorder = AudioRecorder::new()?;

        Ok(Self {
            config,
            recorder: Arc::new(RwLock::new(recorder)),
        })
    }

    /// 开始录音
    pub fn start_recording(&self) -> Result<(), PipelineError> {
        let mut recorder = self.recorder.write().map_err(|e| {
            PipelineError::Other(format!("Failed to acquire recorder lock: {}", e))
        })?;
        recorder.start()?;
        Ok(())
    }

    /// 停止录音并处理
    ///
    /// 参数:
    /// - `original_app_pid`: 开始录音时的应用 PID，用于智能粘贴判断
    pub async fn stop_and_process(&self, original_app_pid: Option<i32>) -> Result<String, PipelineError> {
        // 停止录音 - 使用同步锁，快速获取并释放
        let samples = {
            let mut recorder = self.recorder.write().map_err(|e| {
                PipelineError::Other(format!("Failed to acquire recorder lock: {}", e))
            })?;
            recorder.stop()?
        };

        if samples.is_empty() {
            tracing::warn!("No audio data recorded");
            return Ok(String::new());
        }

        let config = self.config.read().await.clone();
        let sample_rate = {
            let recorder = self.recorder.read().map_err(|e| {
                PipelineError::Other(format!("Failed to acquire recorder lock: {}", e))
            })?;
            recorder.sample_rate()
        };

        tracing::info!("Processing {} samples at {}Hz", samples.len(), sample_rate);

        // 编码音频数据
        let audio_data = if config.asr.provider == "OpenAIWhisper" {
            // OpenAI Whisper 需要 WAV 格式
            let channels = {
                let recorder = self.recorder.read().map_err(|e| {
                    PipelineError::Other(format!("Failed to acquire recorder lock: {}", e))
                })?;
                recorder.channels()
            };
            encode_to_wav(&samples, sample_rate, channels)?
        } else {
            // 其他服务使用 PCM
            encode_to_pcm(&samples)
        };

        // 创建 ASR 服务并识别
        let asr_service = create_asr_service(&config.asr)?;
        let asr_result = asr_service.recognize(&audio_data, sample_rate).await?;

        tracing::info!("ASR result: {}", asr_result.text);

        let mut final_text = asr_result.text.clone();

        // 如果启用了 LLM，进行文本优化
        if config.llm.enabled && !final_text.is_empty() {
            if let Ok(Some(llm_service)) = create_llm_service(&config.llm) {
                match llm_service.refine_text(&final_text).await {
                    Ok(refined) => {
                        tracing::info!("LLM refined: {} -> {}", final_text, refined);
                        final_text = refined;
                    }
                    Err(e) => {
                        tracing::warn!("LLM refinement failed, using original: {}", e);
                    }
                }
            }
        }

        // 输出文本
        if !final_text.is_empty() {
            tracing::info!("About to output text: {}", final_text);
            match output::output_text(
                &final_text,
                config.output.restore_clipboard,
                config.output.paste_delay_ms,
                original_app_pid,
            ) {
                Ok(_) => {
                    tracing::info!("Text output successfully: {}", final_text);
                }
                Err(e) => {
                    tracing::error!("Text output failed: {}", e);
                    return Err(e.into());
                }
            }
        }

        tracing::info!("stop_and_process completed successfully");
        Ok(final_text)
    }
}
