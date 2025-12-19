use arboard::Clipboard;

#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    #[error("Clipboard error: {0}")]
    Clipboard(String),
}

/// 获取剪贴板内容
pub fn get_clipboard_text() -> Result<Option<String>, ClipboardError> {
    let mut clipboard = Clipboard::new().map_err(|e| ClipboardError::Clipboard(e.to_string()))?;

    match clipboard.get_text() {
        Ok(text) => Ok(Some(text)),
        Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(e) => Err(ClipboardError::Clipboard(e.to_string())),
    }
}

/// 设置剪贴板内容
pub fn set_clipboard_text(text: &str) -> Result<(), ClipboardError> {
    let mut clipboard = Clipboard::new().map_err(|e| ClipboardError::Clipboard(e.to_string()))?;

    clipboard
        .set_text(text)
        .map_err(|e| ClipboardError::Clipboard(e.to_string()))?;

    Ok(())
}
