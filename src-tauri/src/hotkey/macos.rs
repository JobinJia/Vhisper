use core_graphics::event::{
    CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType,
};
use core_foundation::runloop::{kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFRunLoop};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

use vhisper_core::{HotkeyBinding, KeyCode};
use crate::get_pipeline;
use crate::output::{self, get_frontmost_app_pid};
use crate::AppState;

#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    #[error("Failed to create event tap")]
    EventTapCreation,
    #[error("Failed to enable event tap")]
    EventTapEnable,
}

/// 将 KeyCode 转换为 macOS CGKeyCode (虚拟键码)
fn keycode_to_cg_keycode(key: &KeyCode) -> Option<u16> {
    match key {
        // 功能键
        KeyCode::F1 => Some(0x7A),
        KeyCode::F2 => Some(0x78),
        KeyCode::F3 => Some(0x63),
        KeyCode::F4 => Some(0x76),
        KeyCode::F5 => Some(0x60),
        KeyCode::F6 => Some(0x61),
        KeyCode::F7 => Some(0x62),
        KeyCode::F8 => Some(0x64),
        KeyCode::F9 => Some(0x65),
        KeyCode::F10 => Some(0x6D),
        KeyCode::F11 => Some(0x67),
        KeyCode::F12 => Some(0x6F),

        // 字母键 (ANSI 布局)
        KeyCode::KeyA => Some(0x00),
        KeyCode::KeyB => Some(0x0B),
        KeyCode::KeyC => Some(0x08),
        KeyCode::KeyD => Some(0x02),
        KeyCode::KeyE => Some(0x0E),
        KeyCode::KeyF => Some(0x03),
        KeyCode::KeyG => Some(0x05),
        KeyCode::KeyH => Some(0x04),
        KeyCode::KeyI => Some(0x22),
        KeyCode::KeyJ => Some(0x26),
        KeyCode::KeyK => Some(0x28),
        KeyCode::KeyL => Some(0x25),
        KeyCode::KeyM => Some(0x2E),
        KeyCode::KeyN => Some(0x2D),
        KeyCode::KeyO => Some(0x1F),
        KeyCode::KeyP => Some(0x23),
        KeyCode::KeyQ => Some(0x0C),
        KeyCode::KeyR => Some(0x0F),
        KeyCode::KeyS => Some(0x01),
        KeyCode::KeyT => Some(0x11),
        KeyCode::KeyU => Some(0x20),
        KeyCode::KeyV => Some(0x09),
        KeyCode::KeyW => Some(0x0D),
        KeyCode::KeyX => Some(0x07),
        KeyCode::KeyY => Some(0x10),
        KeyCode::KeyZ => Some(0x06),

        // 数字键
        KeyCode::Digit0 => Some(0x1D),
        KeyCode::Digit1 => Some(0x12),
        KeyCode::Digit2 => Some(0x13),
        KeyCode::Digit3 => Some(0x14),
        KeyCode::Digit4 => Some(0x15),
        KeyCode::Digit5 => Some(0x17),
        KeyCode::Digit6 => Some(0x16),
        KeyCode::Digit7 => Some(0x1A),
        KeyCode::Digit8 => Some(0x1C),
        KeyCode::Digit9 => Some(0x19),

        // 特殊键
        KeyCode::Space => Some(0x31),
        KeyCode::Tab => Some(0x30),
        KeyCode::CapsLock => Some(0x39),
        KeyCode::Escape => Some(0x35),
        KeyCode::Backquote => Some(0x32),

        // 修饰键不需要 CGKeyCode (通过 flags 检测)
        KeyCode::Alt | KeyCode::Control | KeyCode::Shift | KeyCode::Meta => None,
    }
}

/// 将 KeyCode 转换为 CGEventFlags
fn keycode_to_cg_flag(key: &KeyCode) -> Option<CGEventFlags> {
    match key {
        KeyCode::Alt => Some(CGEventFlags::CGEventFlagAlternate),
        KeyCode::Control => Some(CGEventFlags::CGEventFlagControl),
        KeyCode::Shift => Some(CGEventFlags::CGEventFlagShift),
        KeyCode::Meta => Some(CGEventFlags::CGEventFlagCommand),
        _ => None,
    }
}

/// 检查修饰键是否匹配
fn check_modifiers(flags: CGEventFlags, required: &[KeyCode]) -> bool {
    for modifier in required {
        if let Some(flag) = keycode_to_cg_flag(modifier) {
            if !flags.contains(flag) {
                return false;
            }
        }
    }
    true
}

/// 启动 macOS 快捷键监听
pub fn start_listener(
    app_handle: AppHandle,
    binding: HotkeyBinding,
    stop_rx: Receiver<()>,
) -> Result<(), HotkeyError> {
    let is_key_pressed = Arc::new(AtomicBool::new(false));
    let is_recording = Arc::new(AtomicBool::new(false));
    let original_app_pid = Arc::new(AtomicI32::new(-1));

    // 判断主键类型
    let is_modifier_key = binding.key.is_modifier();
    let binding_clone = binding.clone();

    let is_key_pressed_clone = is_key_pressed.clone();
    let is_recording_clone = is_recording.clone();
    let original_app_pid_clone = original_app_pid.clone();

    // 获取主键的 flag (如果是修饰键)
    let main_key_flag = keycode_to_cg_flag(&binding.key);
    // 获取主键的 keycode (如果是普通键)
    let main_key_code = keycode_to_cg_keycode(&binding.key);

    tracing::info!(
        "Starting hotkey listener for: {:?} (modifier: {}, keycode: {:?}, flag: {:?})",
        binding,
        is_modifier_key,
        main_key_code,
        main_key_flag
    );

    let callback = move |_proxy, event_type, event: &core_graphics::event::CGEvent| {
        let flags = event.get_flags();

        match event_type {
            CGEventType::FlagsChanged => {
                // 根据主键类型检测按键状态
                let key_pressed = if is_modifier_key {
                    // 修饰键作为主键
                    if let Some(flag) = main_key_flag {
                        flags.contains(flag) && check_modifiers(flags, &binding_clone.modifiers)
                    } else {
                        false
                    }
                } else if matches!(binding_clone.key, KeyCode::CapsLock) {
                    // CapsLock 特殊处理
                    flags.contains(CGEventFlags::CGEventFlagAlphaShift)
                        && check_modifiers(flags, &binding_clone.modifiers)
                } else {
                    // 其他键不通过 FlagsChanged 处理
                    return None;
                };

                handle_key_state_change(
                    key_pressed,
                    &is_key_pressed_clone,
                    &is_recording_clone,
                    &original_app_pid_clone,
                    &app_handle,
                );
            }

            CGEventType::KeyDown => {
                if is_modifier_key {
                    return None;
                }
                // 普通键作为主键：检查按下
                // CGEventField 9 = kCGKeyboardEventKeycode
                let key_code = event.get_integer_value_field(9) as u16;

                if let Some(expected_keycode) = main_key_code {
                    if key_code == expected_keycode
                        && check_modifiers(flags, &binding_clone.modifiers)
                    {
                        handle_key_state_change(
                            true,
                            &is_key_pressed_clone,
                            &is_recording_clone,
                            &original_app_pid_clone,
                            &app_handle,
                        );
                    }
                }
            }

            CGEventType::KeyUp => {
                if is_modifier_key {
                    return None;
                }
                // 普通键作为主键：检查释放
                // CGEventField 9 = kCGKeyboardEventKeycode
                let key_code = event.get_integer_value_field(9) as u16;

                if let Some(expected_keycode) = main_key_code {
                    if key_code == expected_keycode {
                        handle_key_state_change(
                            false,
                            &is_key_pressed_clone,
                            &is_recording_clone,
                            &original_app_pid_clone,
                            &app_handle,
                        );
                    }
                }
            }

            _ => {}
        }

        // 返回 None 表示不拦截事件
        None
    };

    // 订阅的事件类型取决于主键类型
    let event_types = if is_modifier_key {
        vec![CGEventType::FlagsChanged]
    } else if matches!(binding.key, KeyCode::CapsLock) {
        // CapsLock 通过 FlagsChanged 检测
        vec![CGEventType::FlagsChanged]
    } else {
        vec![
            CGEventType::FlagsChanged,
            CGEventType::KeyDown,
            CGEventType::KeyUp,
        ]
    };

    tracing::info!("Subscribing to event types: {:?}", event_types);

    // 创建事件监听
    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        event_types,
        callback,
    )
    .map_err(|_| HotkeyError::EventTapCreation)?;

    // 启用事件监听
    tap.enable();

    // 添加到运行循环
    let loop_source = tap
        .mach_port
        .create_runloop_source(0)
        .map_err(|_| HotkeyError::EventTapEnable)?;

    let run_loop = CFRunLoop::get_current();

    unsafe {
        run_loop.add_source(&loop_source, kCFRunLoopCommonModes);
    }

    tracing::info!("macOS hotkey listener started");

    // 使用带超时的运行循环，定期检查停止信号
    loop {
        // 检查是否收到停止信号
        match stop_rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => {
                tracing::info!("Stop signal received, stopping CFRunLoop...");
                break;
            }
            Err(TryRecvError::Empty) => {}
        }

        // 运行事件循环 100ms，然后检查停止信号
        unsafe {
            core_foundation::runloop::CFRunLoopRunInMode(
                kCFRunLoopDefaultMode,
                0.1, // 100ms 超时
                false as u8,
            );
        }
    }

    tracing::info!("macOS hotkey listener stopped");

    Ok(())
}

fn handle_key_state_change(
    key_pressed: bool,
    is_key_pressed: &AtomicBool,
    is_recording: &AtomicBool,
    original_app_pid: &AtomicI32,
    app_handle: &AppHandle,
) {
    let was_pressed = is_key_pressed.load(Ordering::SeqCst);

    if key_pressed && !was_pressed {
        // 按键按下
        is_key_pressed.store(true, Ordering::SeqCst);

        if !is_recording.load(Ordering::SeqCst) {
            is_recording.store(true, Ordering::SeqCst);

            // 记录当前活跃应用的 PID
            let pid = get_frontmost_app_pid().unwrap_or(-1);
            original_app_pid.store(pid, Ordering::SeqCst);
            tracing::info!("Hotkey pressed - starting recording (app pid: {})", pid);

            let app_handle = app_handle.clone();
            std::thread::spawn(move || {
                start_recording(&app_handle);
            });
        }
    } else if !key_pressed && was_pressed {
        // 按键释放
        is_key_pressed.store(false, Ordering::SeqCst);

        if is_recording.load(Ordering::SeqCst) {
            is_recording.store(false, Ordering::SeqCst);
            let pid = original_app_pid.load(Ordering::SeqCst);
            tracing::info!("Hotkey released - stopping recording");

            let app_handle = app_handle.clone();
            std::thread::spawn(move || {
                stop_recording(&app_handle, if pid >= 0 { Some(pid) } else { None });
            });
        }
    }
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

        // 获取配置
        let state = app_handle.state::<AppState>();
        let config = state.config.clone();

        // 获取 tauri async runtime 的 handle，然后在其上 spawn 任务
        tracing::info!("Spawning async task for stop_and_process");
        let handle = tauri::async_runtime::handle();
        handle.spawn(async move {
            tracing::info!("Async task started");
            match pipeline.stop_and_process().await {
                Ok(text) => {
                    tracing::info!("Processing completed successfully, text: {}", text);

                    // 输出文本到当前应用
                    if !text.is_empty() {
                        let cfg = config.read().await;
                        if let Err(e) = output::output_text(
                            &text,
                            cfg.output.restore_clipboard,
                            cfg.output.paste_delay_ms,
                            original_app_pid,
                        ) {
                            tracing::error!("Text output failed: {}", e);
                        }
                    }

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
