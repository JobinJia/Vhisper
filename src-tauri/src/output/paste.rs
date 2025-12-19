use std::thread;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum PasteError {
    #[error("Paste error: {0}")]
    Paste(String),
}

/// 模拟粘贴操作
pub fn simulate_paste(delay_ms: u64) -> Result<(), PasteError> {
    tracing::info!("simulate_paste: sleeping for {}ms", delay_ms);
    // 等待一小段时间，确保剪贴板内容已就绪
    thread::sleep(Duration::from_millis(delay_ms));
    tracing::info!("simulate_paste: sleep done");

    // 在 macOS 上使用 AppleScript 执行粘贴，避免 enigo 与 IMK 的冲突
    #[cfg(target_os = "macos")]
    {
        tracing::info!("simulate_paste: using AppleScript for paste");
        let output = std::process::Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to keystroke \"v\" using command down",
            ])
            .output()
            .map_err(|e| {
                tracing::error!("simulate_paste: osascript failed: {}", e);
                PasteError::Paste(e.to_string())
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!("simulate_paste: AppleScript error: {}", stderr);
            return Err(PasteError::Paste(format!("AppleScript error: {}", stderr)));
        }

        tracing::info!("simulate_paste: AppleScript paste successful");
    }

    #[cfg(target_os = "windows")]
    {
        use enigo::{Enigo, Key, Keyboard, Settings};

        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| PasteError::Paste(e.to_string()))?;

        enigo
            .key(Key::Control, enigo::Direction::Press)
            .map_err(|e| PasteError::Paste(e.to_string()))?;
        enigo
            .key(Key::Unicode('v'), enigo::Direction::Click)
            .map_err(|e| PasteError::Paste(e.to_string()))?;
        enigo
            .key(Key::Control, enigo::Direction::Release)
            .map_err(|e| PasteError::Paste(e.to_string()))?;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        use enigo::{Enigo, Key, Keyboard, Settings};

        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| PasteError::Paste(e.to_string()))?;

        enigo
            .key(Key::Control, enigo::Direction::Press)
            .map_err(|e| PasteError::Paste(e.to_string()))?;
        enigo
            .key(Key::Unicode('v'), enigo::Direction::Click)
            .map_err(|e| PasteError::Paste(e.to_string()))?;
        enigo
            .key(Key::Control, enigo::Direction::Release)
            .map_err(|e| PasteError::Paste(e.to_string()))?;
    }

    tracing::info!("simulate_paste: completed successfully");
    Ok(())
}
