#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use serde::Serialize;

/// Permission status for the application
#[derive(Debug, Clone, Serialize)]
pub struct PermissionStatus {
    /// Whether accessibility permission is granted (required for global hotkeys on macOS)
    pub accessibility: bool,
    /// Microphone permission state (required for audio recording)
    pub microphone: PermissionState,
}

/// State of a permission
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum PermissionState {
    /// Permission has been granted
    Granted,
    /// Permission has been explicitly denied
    Denied,
    /// Permission has not been requested yet
    NotDetermined,
    /// Permission is restricted by system policy
    Restricted,
    /// Permission is not applicable on this platform
    NotApplicable,
}

/// Check all permissions
pub fn check_permissions() -> PermissionStatus {
    #[cfg(target_os = "macos")]
    {
        macos::check_permissions()
    }
    #[cfg(target_os = "windows")]
    {
        windows::check_permissions()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        PermissionStatus {
            accessibility: true,
            microphone: PermissionState::NotApplicable,
        }
    }
}

/// Request microphone permission (triggers system dialog on macOS)
pub async fn request_microphone() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::request_microphone().await
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Open accessibility settings
pub fn open_accessibility_settings() {
    #[cfg(target_os = "macos")]
    {
        macos::open_accessibility_settings();
    }
}

/// Open microphone settings
pub fn open_microphone_settings() {
    #[cfg(target_os = "macos")]
    {
        macos::open_microphone_settings();
    }
}
