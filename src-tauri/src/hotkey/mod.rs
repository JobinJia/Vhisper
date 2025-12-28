#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

use std::sync::OnceLock;
use tauri::AppHandle;
use tokio::sync::mpsc;

use vhisper_core::HotkeyBinding;

#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    #[error("Hotkey error: {0}")]
    Error(String),
}

/// 全局的配置更新发送器
static CONFIG_SENDER: OnceLock<mpsc::UnboundedSender<HotkeyBinding>> = OnceLock::new();

/// 请求重新加载快捷键配置
pub fn reload_hotkey(binding: HotkeyBinding) {
    if let Some(sender) = CONFIG_SENDER.get() {
        tracing::info!("Hotkey reload requested: {:?}", binding);
        let _ = sender.send(binding);
    }
}

/// 启动快捷键监听（带热重载支持）
pub fn start_listener(app_handle: AppHandle, initial_binding: HotkeyBinding) -> Result<(), HotkeyError> {
    // 创建配置更新 channel
    let (tx, mut rx) = mpsc::unbounded_channel::<HotkeyBinding>();
    let _ = CONFIG_SENDER.set(tx);

    let mut current_binding = initial_binding;

    loop {
        tracing::info!("Starting hotkey listener with binding: {:?}", current_binding);

        #[cfg(target_os = "macos")]
        {
            // macOS: 启动监听器，它会在收到停止信号时返回
            let binding_clone = current_binding.clone();
            let app_handle_clone = app_handle.clone();

            // 在单独线程中运行监听器
            let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();

            let listener_handle = std::thread::spawn(move || {
                macos::start_listener(app_handle_clone, binding_clone, stop_rx)
            });

            // 等待新配置
            if let Some(new_binding) = rx.blocking_recv() {
                tracing::info!("Received new hotkey binding: {:?}", new_binding);
                current_binding = new_binding;
                // 发送停止信号
                let _ = stop_tx.send(());
                // 等待监听器线程结束
                let _ = listener_handle.join();
                tracing::info!("Previous listener stopped, restarting...");
            } else {
                // Channel 关闭，退出
                break;
            }
        }

        #[cfg(target_os = "windows")]
        {
            let binding_clone = current_binding.clone();
            let app_handle_clone = app_handle.clone();

            let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();

            let listener_handle = std::thread::spawn(move || {
                windows::start_listener(app_handle_clone, binding_clone, stop_rx)
            });

            if let Some(new_binding) = rx.blocking_recv() {
                tracing::info!("Received new hotkey binding: {:?}", new_binding);
                current_binding = new_binding;
                let _ = stop_tx.send(());
                let _ = listener_handle.join();
                tracing::info!("Previous listener stopped, restarting...");
            } else {
                break;
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            return Err(HotkeyError::Error("Unsupported platform".to_string()));
        }
    }

    Ok(())
}
