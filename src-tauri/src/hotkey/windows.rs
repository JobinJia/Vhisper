#[cfg(target_os = "windows")]
use windows::{
    Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_MENU},
    Win32::UI::WindowsAndMessaging::{GetMessageW, MSG},
};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::get_pipeline;

#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    #[error("Failed to start hotkey listener: {0}")]
    Start(String),
}

/// 启动 Windows 快捷键监听
#[cfg(target_os = "windows")]
pub fn start_listener(app_handle: AppHandle) -> Result<(), HotkeyError> {
    let is_alt_pressed = Arc::new(AtomicBool::new(false));
    let is_recording = Arc::new(AtomicBool::new(false));

    loop {
        // 检查 Alt 键状态
        let alt_state = unsafe { GetAsyncKeyState(VK_MENU.0 as i32) };
        let alt_pressed = (alt_state as u16 & 0x8000) != 0;

        let was_pressed = is_alt_pressed.load(Ordering::SeqCst);

        if alt_pressed && !was_pressed {
            // Alt 按下
            is_alt_pressed.store(true, Ordering::SeqCst);

            if !is_recording.load(Ordering::SeqCst) {
                is_recording.store(true, Ordering::SeqCst);
                tracing::info!("Alt pressed - starting recording");
                start_recording(&app_handle);
            }
        } else if !alt_pressed && was_pressed {
            // Alt 释放
            is_alt_pressed.store(false, Ordering::SeqCst);

            if is_recording.load(Ordering::SeqCst) {
                is_recording.store(false, Ordering::SeqCst);
                tracing::info!("Alt released - stopping recording");

                let app_handle_clone = app_handle.clone();
                thread::spawn(move || {
                    stop_recording(&app_handle_clone);
                });
            }
        }

        // 短暂休眠以减少 CPU 使用
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(not(target_os = "windows"))]
pub fn start_listener(_app_handle: AppHandle) -> Result<(), HotkeyError> {
    Err(HotkeyError::Start("Windows hotkey not supported on this platform".to_string()))
}

fn start_recording(app_handle: &AppHandle) {
    let _ = app_handle.emit("recording-started", ());

    if let Some(pipeline) = get_pipeline() {
        if let Err(e) = pipeline.start_recording() {
            tracing::error!("Failed to start recording: {}", e);
            let _ = app_handle.emit("processing-error", e.to_string());
        }
    }
}

fn stop_recording(app_handle: &AppHandle) {
    let _ = app_handle.emit("recording-stopped", ());

    if let Some(pipeline) = get_pipeline() {
        let app_handle_clone = app_handle.clone();

        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                match pipeline.stop_and_process(None).await {
                    Ok(_) => {
                        let _ = app_handle_clone.emit("processing-complete", ());
                    }
                    Err(e) => {
                        tracing::error!("Processing error: {}", e);
                        let _ = app_handle_clone.emit("processing-error", e.to_string());
                    }
                }
            });
    }
}
