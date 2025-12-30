use std::sync::atomic::{AtomicU8, Ordering};
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
    #[error("Operation cancelled")]
    Cancelled,
}

/// Pipeline 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PipelineState {
    Idle = 0,
    Recording = 1,
    Processing = 2,
}

impl From<u8> for PipelineState {
    fn from(v: u8) -> Self {
        match v {
            1 => PipelineState::Recording,
            2 => PipelineState::Processing,
            _ => PipelineState::Idle,
        }
    }
}

/// 语音处理管道
pub struct VoicePipeline {
    config: Arc<TokioRwLock<AppConfig>>,
    // 使用 std::sync::RwLock，因为 AudioRecorder 的操作是同步的
    // 这样避免了 blocking_write() 在 async 上下文中可能的问题
    recorder: Arc<RwLock<AudioRecorder>>,
    /// 当前状态（原子操作保证线程安全）
    state: AtomicU8,
    /// 取消标志
    cancelled: std::sync::atomic::AtomicBool,
}

impl VoicePipeline {
    /// 创建新的语音管道
    pub fn new(config: Arc<TokioRwLock<AppConfig>>) -> Result<Self, PipelineError> {
        let recorder = AudioRecorder::new()?;

        Ok(Self {
            config,
            recorder: Arc::new(RwLock::new(recorder)),
            state: AtomicU8::new(PipelineState::Idle as u8),
            cancelled: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// 获取当前状态
    pub fn get_state(&self) -> PipelineState {
        PipelineState::from(self.state.load(Ordering::SeqCst))
    }

    /// 是否正在录音
    pub fn is_recording(&self) -> bool {
        self.get_state() == PipelineState::Recording
    }

    /// 取消当前操作
    ///
    /// - 如果正在录音，停止录音并丢弃数据
    /// - 如果正在处理，标记取消（处理完成后返回 Cancelled 错误）
    /// - 如果空闲，无操作
    pub fn cancel(&self) -> Result<(), PipelineError> {
        let current = self.get_state();

        match current {
            PipelineState::Idle => {
                // 已经空闲，无需操作
                Ok(())
            }
            PipelineState::Recording => {
                // 停止录音并丢弃数据
                self.cancelled.store(true, Ordering::SeqCst);
                let mut recorder = self.recorder.write().map_err(|e| {
                    PipelineError::Other(format!("Failed to acquire recorder lock: {}", e))
                })?;
                let _ = recorder.stop(); // 忽略数据
                self.state.store(PipelineState::Idle as u8, Ordering::SeqCst);
                self.cancelled.store(false, Ordering::SeqCst);
                tracing::info!("Recording cancelled");
                Ok(())
            }
            PipelineState::Processing => {
                // 标记取消，异步处理会检查此标志
                self.cancelled.store(true, Ordering::SeqCst);
                tracing::info!("Processing cancellation requested");
                Ok(())
            }
        }
    }

    /// 开始录音
    pub fn start_recording(&self) -> Result<(), PipelineError> {
        // 检查状态，只有 Idle 才能开始
        let current = self.state.load(Ordering::SeqCst);
        if current != PipelineState::Idle as u8 {
            tracing::warn!("Cannot start recording: state is {:?}", PipelineState::from(current));
            return Err(PipelineError::Other("Pipeline is busy".to_string()));
        }

        // 重置取消标志
        self.cancelled.store(false, Ordering::SeqCst);

        let mut recorder = self.recorder.write().map_err(|e| {
            PipelineError::Other(format!("Failed to acquire recorder lock: {}", e))
        })?;
        recorder.start()?;

        self.state.store(PipelineState::Recording as u8, Ordering::SeqCst);
        Ok(())
    }

    /// 停止录音并处理，返回识别结果文本
    ///
    /// 此方法是幂等的：
    /// - 如果不在录音状态，直接返回空字符串
    /// - 如果已取消，返回 Cancelled 错误
    pub async fn stop_and_process(&self) -> Result<String, PipelineError> {
        // 检查是否已取消
        if self.cancelled.load(Ordering::SeqCst) {
            self.state.store(PipelineState::Idle as u8, Ordering::SeqCst);
            self.cancelled.store(false, Ordering::SeqCst);
            return Err(PipelineError::Cancelled);
        }

        // 幂等检查：非录音状态直接返回
        let current = self.state.load(Ordering::SeqCst);
        if current != PipelineState::Recording as u8 {
            tracing::warn!("stop_and_process called but not recording, state={:?}", PipelineState::from(current));
            return Ok(String::new());
        }

        // 转换到 Processing 状态
        self.state.store(PipelineState::Processing as u8, Ordering::SeqCst);

        // 停止录音 - 使用同步锁，快速获取并释放
        let samples = {
            let mut recorder = self.recorder.write().map_err(|e| {
                self.state.store(PipelineState::Idle as u8, Ordering::SeqCst);
                PipelineError::Other(format!("Failed to acquire recorder lock: {}", e))
            })?;
            recorder.stop()?
        };

        // 检查是否在停止后被取消
        if self.cancelled.load(Ordering::SeqCst) {
            self.state.store(PipelineState::Idle as u8, Ordering::SeqCst);
            self.cancelled.store(false, Ordering::SeqCst);
            return Err(PipelineError::Cancelled);
        }

        if samples.is_empty() {
            tracing::warn!("No audio data recorded");
            self.state.store(PipelineState::Idle as u8, Ordering::SeqCst);
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
            self.state.store(PipelineState::Idle as u8, Ordering::SeqCst);
            return Err(PipelineError::Other(
                "录音无声音，请检查麦克风权限是否已授予当前应用".to_string()
            ));
        }

        if max_amplitude < 0.05 {
            tracing::warn!(">>> AUDIO TOO QUIET (amplitude={:.6}) - speak louder or closer <<<", max_amplitude);
            self.state.store(PipelineState::Idle as u8, Ordering::SeqCst);
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

        // 检查取消标志
        if self.cancelled.load(Ordering::SeqCst) {
            self.state.store(PipelineState::Idle as u8, Ordering::SeqCst);
            self.cancelled.store(false, Ordering::SeqCst);
            return Err(PipelineError::Cancelled);
        }

        // 创建 ASR 服务并识别
        let asr_service = create_asr_service(&config.asr)?;
        let asr_result = match asr_service.recognize(&audio_data, sample_rate).await {
            Ok(r) => r,
            Err(e) => {
                self.state.store(PipelineState::Idle as u8, Ordering::SeqCst);
                return Err(e.into());
            }
        };

        tracing::info!("ASR result: {}", asr_result.text);

        // 再次检查取消标志
        if self.cancelled.load(Ordering::SeqCst) {
            self.state.store(PipelineState::Idle as u8, Ordering::SeqCst);
            self.cancelled.store(false, Ordering::SeqCst);
            return Err(PipelineError::Cancelled);
        }

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

        // 完成，恢复 Idle 状态
        self.state.store(PipelineState::Idle as u8, Ordering::SeqCst);
        tracing::info!("stop_and_process completed successfully");
        Ok(final_text)
    }
}
