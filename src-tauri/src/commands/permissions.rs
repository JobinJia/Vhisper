use crate::permissions::{self, PermissionStatus};

/// Check all system permissions
#[tauri::command]
pub fn check_permissions() -> PermissionStatus {
    permissions::check_permissions()
}

/// Request microphone permission (triggers system dialog on macOS)
#[tauri::command]
pub async fn request_microphone_permission() -> bool {
    permissions::request_microphone().await
}

/// Open accessibility settings in System Preferences
#[tauri::command]
pub fn open_accessibility_settings() {
    permissions::open_accessibility_settings();
}

/// Request accessibility permission (triggers system dialog on macOS)
#[tauri::command]
pub fn request_accessibility_permission() {
    permissions::request_accessibility();
}

/// Open microphone settings in System Preferences
#[tauri::command]
pub fn open_microphone_settings() {
    permissions::open_microphone_settings();
}
