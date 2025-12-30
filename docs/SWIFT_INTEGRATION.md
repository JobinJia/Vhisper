# Vhisper Swift 集成设计文档

## 1. 项目概述

将 Vhisper Rust Core 库集成到 macOS 原生 Swift 应用，实现语音输入功能。

### 1.1 职责划分

| 层 | 职责 | 技术栈 |
|---|------|-------|
| **Rust Core** | 录音、ASR 识别、LLM 优化 | cpal, reqwest, tokio |
| **FFI 层** | C ABI 接口 | #[no_mangle] extern "C" |
| **Swift 层** | 权限、热键、输出、UI | SwiftUI, CGEvent, NSPasteboard |

### 1.2 Core 不负责的事项

- 麦克风权限申请（TCC 限制，必须由宿主 App 触发）
- 全局热键监听
- 文本输出到光标位置
- UI 展示

---

## 2. FFI 接口设计

### 2.1 接口定义

```c
// 生命周期
VhisperHandle* vhisper_create(const char* config_json);
void vhisper_destroy(VhisperHandle* handle);

// 状态查询
int32_t vhisper_get_state(VhisperHandle* handle);
// 返回: 0=Idle, 1=Recording, 2=Processing, -1=handle无效

// 录音控制
int32_t vhisper_start_recording(VhisperHandle* handle);
int32_t vhisper_stop_recording(
    VhisperHandle* handle,
    VhisperResultCallback callback,
    void* context
);
int32_t vhisper_cancel(VhisperHandle* handle);

// 配置更新
int32_t vhisper_update_config(VhisperHandle* handle, const char* config_json);

// 内存管理
void vhisper_string_free(char* s);
const char* vhisper_version(void);

// 回调类型
typedef void (*VhisperResultCallback)(
    void* context,
    const char* text,   // 成功时非 NULL
    const char* error   // 失败时非 NULL，包括 "Operation cancelled"
);
```

### 2.2 错误码

| 返回值 | 含义 |
|-------|------|
| 0 | 成功 |
| -1 | handle 无效 |
| -2 | 操作失败（Pipeline 忙、参数错误等） |

### 2.3 幂等性保证

| 函数 | 幂等性 | 说明 |
|-----|--------|-----|
| `vhisper_start_recording` | 否 | 非 Idle 状态返回 -2 |
| `vhisper_stop_recording` | 是 | 非 Recording 状态返回空字符串 |
| `vhisper_cancel` | 是 | 任意状态可调用 |
| `vhisper_get_state` | 是 | 只读操作 |

### 2.4 内存约定

- `vhisper_create` 返回的 handle 由调用方持有，需调用 `vhisper_destroy` 释放
- 回调中的字符串指针仅在回调期间有效，Swift 侧需立即复制
- `vhisper_version` 返回静态字符串，无需释放

### 2.5 线程模型

- `vhisper_start_recording`: 同步调用，立即返回
- `vhisper_stop_recording`: 异步调用，立即返回，结果通过回调通知
- `vhisper_cancel`: 同步调用，立即返回
- `vhisper_get_state`: 同步调用，原子读取
- 回调在 Rust tokio 线程执行，Swift 侧需 dispatch 到主线程
- **重要**：回调中不可直接操作 UI，必须切换到主线程

---

## 3. Swift 应用架构

### 3.1 应用形态

- **菜单栏应用**：无 Dock 图标，菜单栏显示状态
- **悬浮窗**：录音时显示波形/计时，处理时显示进度
- **设置窗口**：配置 ASR/LLM/热键

### 3.2 交互模式

采用 **PTT (Push-To-Talk)** 模式：

```
按下热键 → 开始录音 → 松开热键 → 处理 → 输出文本
```

### 3.3 状态机

```
                            cancel()
         ┌──────────────────────────────────────────┐
         │                                          │
         ▼                cancel()                  │
     ┌───────┐  按下热键   ┌───────────┐────────────┤
     │ Idle  │ ─────────▶ │ Recording │            │
     └───────┘            └───────────┘            │
         ▲                      │                  │
         │                 松开热键                │
         │                      ▼                  │
         │               ┌────────────┐  cancel()  │
         │               │ Processing │────────────┘
         │               └────────────┘
         │                      │
         │          ┌───────────┴───────────┐
         │          ▼                       ▼
     ┌───────────────────┐         ┌─────────────┐
     │ Completed(text)   │         │Error/Cancel │
     └───────────────────┘         └─────────────┘
         │                              │
         │ 1.5s 后自动                   │
         └──────────────────────────────┘
```

**状态转换规则**：
- `Idle → Recording`: 仅当 `start_recording()` 成功
- `Recording → Processing`: 调用 `stop_recording()`
- `Recording → Idle`: 调用 `cancel()`
- `Processing → Idle`: 处理完成或取消
- 任意状态可查询 `get_state()`

### 3.4 模块划分

```
VhisperSwift/
├── App/
│   └── VhisperApp.swift          # SwiftUI App 入口
├── Core/
│   ├── VhisperCore.swift         # FFI 封装类
│   └── VhisperConfig.swift       # 配置模型 (Codable)
├── Managers/
│   ├── VoiceInputManager.swift   # 主状态管理 (@MainActor)
│   ├── HotkeyMonitor.swift       # 全局热键 (CGEvent)
│   ├── TextInjector.swift        # 文本输出 (NSPasteboard + CGEvent)
│   └── PermissionManager.swift   # 权限管理 (AVCaptureDevice, AX)
├── Views/
│   ├── MenuBarView.swift         # 菜单栏下拉菜单
│   ├── FloatingPanelView.swift   # 录音状态悬浮窗
│   ├── SettingsView.swift        # 设置界面
│   └── Components/
│       └── WaveformView.swift    # 波形动画组件
└── Resources/
    └── VhisperCore.xcframework   # Rust 编译产物
```

---

## 4. 关键流程

### 4.1 启动流程

```
1. App 启动
2. 加载配置 (UserDefaults / JSON 文件)
3. 检查权限状态 (不主动请求，只检查)
4. 初始化 VhisperCore (FFI)
5. 注册全局热键
6. 显示菜单栏图标
```

### 4.2 录音流程

```
1. 用户按下热键
2. 检查麦克风权限
   - 未授权：弹出系统权限请求
   - 已拒绝：显示错误，引导到系统设置
3. 调用 vhisper_start_recording()
4. 显示悬浮窗 (波形 + 计时)
5. 菜单栏图标变红
6. 用户松开热键
7. 调用 vhisper_stop_recording(callback)
8. 悬浮窗显示 "处理中..."
9. 等待回调
10. 回调返回：
    - 成功：调用 TextInjector 输出文本
    - 失败：显示错误信息
11. 延迟 1.5s 后隐藏悬浮窗
12. 恢复 Idle 状态
```

### 4.3 文本输出流程

```
1. 保存当前剪贴板内容
2. 将识别文本写入剪贴板
3. 模拟 Cmd+V 粘贴
4. 等待 500ms
5. 恢复原剪贴板内容 (可选，根据配置)
```

### 4.4 粘贴失败回退

某些应用（安全文本框、禁止模拟事件的应用）可能阻止粘贴。回退方案：

| 失败场景 | 回退策略 |
|---------|---------|
| 模拟按键被阻止 | 保留文本在剪贴板，显示通知"已复制到剪贴板" |
| 安全输入框 | 显示浮窗，提供"点击复制"按钮 |
| 连续失败 | 自动切换到"仅复制"模式 |

---

## 5. 权限要求

### 5.1 麦克风权限

- **Info.plist**: `NSMicrophoneUsageDescription`
- **请求时机**: 首次录音时
- **API**: `AVCaptureDevice.requestAccess(for: .audio)`

### 5.2 辅助功能权限

- **用途**: 全局热键监听 + 模拟按键
- **请求时机**: 首次注册热键时
- **API**: `AXIsProcessTrustedWithOptions`

---

## 6. 构建配置

### 6.1 Rust 侧

```toml
# Cargo.toml
[lib]
crate-type = ["lib", "staticlib", "cdylib"]
```

构建命令：
```bash
# 编译两个架构
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# 生成头文件
cbindgen --config cbindgen.toml --output include/vhisper.h

# 打包 xcframework
xcodebuild -create-xcframework \
  -library target/aarch64-apple-darwin/release/libvhisper_core.a \
  -headers include/ \
  -library target/x86_64-apple-darwin/release/libvhisper_core.a \
  -headers include/ \
  -output VhisperCore.xcframework
```

### 6.2 Swift 侧

- 引入 `VhisperCore.xcframework`
- Bridging Header 引入 `vhisper.h`
- Link: `libvhisper_core.a` + 系统框架 (CoreAudio, Security, etc.)

---

## 7. 配置模型

### 7.1 配置结构

```json
{
  "asr": {
    "provider": "dashscope",
    "model": "paraformer-realtime-v2",
    "base_url": null
  },
  "llm": {
    "enabled": false,
    "provider": "openai",
    "model": "gpt-4o-mini",
    "prompt": "优化以下语音识别文本，修正错别字和标点..."
  },
  "hotkey": {
    "key_code": 58,
    "modifiers": ["option"]
  },
  "output": {
    "restore_clipboard": true,
    "paste_delay_ms": 100
  }
}
```

### 7.2 密钥存储（Keychain）

**重要**：API Key 不存储在配置文件中，使用 macOS Keychain 安全存储。

```swift
// 存储格式
// Service: "com.vhisper.credentials"
// Account: "{provider}_api_key" (e.g., "dashscope_api_key", "openai_api_key")

// 读取示例
let query: [String: Any] = [
    kSecClass as String: kSecClassGenericPassword,
    kSecAttrService as String: "com.vhisper.credentials",
    kSecAttrAccount as String: "dashscope_api_key",
    kSecReturnData as String: true
]
```

### 7.3 配置传递给 Core

Swift 侧构建完整 JSON（包含从 Keychain 读取的密钥）后传递给 `vhisper_create` 或 `vhisper_update_config`。

---

## 8. UI 设计

### 8.1 菜单栏图标

| 状态 | 图标 | 颜色 |
|-----|------|-----|
| Idle | mic | 默认 |
| Recording | mic.fill | 红色 |
| Processing | ellipsis.circle | 默认 |
| Completed | checkmark.circle | 绿色 |
| Error | exclamationmark.triangle | 黄色 |

### 8.2 悬浮窗

- 尺寸: 200 x 80 pt
- 位置: 屏幕右上角
- 样式: `.ultraThinMaterial` 毛玻璃
- 内容:
  - Recording: 波形动画 + 计时器
  - Processing: 加载动画 + "识别中..."
  - Completed: "✓ 已输出"
  - Error: 错误图标 + 错误信息

### 8.3 设置窗口

- Tab 1: ASR 配置 (提供商选择、API Key、模型)
- Tab 2: LLM 配置 (开关、提供商、Prompt)
- Tab 3: 热键配置 (按键录入)
- Tab 4: 输出配置 (恢复剪贴板、延迟)
- Tab 5: 关于 (版本、许可证)

---

## 9. 错误处理

### 9.1 错误分类

| 类型 | 示例 | 处理方式 |
|-----|------|---------|
| 权限错误 | 麦克风未授权 | 显示引导，跳转系统设置 |
| 网络错误 | API 请求失败 | 显示重试提示 |
| 配置错误 | API Key 无效 | 显示错误，引导到设置 |
| Core 错误 | 录音设备不可用 | 显示错误信息 |

### 9.2 用户反馈

- 所有错误在悬浮窗显示 1.5s
- 严重错误同时发送系统通知
- 错误日志写入 `~/Library/Logs/Vhisper/`

---

## 10. 待定事项

- [ ] 是否支持实时 (流式) 识别？
- [ ] 是否需要录音历史记录？
- [ ] 是否需要多语言 UI？
- [ ] 是否需要自动更新机制 (Sparkle)？
- [ ] 最大录音时长限制？（建议 60s，防止误操作）
- [ ] 是否需要录音波形可视化？

---

## 11. 参考

- [cpal - Rust 跨平台音频库](https://github.com/RustAudio/cpal)
- [cbindgen - Rust C 绑定生成器](https://github.com/mozilla/cbindgen)
- [Swift 与 C 互操作](https://developer.apple.com/documentation/swift/c-interoperability)
- [macOS TCC 权限模型](https://developer.apple.com/documentation/avfoundation/capture_setup/requesting_authorization_to_capture_and_save_media)
