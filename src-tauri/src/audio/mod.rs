mod recorder;

pub use recorder::AudioRecorder;

use std::io::Cursor;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("No input device found")]
    NoInputDevice,
    #[error("Stream error: {0}")]
    Stream(String),
    #[error("Encoding error: {0}")]
    Encoding(String),
    #[error("Device error: {0}")]
    Device(String),
}

/// 将 f32 采样数据编码为 PCM 格式 (16-bit little-endian)
pub fn encode_to_pcm(samples: &[f32]) -> Vec<u8> {
    let mut pcm_data = Vec::with_capacity(samples.len() * 2);
    for &sample in samples {
        // 将 f32 (-1.0 到 1.0) 转换为 i16
        let amplitude = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        pcm_data.extend_from_slice(&amplitude.to_le_bytes());
    }
    pcm_data
}

/// 将 f32 采样数据编码为 WAV 格式
pub fn encode_to_wav(samples: &[f32], sample_rate: u32, channels: u16) -> Result<Vec<u8>, AudioError> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut cursor, spec)
        .map_err(|e| AudioError::Encoding(e.to_string()))?;

    for &sample in samples {
        // 将 f32 (-1.0 到 1.0) 转换为 i16
        let amplitude = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer
            .write_sample(amplitude)
            .map_err(|e| AudioError::Encoding(e.to_string()))?;
    }

    writer
        .finalize()
        .map_err(|e| AudioError::Encoding(e.to_string()))?;

    Ok(cursor.into_inner())
}
