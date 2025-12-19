use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};

use super::AudioError;

/// 录音控制命令
enum RecorderCommand {
    Start,
    Stop,
}

/// 录音状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecordingState {
    Idle,
    Recording,
}

/// 音频录制器 - 线程安全版本
pub struct AudioRecorder {
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
    state: Arc<Mutex<RecordingState>>,
    command_tx: Option<mpsc::Sender<RecorderCommand>>,
    worker_handle: Option<JoinHandle<()>>,
}

impl AudioRecorder {
    /// 创建新的录音器
    pub fn new() -> Result<Self, AudioError> {
        Ok(Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
            sample_rate: 16000, // Whisper 需要 16kHz
            channels: 1,       // 单声道
            state: Arc::new(Mutex::new(RecordingState::Idle)),
            command_tx: None,
            worker_handle: None,
        })
    }

    /// 开始录音
    pub fn start(&mut self) -> Result<(), AudioError> {
        {
            let state = self.state.lock().unwrap();
            if *state == RecordingState::Recording {
                return Ok(());
            }
        }

        // 清空缓冲区
        {
            let mut buffer = self.buffer.lock().unwrap();
            buffer.clear();
        }

        // 创建命令通道
        let (tx, rx) = mpsc::channel::<RecorderCommand>();
        self.command_tx = Some(tx);

        // 克隆需要的数据给工作线程
        let buffer = self.buffer.clone();
        let state = self.state.clone();
        let target_sample_rate = self.sample_rate;

        // 启动工作线程
        let handle = thread::spawn(move || {
            if let Err(e) = run_recording_loop(rx, buffer, state, target_sample_rate) {
                tracing::error!("Recording thread error: {}", e);
            }
        });

        self.worker_handle = Some(handle);

        // 发送开始命令
        if let Some(tx) = &self.command_tx {
            tx.send(RecorderCommand::Start).ok();
        }

        {
            let mut state = self.state.lock().unwrap();
            *state = RecordingState::Recording;
        }

        tracing::info!("Recording started");
        Ok(())
    }

    /// 停止录音并返回音频数据
    pub fn stop(&mut self) -> Result<Vec<f32>, AudioError> {
        {
            let state = self.state.lock().unwrap();
            if *state != RecordingState::Recording {
                return Ok(Vec::new());
            }
        }

        // 发送停止命令
        if let Some(tx) = self.command_tx.take() {
            tx.send(RecorderCommand::Stop).ok();
        }

        // 等待工作线程结束
        if let Some(handle) = self.worker_handle.take() {
            handle.join().ok();
        }

        {
            let mut state = self.state.lock().unwrap();
            *state = RecordingState::Idle;
        }

        // 获取录制的数据
        let buffer = self.buffer.lock().unwrap();
        let data = buffer.clone();

        tracing::info!("Recording stopped, {} samples collected", data.len());
        Ok(data)
    }

    /// 获取采样率
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// 获取声道数
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new().expect("Failed to create audio recorder")
    }
}

/// 在单独线程中运行录音循环
fn run_recording_loop(
    rx: mpsc::Receiver<RecorderCommand>,
    buffer: Arc<Mutex<Vec<f32>>>,
    _state: Arc<Mutex<RecordingState>>,
    target_sample_rate: u32,
) -> Result<(), AudioError> {
    // 等待开始命令
    match rx.recv() {
        Ok(RecorderCommand::Start) => {}
        _ => return Ok(()),
    }

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or(AudioError::NoInputDevice)?;

    let config = device
        .default_input_config()
        .map_err(|e| AudioError::Device(e.to_string()))?;

    tracing::info!(
        "Using input device: {:?}, config: {:?}",
        device.name(),
        config
    );

    let source_sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;

    // 计算精确的重采样比率
    let resample_ratio = source_sample_rate as f64 / target_sample_rate as f64;

    tracing::info!(
        "Resampling: {}Hz -> {}Hz, ratio: {:.4}",
        source_sample_rate,
        target_sample_rate,
        resample_ratio
    );

    let buffer_clone = buffer.clone();
    // 使用浮点累加器实现精确重采样
    let accumulator = Arc::new(Mutex::new(0.0f64));
    let accumulator_clone = accumulator.clone();

    // 构建输入流
    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut buffer = buffer_clone.lock().unwrap();
                let mut acc = accumulator_clone.lock().unwrap();

                // 转换为单声道并精确重采样
                for frame in data.chunks(channels) {
                    let mono: f32 = frame.iter().sum::<f32>() / channels as f32;

                    // 当累加器 >= 1.0 时输出一个样本
                    *acc += 1.0 / resample_ratio;
                    while *acc >= 1.0 {
                        buffer.push(mono);
                        *acc -= 1.0;
                    }
                }
            },
            |err| {
                tracing::error!("Audio stream error: {}", err);
            },
            None,
        )
        .map_err(|e| AudioError::Stream(e.to_string()))?;

    stream.play().map_err(|e| AudioError::Stream(e.to_string()))?;
    tracing::info!("Audio stream playing");

    // 等待停止命令
    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(RecorderCommand::Stop) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            _ => {}
        }
    }

    // 流会在 drop 时自动停止
    drop(stream);
    tracing::info!("Audio stream stopped");

    Ok(())
}
