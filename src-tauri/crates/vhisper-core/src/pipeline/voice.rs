use std::sync::{Arc, RwLock};
use tokio::sync::RwLock as TokioRwLock;

use crate::asr::create_asr_service;
use crate::audio::{encode_to_pcm, encode_to_wav, AudioRecorder};
use crate::config::AppConfig;
use crate::llm::create_llm_service;

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Audio error: {0}")]
    Audio(#[from] crate::audio::AudioError),
    #[error("ASR error: {0}")]
    Asr(#[from] crate::asr::AsrError),
    #[error("LLM error: {0}")]
    Llm(#[from] crate::llm::LlmError),
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

    /// 停止录音并处理，返回识别结果文本
    pub async fn stop_and_process(&self) -> Result<String, PipelineError> {
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

        // 检测是否全静音
        let max_amplitude = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        let avg_amplitude = samples.iter().map(|s| s.abs()).sum::<f32>() / samples.len() as f32;
        let non_zero_count = samples.iter().filter(|&&s| s != 0.0).count();

        tracing::info!(
            "Audio stats: max={:.6}, avg={:.6}, non_zero={}/{}, threshold=0.001",
            max_amplitude, avg_amplitude, non_zero_count, samples.len()
        );

        // 阈值判断：
        // < 0.001 = 完全静音（权限问题）
        // < 0.05  = 音量太低（只有背景噪音）
        // >= 0.05 = 正常语音
        if max_amplitude < 0.001 {
            tracing::warn!(">>> SILENT (amplitude={:.6}) - likely permission issue <<<", max_amplitude);
            return Err(PipelineError::Other(
                "录音无声音，请检查麦克风权限是否已授予当前应用".to_string()
            ));
        }

        if max_amplitude < 0.05 {
            tracing::warn!(">>> AUDIO TOO QUIET (amplitude={:.6}) - speak louder or closer <<<", max_amplitude);
            return Err(PipelineError::Other(
                "录音音量太低，请靠近麦克风或大声说话".to_string()
            ));
        }

        tracing::info!("Audio OK, proceeding to ASR...");

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

        tracing::info!("stop_and_process completed successfully");
        Ok(final_text)
    }
}
