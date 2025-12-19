use core_graphics::event::{CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapPlacement, CGEventTapOptions, CGEventType};
use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use crate::get_pipeline;
use crate::output::get_frontmost_app_pid;

#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    #[error("Failed to create event tap")]
    EventTapCreation,
    #[error("Failed to enable event tap")]
    EventTapEnable,
}

/// 启动 macOS 快捷键监听
pub fn start_listener(app_handle: AppHandle) -> Result<(), HotkeyError> {
    let is_alt_pressed = Arc::new(AtomicBool::new(false));
    let is_recording = Arc::new(AtomicBool::new(false));
    // 存储录音开始时的活跃应用 PID，-1 表示无
    let original_app_pid = Arc::new(AtomicI32::new(-1));

    let is_alt_pressed_clone = is_alt_pressed.clone();
    let is_recording_clone = is_recording.clone();
    let original_app_pid_clone = original_app_pid.clone();

    let callback = move |_proxy, event_type, event: &core_graphics::event::CGEvent| {
        let flags = event.get_flags();
        let alt_pressed = flags.contains(CGEventFlags::CGEventFlagAlternate);

        match event_type {
            CGEventType::FlagsChanged => {
                let was_pressed = is_alt_pressed_clone.load(Ordering::SeqCst);

                if alt_pressed && !was_pressed {
                    // Alt 按下
                    is_alt_pressed_clone.store(true, Ordering::SeqCst);

                    if !is_recording_clone.load(Ordering::SeqCst) {
                        is_recording_clone.store(true, Ordering::SeqCst);

                        // 记录当前活跃应用的 PID
                        let pid = get_frontmost_app_pid().unwrap_or(-1);
                        original_app_pid_clone.store(pid, Ordering::SeqCst);
                        tracing::info!("Alt pressed - starting recording (app pid: {})", pid);

                        let app_handle = app_handle.clone();
                        std::thread::spawn(move || {
                            start_recording(&app_handle);
                        });
                    }
                } else if !alt_pressed && was_pressed {
                    // Alt 释放
                    is_alt_pressed_clone.store(false, Ordering::SeqCst);

                    if is_recording_clone.load(Ordering::SeqCst) {
                        is_recording_clone.store(false, Ordering::SeqCst);
                        let pid = original_app_pid_clone.load(Ordering::SeqCst);
                        tracing::info!("Alt released - stopping recording");

                        let app_handle = app_handle.clone();
                        std::thread::spawn(move || {
                            stop_recording(&app_handle, if pid >= 0 { Some(pid) } else { None });
                        });
                    }
                }
            }
            _ => {}
        }

        // 返回 None 表示不拦截事件
        None
    };

    // 创建事件监听
    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![CGEventType::FlagsChanged],
        callback,
    )
    .map_err(|_| HotkeyError::EventTapCreation)?;

    // 启用事件监听
    tap.enable();

    // 添加到运行循环
    let loop_source = tap.mach_port
        .create_runloop_source(0)
        .map_err(|_| HotkeyError::EventTapEnable)?;

    unsafe {
        CFRunLoop::get_current().add_source(&loop_source, kCFRunLoopCommonModes);
    }

    tracing::info!("macOS hotkey listener started");

    // 运行事件循环
    CFRunLoop::run_current();

    // 如果到达这里，说明 CFRunLoop 退出了
    tracing::error!("!!! CFRunLoop exited unexpectedly !!!");

    Ok(())
}

fn start_recording(app_handle: &AppHandle) {
    // 发送事件到前端
    let _ = app_handle.emit("recording-started", ());

    // 获取 pipeline 并开始录音
    if let Some(pipeline) = get_pipeline() {
        if let Err(e) = pipeline.start_recording() {
            tracing::error!("Failed to start recording: {}", e);
            let _ = app_handle.emit("processing-error", e.to_string());
        }
    }
}

fn stop_recording(app_handle: &AppHandle, original_app_pid: Option<i32>) {
    tracing::info!("stop_recording called");

    // 发送事件到前端
    let _ = app_handle.emit("recording-stopped", ());

    // 获取 pipeline 并停止录音、处理
    if let Some(pipeline) = get_pipeline() {
        let app_handle_clone = app_handle.clone();

        // 获取 tauri async runtime 的 handle，然后在其上 spawn 任务
        // 这样即使当前线程没有 runtime 上下文也能正常工作
        tracing::info!("Spawning async task for stop_and_process");
        let handle = tauri::async_runtime::handle();
        handle.spawn(async move {
            tracing::info!("Async task started");
            match pipeline.stop_and_process(original_app_pid).await {
                Ok(text) => {
                    tracing::info!("Processing completed successfully, text: {}", text);
                    let _ = app_handle_clone.emit("processing-complete", ());
                }
                Err(e) => {
                    tracing::error!("Processing error: {}", e);
                    let _ = app_handle_clone.emit("processing-error", e.to_string());
                }
            }
            tracing::info!("Async task finished");
        });
        tracing::info!("Async task spawned");
    } else {
        tracing::warn!("Pipeline not available");
    }

    tracing::info!("stop_recording finished");
}
