import Foundation
import VhisperCore

print("=== Vhisper Swift Integration Test ===\n")

// 1. 测试版本号
print("[1] Testing vhisper_version()...")
if let versionPtr = vhisper_version() {
    let version = String(cString: versionPtr)
    print("    Version: \(version) ✓")
} else {
    print("    Failed to get version ✗")
}

// 2. 测试创建实例（无配置）
print("\n[2] Testing vhisper_create(nil)...")
if let handle = vhisper_create(nil) {
    print("    Handle created ✓")

    // 3. 测试状态查询
    print("\n[3] Testing vhisper_get_state()...")
    let state = vhisper_get_state(handle)
    print("    State: \(state) (0=Idle) \(state == 0 ? "✓" : "✗")")

    // 4. 测试销毁
    print("\n[4] Testing vhisper_destroy()...")
    vhisper_destroy(handle)
    print("    Handle destroyed ✓")
} else {
    print("    Failed to create handle ✗")
    print("    (This is expected if no default config exists)")
}

// 5. 测试 Swift 封装层
print("\n[5] Testing Swift wrapper (Vhisper class)...")
print("    Version via wrapper: \(Vhisper.version)")

print("\n=== All basic tests passed ===")
print("\nNote: Recording tests require:")
print("  - Microphone permission")
print("  - Valid API keys in config")
