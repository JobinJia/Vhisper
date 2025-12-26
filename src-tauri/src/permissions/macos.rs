use super::{PermissionState, PermissionStatus};
use std::process::Command;

// Link against ApplicationServices framework for AXIsProcessTrusted
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

// Link against AVFoundation framework
#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

// AVAuthorizationStatus values
const AV_AUTH_STATUS_NOT_DETERMINED: i64 = 0;
const AV_AUTH_STATUS_RESTRICTED: i64 = 1;
const AV_AUTH_STATUS_DENIED: i64 = 2;
const AV_AUTH_STATUS_AUTHORIZED: i64 = 3;

/// Check if accessibility permission is granted
pub fn check_accessibility() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Check microphone permission status using Objective-C runtime
pub fn check_microphone() -> PermissionState {
    use objc2::runtime::AnyClass;
    use objc2_foundation::NSString;

    unsafe {
        // Get AVCaptureDevice class
        let class = AnyClass::get("AVCaptureDevice");

        if let Some(cls) = class {
            // Create "soun" string for AVMediaTypeAudio
            let media_type = NSString::from_str("soun");

            // Call authorizationStatusForMediaType: directly
            let status: i64 = objc2::msg_send![cls, authorizationStatusForMediaType: &*media_type];

            match status {
                AV_AUTH_STATUS_AUTHORIZED => PermissionState::Granted,
                AV_AUTH_STATUS_DENIED => PermissionState::Denied,
                AV_AUTH_STATUS_NOT_DETERMINED => PermissionState::NotDetermined,
                AV_AUTH_STATUS_RESTRICTED => PermissionState::Restricted,
                _ => PermissionState::NotDetermined,
            }
        } else {
            // AVCaptureDevice class not found, assume not applicable
            PermissionState::NotApplicable
        }
    }
}

/// Check all permissions
pub fn check_permissions() -> PermissionStatus {
    PermissionStatus {
        accessibility: check_accessibility(),
        microphone: check_microphone(),
    }
}

/// Request microphone permission
/// This triggers the system permission dialog
pub async fn request_microphone() -> bool {
    // Use osascript to trigger the permission dialog via a simple audio recording attempt
    // This is a workaround since directly calling AVCaptureDevice.requestAccess requires complex block handling

    // First check if already granted
    if check_microphone() == PermissionState::Granted {
        return true;
    }

    // Try to access the microphone which will trigger the permission dialog
    // We use a simple cpal device query which should trigger the permission prompt
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    if let Some(device) = host.default_input_device() {
        // Just getting the device info should trigger the permission dialog
        let _ = device.default_input_config();

        // Wait a bit for the user to respond
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Check the result
        return check_microphone() == PermissionState::Granted;
    }

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
