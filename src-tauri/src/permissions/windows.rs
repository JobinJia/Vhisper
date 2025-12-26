use super::{PermissionState, PermissionStatus};

/// Check all permissions on Windows
/// Windows doesn't require explicit accessibility permissions like macOS
/// Microphone permission is typically handled by the audio library
pub fn check_permissions() -> PermissionStatus {
    PermissionStatus {
        // Windows doesn't require accessibility permission for global hotkeys
        accessibility: true,
        // Windows microphone permission is handled differently
        // For now, we assume it's available (cpal will fail if not)
        microphone: PermissionState::NotApplicable,
    }
}
