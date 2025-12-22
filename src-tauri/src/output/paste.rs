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

    #[cfg(target_os = "macos")]
    {
        use core_graphics::event::{CGEvent, CGEventFlags, CGKeyCode};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

        tracing::info!("simulate_paste: using CGEvent for paste");

        // 创建事件源
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| PasteError::Paste("Failed to create CGEventSource".to_string()))?;

        // 'v' 键的虚拟键码是 9
        const KEY_V: CGKeyCode = 9;

        // 创建按下 Cmd+V 事件
        let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
            .map_err(|_| PasteError::Paste("Failed to create key down event".to_string()))?;
        key_down.set_flags(CGEventFlags::CGEventFlagCommand);

        // 创建释放 Cmd+V 事件
        let key_up = CGEvent::new_keyboard_event(source, KEY_V, false)
            .map_err(|_| PasteError::Paste("Failed to create key up event".to_string()))?;
        key_up.set_flags(CGEventFlags::CGEventFlagCommand);

        // 发送事件
        key_down.post(core_graphics::event::CGEventTapLocation::HID);
        thread::sleep(Duration::from_millis(10));
        key_up.post(core_graphics::event::CGEventTapLocation::HID);

        tracing::info!("simulate_paste: CGEvent paste successful");
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
