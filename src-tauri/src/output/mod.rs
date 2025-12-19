mod clipboard;
mod focus;
mod paste;

pub use clipboard::{get_clipboard_text, set_clipboard_text, ClipboardError};
pub use focus::get_frontmost_app_pid;
pub use paste::{simulate_paste, PasteError};

#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    #[error("Clipboard error: {0}")]
    Clipboard(#[from] ClipboardError),
    #[error("Paste error: {0}")]
    Paste(#[from] PasteError),
}

/// 输出文本到当前应用
///
/// - 如果 `original_app_pid` 与当前活跃应用相同，则执行粘贴
/// - 如果不同（用户切换了应用），则只复制到剪贴板
///
/// 参数:
/// - `text`: 要输出的文本
/// - `restore_clipboard`: 是否恢复原剪贴板内容
/// - `paste_delay_ms`: 粘贴前的延迟（毫秒）
/// - `original_app_pid`: 开始录音时的应用 PID，None 表示总是粘贴
pub fn output_text(
    text: &str,
    restore_clipboard: bool,
    paste_delay_ms: u64,
    original_app_pid: Option<i32>,
) -> Result<(), OutputError> {
    tracing::info!("output_text: starting, original_app_pid={:?}", original_app_pid);

    // 检查是否需要粘贴（用户是否还在原应用）
    let should_paste = match original_app_pid {
        Some(original_pid) => {
            tracing::info!("output_text: getting current frontmost app pid");
            let current_pid = get_frontmost_app_pid();
            tracing::info!("output_text: current_pid={:?}, original_pid={}", current_pid, original_pid);
            let same_app = current_pid == Some(original_pid);
            if !same_app {
                tracing::info!(
                    "应用已切换 (原: {}, 当前: {:?})，只复制到剪贴板",
                    original_pid,
                    current_pid
                );
            }
            same_app
        }
        None => true, // 没有原始 PID，总是粘贴
    };

    tracing::info!("output_text: should_paste={}", should_paste);

    // 保存当前剪贴板内容
    let original_clipboard = if restore_clipboard && should_paste {
        tracing::info!("output_text: getting original clipboard");
        get_clipboard_text()?
    } else {
        None
    };

    tracing::info!("output_text: setting clipboard text");
    // 设置新的剪贴板内容
    set_clipboard_text(text)?;
    tracing::info!("output_text: clipboard text set successfully");

    // 只有在同一应用时才模拟粘贴
    if should_paste {
        tracing::info!("output_text: simulating paste with delay {}ms", paste_delay_ms);
        simulate_paste(paste_delay_ms)?;
        tracing::info!("output_text: paste simulated successfully");

        // 恢复原剪贴板内容
        if restore_clipboard {
            if let Some(original) = original_clipboard {
                tracing::info!("output_text: restoring original clipboard");
                // 延迟一下再恢复，确保粘贴完成
                std::thread::sleep(std::time::Duration::from_millis(100));
                set_clipboard_text(&original)?;
                tracing::info!("output_text: original clipboard restored");
            }
        }
    }

    tracing::info!("output_text: completed successfully");
    Ok(())
}
