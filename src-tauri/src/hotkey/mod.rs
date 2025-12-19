#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

use tauri::AppHandle;

#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    #[error("Hotkey error: {0}")]
    Error(String),
}

/// 启动快捷键监听
pub fn start_listener(app_handle: AppHandle) -> Result<(), HotkeyError> {
    #[cfg(target_os = "macos")]
    {
        macos::start_listener(app_handle).map_err(|e| HotkeyError::Error(e.to_string()))
    }

    #[cfg(target_os = "windows")]
    {
        windows::start_listener(app_handle).map_err(|e| HotkeyError::Error(e.to_string()))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(HotkeyError::Error("Unsupported platform".to_string()))
    }
}
