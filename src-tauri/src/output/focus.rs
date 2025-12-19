//! 应用焦点管理模块 (macOS)

#[cfg(target_os = "macos")]
use objc2_app_kit::NSWorkspace;

/// 获取当前活跃应用的进程 ID
#[cfg(target_os = "macos")]
pub fn get_frontmost_app_pid() -> Option<i32> {
    // 使用 catch_unwind 防止 objc 调用导致的 panic
    std::panic::catch_unwind(|| {
        unsafe {
            let workspace = NSWorkspace::sharedWorkspace();
            let app = workspace.frontmostApplication()?;
            Some(app.processIdentifier())
        }
    })
    .ok()
    .flatten()
}

/// Windows 占位实现
#[cfg(target_os = "windows")]
pub fn get_frontmost_app_pid() -> Option<i32> {
    // TODO: Windows 实现
    None
}

/// 其他平台占位实现
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn get_frontmost_app_pid() -> Option<i32> {
    None
}
