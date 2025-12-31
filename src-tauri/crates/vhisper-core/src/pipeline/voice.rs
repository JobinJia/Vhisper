use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::RwLock as TokioRwLock;

use crate::asr::{
    create_asr_service, create_streaming_asr_service, StreamingAsrEvent, StreamingControl,
};
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
    recorder: Arc<RwLock<AudioRecorder>>,
    /// 当前状态（Arc 包装以便后台任务共享）
    state: Arc<AtomicU8>,
    /// 取消标志（Arc 包装以便后台任务共享）
    cancelled: Arc<AtomicBool>,
    /// 流式模式标志（Arc 包装以便后台任务共享）
    streaming_mode: Arc<AtomicBool>,
    /// 流式 ASR 控制通道（用于发送音频和控制命令）
    /// 音频发送任务从这里读取当前活跃的 control_tx
    streaming_control_tx: Arc<TokioRwLock<Option<mpsc::Sender<StreamingControl>>>>,
    /// 流式任务取消标志（每次会话独立，用于通知后台任务停止）
    streaming_task_cancelled: Arc<TokioRwLock<Option<Arc<AtomicBool>>>>,
    /// 是否应该完全停止（热键松开时设为 true，区别于 VAD Final）
    should_stop: Arc<AtomicBool>,
}

impl VoicePipeline {
    /// 创建新的语音管道
    pub fn new(config: Arc<TokioRwLock<AppConfig>>) -> Result<Self, PipelineError> {
        let recorder = AudioRecorder::new()?;

        Ok(Self {
            config,
            recorder: Arc::new(RwLock::new(recorder)),
            state: Arc::new(AtomicU8::new(PipelineState::Idle as u8)),
            cancelled: Arc::new(AtomicBool::new(false)),
            streaming_mode: Arc::new(AtomicBool::new(false)),
            streaming_control_tx: Arc::new(TokioRwLock::new(None)),
            streaming_task_cancelled: Arc::new(TokioRwLock::new(None)),
            should_stop: Arc::new(AtomicBool::new(false)),
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

    // ========================================================================
    // 流式识别方法
    // ========================================================================

    /// 清理流式会话资源
    async fn cleanup_streaming(&self) {
        // 设置任务取消标志
        if let Some(task_cancelled) = self.streaming_task_cancelled.read().await.as_ref() {
            task_cancelled.store(true, Ordering::SeqCst);
        }

        // 清理控制通道
        {
            let mut tx_guard = self.streaming_control_tx.write().await;
            *tx_guard = None;
        }

        // 清理任务取消标志
        {
            let mut cancelled_guard = self.streaming_task_cancelled.write().await;
            *cancelled_guard = None;
        }
    }

    /// 开始流式录音和识别（支持连续输入模式）
    ///
    /// 返回事件接收器，用于接收识别结果
    ///
    /// 新架构特性：
    /// - 录音持续运行，不受 ASR 会话影响
    /// - VAD 触发 Final 后自动重连 ASR，继续接收音频
    /// - 只有调用 stop_streaming() 才会真正停止
    ///
    /// 使用方式:
    /// 1. 调用 start_streaming() 获取事件接收器
    /// 2. 从接收器读取 StreamingAsrEvent（Partial/Final）
    /// 3. Final 事件表示一句话结束，会自动开始新的识别
    /// 4. 调用 stop_streaming() 完全停止
    pub async fn start_streaming(&self) -> Result<mpsc::Receiver<StreamingAsrEvent>, PipelineError> {
        // 先停止旧会话（如果有）
        self.should_stop.store(true, Ordering::SeqCst);
        if let Some(task_cancelled) = self.streaming_task_cancelled.read().await.as_ref() {
            task_cancelled.store(true, Ordering::SeqCst);
        }
        {
            let mut tx_guard = self.streaming_control_tx.write().await;
            *tx_guard = None;
        }
        {
            let mut recorder = self.recorder.write().map_err(|e| {
                PipelineError::Other(format!("Failed to acquire recorder lock: {}", e))
            })?;
            let _ = recorder.stop();
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        // 检查状态
        let current = self.state.load(Ordering::SeqCst);
        if current != PipelineState::Idle as u8 {
            return Err(PipelineError::Other("Pipeline is busy".to_string()));
        }

        // 重置标志，开始新会话
        self.should_stop.store(false, Ordering::SeqCst);
        self.cancelled.store(false, Ordering::SeqCst);
        self.streaming_mode.store(true, Ordering::SeqCst);

        // 获取配置和采样率
        let config = self.config.read().await.clone();
        let sample_rate = {
            let recorder = self.recorder.read().map_err(|e| {
                PipelineError::Other(format!("Failed to acquire recorder lock: {}", e))
            })?;
            recorder.sample_rate()
        };

        // 创建首个 ASR 连接
        let streaming_service = create_streaming_asr_service(&config.asr)?;
        let (control_tx, event_rx) = streaming_service.start_streaming(sample_rate).await?;

        // 保存控制通道
        {
            let mut tx_guard = self.streaming_control_tx.write().await;
            *tx_guard = Some(control_tx);
        }

        // 启动录音
        {
            let mut recorder = self.recorder.write().map_err(|e| {
                PipelineError::Other(format!("Failed to acquire recorder lock: {}", e))
            })?;
            recorder.start()?;
        }

        self.state.store(PipelineState::Recording as u8, Ordering::SeqCst);

        // 创建事件转发通道
        let (forward_tx, forward_rx) = mpsc::channel::<StreamingAsrEvent>(32);

        // === 音频发送任务 ===
        // 持续运行，从 streaming_control_tx 读取当前活跃的 control_tx
        let recorder = self.recorder.clone();
        let should_stop_for_audio = self.should_stop.clone();
        let control_tx_holder = self.streaming_control_tx.clone();

        tokio::spawn(async move {
            let chunk_interval = Duration::from_millis(50);

            loop {
                // 检查是否应该停止
                if should_stop_for_audio.load(Ordering::SeqCst) {
                    tracing::info!("Audio task stopping: should_stop=true");
                    break;
                }

                // 获取音频数据
                let samples = {
                    let recorder_guard = match recorder.read() {
                        Ok(r) => r,
                        Err(_) => break,
                    };
                    recorder_guard.drain_buffer()
                };

                // 发送到当前活跃的 ASR 连接
                if !samples.is_empty() {
                    let pcm_data = encode_to_pcm(&samples);
                    if let Some(tx) = control_tx_holder.read().await.as_ref() {
                        // 忽略发送错误（ASR 可能在重连中）
                        let _ = tx.send(StreamingControl::Audio(pcm_data)).await;
                    }
                }

                tokio::time::sleep(chunk_interval).await;
            }
        });

        // === ASR 会话管理任务 ===
        // Final 后自动重连，直到 should_stop 为 true
        let should_stop_for_asr = self.should_stop.clone();
        let control_tx_holder_for_asr = self.streaming_control_tx.clone();
        let state = self.state.clone();
        let streaming_mode = self.streaming_mode.clone();
        let config_for_asr = config.clone();

        tokio::spawn(async move {
            let mut current_event_rx = event_rx;

            loop {
                // 处理当前 ASR 连接的事件
                // 注意：不在这里检查 should_stop，必须等到 Final/Error 才能退出
                while let Some(event) = current_event_rx.recv().await {
                    let is_final = matches!(event, StreamingAsrEvent::Final { .. });
                    let is_error = matches!(event, StreamingAsrEvent::Error(_));

                    // 转发事件
                    if forward_tx.send(event).await.is_err() {
                        tracing::info!("ASR task stopping: forward channel closed");
                        return;
                    }

                    // Final 事件：检查是否应该重连
                    if is_final {
                        if should_stop_for_asr.load(Ordering::SeqCst) {
                            // 热键已松开，不再重连，正常退出
                            tracing::info!("Final received, should_stop=true, stopping");
                            state.store(PipelineState::Idle as u8, Ordering::SeqCst);
                            streaming_mode.store(false, Ordering::SeqCst);
                            return;
                        } else {
                            // 热键还按着，VAD Final，自动重连
                            tracing::info!("VAD Final received, reconnecting ASR...");
                            break; // 跳出内层循环，重新创建 ASR 连接
                        }
                    }

                    // 错误：停止
                    if is_error {
                        tracing::error!("ASR error, stopping");
                        state.store(PipelineState::Idle as u8, Ordering::SeqCst);
                        streaming_mode.store(false, Ordering::SeqCst);
                        return;
                    }
                }

                // 内层循环因为 channel 关闭而退出（不是因为 Final）
                // 检查是否应该停止
                if should_stop_for_asr.load(Ordering::SeqCst) {
                    tracing::info!("ASR task stopping: channel closed, should_stop=true");
                    state.store(PipelineState::Idle as u8, Ordering::SeqCst);
                    streaming_mode.store(false, Ordering::SeqCst);
                    return;
                }

                // 重新创建 ASR 连接
                tracing::info!("Creating new ASR connection...");
                let new_service = match create_streaming_asr_service(&config_for_asr.asr) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to create ASR service: {}", e);
                        state.store(PipelineState::Idle as u8, Ordering::SeqCst);
                        streaming_mode.store(false, Ordering::SeqCst);
                        return;
                    }
                };

                let (new_control_tx, new_event_rx) = match new_service.start_streaming(sample_rate).await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!("Failed to start ASR streaming: {}", e);
                        state.store(PipelineState::Idle as u8, Ordering::SeqCst);
                        streaming_mode.store(false, Ordering::SeqCst);
                        return;
                    }
                };

                // 更新共享的 control_tx（音频发送任务会自动使用新的）
                {
                    let mut tx_guard = control_tx_holder_for_asr.write().await;
                    *tx_guard = Some(new_control_tx);
                }

                current_event_rx = new_event_rx;
                tracing::info!("ASR reconnected successfully");
            }
        });

        Ok(forward_rx)
    }

    /// 停止流式录音（真正停止，不再自动重连）
    ///
    /// 提交当前音频缓冲区，等待最终识别结果
    pub async fn stop_streaming(&self) -> Result<(), PipelineError> {
        // 检查是否在流式模式
        if !self.streaming_mode.load(Ordering::SeqCst) {
            return Ok(());
        }

        tracing::info!("stop_streaming: setting should_stop=true");

        // 设置停止标志，通知后台任务停止
        self.should_stop.store(true, Ordering::SeqCst);

        // 停止录音
        {
            let mut recorder = self.recorder.write().map_err(|e| {
                PipelineError::Other(format!("Failed to acquire recorder lock: {}", e))
            })?;
            let _ = recorder.stop();
        }

        // 发送最后一批音频和 commit
        if let Some(control_tx) = self.streaming_control_tx.read().await.as_ref() {
            // 获取剩余音频
            let samples = {
                let recorder = self.recorder.read().map_err(|e| {
                    PipelineError::Other(format!("Failed to acquire recorder lock: {}", e))
                })?;
                recorder.drain_buffer()
            };

            if !samples.is_empty() {
                let pcm_data = encode_to_pcm(&samples);
                let _ = control_tx.send(StreamingControl::Audio(pcm_data)).await;
            }

            // 提交
            let _ = control_tx.send(StreamingControl::Commit).await;
        }

        self.state.store(PipelineState::Processing as u8, Ordering::SeqCst);

        Ok(())
    }

    /// 取消流式识别（立即停止，不提交）
    pub async fn cancel_streaming(&self) -> Result<(), PipelineError> {
        if !self.streaming_mode.load(Ordering::SeqCst) {
            return Ok(());
        }

        tracing::info!("cancel_streaming: setting should_stop=true");

        // 设置停止标志
        self.should_stop.store(true, Ordering::SeqCst);

        // 停止录音
        {
            let mut recorder = self.recorder.write().map_err(|e| {
                PipelineError::Other(format!("Failed to acquire recorder lock: {}", e))
            })?;
            let _ = recorder.stop();
        }

        // 发送取消命令
        if let Some(control_tx) = self.streaming_control_tx.read().await.as_ref() {
            let _ = control_tx.send(StreamingControl::Cancel).await;
        }

        // 清理所有资源
        self.cleanup_streaming().await;

        self.streaming_mode.store(false, Ordering::SeqCst);
        self.cancelled.store(true, Ordering::SeqCst);
        self.state.store(PipelineState::Idle as u8, Ordering::SeqCst);

        Ok(())
    }

    /// 是否在流式模式
    pub fn is_streaming(&self) -> bool {
        self.streaming_mode.load(Ordering::SeqCst)
    }
}
