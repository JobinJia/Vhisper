use super::{PermissionState, PermissionStatus};
use std::process::Command;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
}

/// Check if accessibility permission is granted
pub fn check_accessibility() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Request accessibility permission - shows system dialog
pub fn request_accessibility_with_prompt() {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    unsafe {
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let value = CFBoolean::true_value();
        let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as *const _);
    }
}

/// Check microphone permission by trying to access audio device
pub fn check_microphone() -> PermissionState {
    use cpal::traits::HostTrait;

    let host = cpal::default_host();
    match host.default_input_device() {
        Some(_) => PermissionState::Granted,
        None => PermissionState::Denied,
    }
}

/// Check all permissions
pub fn check_permissions() -> PermissionStatus {
    PermissionStatus {
        accessibility: check_accessibility(),
        microphone: check_microphone(),
    }
}

/// Request microphone permission - opens settings
pub async fn request_microphone() -> bool {
    if check_microphone() == PermissionState::Granted {
        return true;
    }
    open_microphone_settings();
    false
}

/// Open System Settings > Privacy & Security > Accessibility
pub fn open_accessibility_settings() {
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

/// Open System Settings > Privacy & Security > Microphone
pub fn open_microphone_settings() {
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
        .spawn();
}
