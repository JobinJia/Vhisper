//
//  vhisperApp.swift
//  vhisper
//
//  Menu Bar 语音输入应用
//

import SwiftUI
import AVFoundation
import Combine
import Carbon.HIToolbox
import ApplicationServices

// MARK: - Array Extension

extension Array {
    func chunked(into size: Int) -> [[Element]] {
        stride(from: 0, to: count, by: size).map {
            Array(self[$0..<Swift.min($0 + size, count)])
        }
    }
}

@main
struct VhisperApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate

    var body: some Scene {
        Settings {
            SettingsView()
        }
    }
}

// MARK: - App Delegate

class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem?
    private var popover: NSPopover?
    private var hotkeyManager: HotkeyManager?

    func applicationDidFinishLaunching(_ notification: Notification) {
        // 隐藏 Dock 图标
        NSApp.setActivationPolicy(.accessory)

        // 创建菜单栏图标
        setupStatusItem()

        // 初始化热键
        hotkeyManager = HotkeyManager.shared
        hotkeyManager?.register()

        // 请求麦克风权限
        requestMicrophonePermission()

        // 初始化 Vhisper（从保存的配置加载）
        initializeVhisper()
    }

    private func initializeVhisper() {
        // 从 UserDefaults 读取配置
        var asrProvider = UserDefaults.standard.string(forKey: "vhisper.asr.provider") ?? "Qwen"
        let asrApiKey = UserDefaults.standard.string(forKey: "vhisper.asr.apiKey") ?? ""

        // 迁移旧配置格式
        asrProvider = migrateProvider(asrProvider)

        guard !asrApiKey.isEmpty else {
            print("⚠️ 未配置 API Key，请在设置中配置")
            return
        }

        // 构建配置 JSON（Rust 期望特定格式）
        let config = buildConfigJSON(provider: asrProvider, apiKey: asrApiKey)

        if let jsonData = try? JSONSerialization.data(withJSONObject: config),
           let jsonString = String(data: jsonData, encoding: .utf8) {
            print("📋 配置 JSON: \(jsonString)")
            VhisperManager.shared.initialize(configJSON: jsonString)
        }
    }

    /// 迁移旧的 provider 名称到新格式
    private func migrateProvider(_ provider: String) -> String {
        switch provider.lowercased() {
        case "qwen": return "Qwen"
        case "dashscope": return "DashScope"
        case "openai", "openaiwhisper": return "OpenAIWhisper"
        case "funasr": return "FunAsr"
        default: return provider
        }
    }

    /// 构建 Rust 期望的配置 JSON
    private func buildConfigJSON(provider: String, apiKey: String) -> [String: Any] {
        var asrConfig: [String: Any] = ["provider": provider]

        // 根据 provider 设置对应的嵌套配置
        switch provider {
        case "Qwen":
            asrConfig["qwen"] = ["api_key": apiKey]
        case "DashScope":
            asrConfig["dashscope"] = ["api_key": apiKey]
        case "OpenAIWhisper":
            asrConfig["openai"] = ["api_key": apiKey]
        case "FunAsr":
            asrConfig["funasr"] = ["endpoint": "http://localhost:10096"]
        default:
            // 默认使用 Qwen
            asrConfig["provider"] = "Qwen"
            asrConfig["qwen"] = ["api_key": apiKey]
        }

        return ["asr": asrConfig]
    }

    private func setupStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

        if let button = statusItem?.button {
            button.image = NSImage(systemSymbolName: "mic", accessibilityDescription: "Vhisper")
            button.action = #selector(togglePopover)
            button.target = self
        }

        popover = NSPopover()
        popover?.contentSize = NSSize(width: 280, height: 240)
        popover?.behavior = .transient
        popover?.contentViewController = NSHostingController(
            rootView: MenuBarView()
        )
    }

    @objc private func togglePopover() {
        guard let button = statusItem?.button, let popover = popover else { return }

        if popover.isShown {
            popover.performClose(nil)
        } else {
            popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
            NSApp.activate(ignoringOtherApps: true)
        }
    }

    private func requestMicrophonePermission() {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .audio) { granted in
                DispatchQueue.main.async {
                    if granted {
                        print("✅ 麦克风权限已授权")
                    } else {
                        print("⚠️ 麦克风权限被拒绝")
                    }
                }
            }
        case .denied, .restricted:
            print("⚠️ 麦克风权限被拒绝，请在系统设置中开启")
        case .authorized:
            print("✅ 麦克风权限已授权")
        @unknown default:
            break
        }
    }

    func updateStatusIcon(isRecording: Bool) {
        DispatchQueue.main.async {
            if let button = self.statusItem?.button {
                let imageName = isRecording ? "mic.fill" : "mic"
                button.image = NSImage(systemSymbolName: imageName, accessibilityDescription: "Vhisper")
                button.contentTintColor = isRecording ? .systemRed : nil
            }
        }
    }
}

// MARK: - Hotkey Manager

class HotkeyManager: ObservableObject {
    static let shared = HotkeyManager()

    @Published var currentHotkey: Hotkey = Hotkey.default
    @Published var isListeningForHotkey = false

    private var eventMonitor: Any?
    private var flagsMonitor: Any?

    struct Hotkey: Codable, Equatable {
        var keyCode: UInt16      // 0xFFFF 表示纯修饰键模式
        var modifiers: UInt32
        var isModifierOnly: Bool // 是否纯修饰键触发

        static let `default` = Hotkey(keyCode: 0xFFFF, modifiers: UInt32(optionKey), isModifierOnly: true) // 默认: 单按 Option

        init(keyCode: UInt16, modifiers: UInt32, isModifierOnly: Bool = false) {
            self.keyCode = keyCode
            self.modifiers = modifiers
            self.isModifierOnly = isModifierOnly
        }

        var displayString: String {
            var parts: [String] = []

            if modifiers & UInt32(controlKey) != 0 { parts.append("⌃") }
            if modifiers & UInt32(optionKey) != 0 { parts.append("⌥") }
            if modifiers & UInt32(shiftKey) != 0 { parts.append("⇧") }
            if modifiers & UInt32(cmdKey) != 0 { parts.append("⌘") }
            if modifiers & UInt32(NSEvent.ModifierFlags.function.rawValue) != 0 { parts.append("🌐") }

            if !isModifierOnly {
                parts.append(keyCodeToString(keyCode))
            }

            return parts.isEmpty ? "未设置" : parts.joined()
        }

        private func keyCodeToString(_ keyCode: UInt16) -> String {
            switch Int(keyCode) {
            case kVK_Space: return "Space"
            case kVK_Return: return "↩"
            case kVK_Tab: return "⇥"
            case kVK_Escape: return "⎋"
            case kVK_Delete: return "⌫"
            case kVK_ANSI_A...kVK_ANSI_Z:
                let letters = "ASDFHGZXCVBQWERYT123465=97-80]OU[IP"
                let index = letters.index(letters.startIndex, offsetBy: Int(keyCode))
                return String(letters[index])
            case kVK_ANSI_0: return "0"
            case kVK_ANSI_1: return "1"
            case kVK_ANSI_2: return "2"
            case kVK_ANSI_3: return "3"
            case kVK_ANSI_4: return "4"
            case kVK_ANSI_5: return "5"
            case kVK_ANSI_6: return "6"
            case kVK_ANSI_7: return "7"
            case kVK_ANSI_8: return "8"
            case kVK_ANSI_9: return "9"
            case kVK_F1: return "F1"
            case kVK_F2: return "F2"
            case kVK_F3: return "F3"
            case kVK_F4: return "F4"
            case kVK_F5: return "F5"
            case kVK_F6: return "F6"
            case kVK_F7: return "F7"
            case kVK_F8: return "F8"
            case kVK_F9: return "F9"
            case kVK_F10: return "F10"
            case kVK_F11: return "F11"
            case kVK_F12: return "F12"
            case 0x3F: return "🌐" // Fn/Globe key
            default: return "Key\(keyCode)"
            }
        }
    }

    private init() {
        loadHotkey()
    }

    private var isHotkeyPressed = false

    func register() {
        unregister()

        if currentHotkey.isModifierOnly {
            // 纯修饰键模式：只监听 flagsChanged
            flagsMonitor = NSEvent.addGlobalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
                self?.handleModifierOnlyHotkey(event)
            }
        } else {
            // 普通按键模式
            eventMonitor = NSEvent.addGlobalMonitorForEvents(matching: .keyDown) { [weak self] event in
                self?.handleKeyDown(event)
            }
            flagsMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.keyUp, .flagsChanged]) { [weak self] event in
                self?.handleKeyUp(event)
            }
        }

        print("✅ 热键已注册: \(currentHotkey.displayString)")
    }

    func unregister() {
        if let monitor = eventMonitor {
            NSEvent.removeMonitor(monitor)
            eventMonitor = nil
        }
        if let monitor = flagsMonitor {
            NSEvent.removeMonitor(monitor)
            flagsMonitor = nil
        }
        isHotkeyPressed = false
    }

    private func handleModifierOnlyHotkey(_ event: NSEvent) {
        guard !isListeningForHotkey else { return }

        let modifiers = event.modifierFlags.carbonFlags

        // 检查修饰键是否匹配
        let isPressed = (modifiers & currentHotkey.modifiers) == currentHotkey.modifiers

        if isPressed && !isHotkeyPressed {
            // 按下
            isHotkeyPressed = true
            DispatchQueue.main.async {
                VhisperManager.shared.startRecording()
            }
        } else if !isPressed && isHotkeyPressed {
            // 释放
            isHotkeyPressed = false
            DispatchQueue.main.async {
                if VhisperManager.shared.state == .recording {
                    VhisperManager.shared.stopRecording()
                }
            }
        }
    }

    private func handleKeyDown(_ event: NSEvent) {
        guard !isListeningForHotkey else { return }

        let keyCode = event.keyCode
        let modifiers = event.modifierFlags.carbonFlags

        if keyCode == currentHotkey.keyCode && modifiers == currentHotkey.modifiers && !isHotkeyPressed {
            isHotkeyPressed = true
            DispatchQueue.main.async {
                VhisperManager.shared.startRecording()
            }
        }
    }

    private func handleKeyUp(_ event: NSEvent) {
        guard !isListeningForHotkey else { return }

        if event.type == .keyUp && event.keyCode == currentHotkey.keyCode && isHotkeyPressed {
            isHotkeyPressed = false
            DispatchQueue.main.async {
                if VhisperManager.shared.state == .recording {
                    VhisperManager.shared.stopRecording()
                }
            }
        }
    }

    private var hotkeyRecordingMonitor: Any?
    private var hotkeyRecordingFlagsMonitor: Any?
    private var recordedModifiers: UInt32 = 0

    func startListeningForNewHotkey(completion: @escaping (Hotkey) -> Void) {
        unregister()
        isListeningForHotkey = true
        recordedModifiers = 0

        // 监听普通按键
        hotkeyRecordingMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            self?.handleHotkeyRecordingKeyDown(event: event, completion: completion)
            return nil
        }

        // 监听修饰键变化（用于纯修饰键模式）
        hotkeyRecordingFlagsMonitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            self?.handleHotkeyRecordingFlags(event: event, completion: completion)
            return event
        }

        // 5秒后自动取消
        DispatchQueue.main.asyncAfter(deadline: .now() + 5) { [weak self] in
            guard let self = self, self.isListeningForHotkey else { return }
            self.stopListeningForNewHotkey()
            self.register()
        }
    }

    private func handleHotkeyRecordingKeyDown(event: NSEvent, completion: @escaping (Hotkey) -> Void) {
        guard isListeningForHotkey else { return }

        // 普通按键 + 可能的修饰键
        let newHotkey = Hotkey(
            keyCode: event.keyCode,
            modifiers: event.modifierFlags.carbonFlags,
            isModifierOnly: false
        )

        finishHotkeyRecording(hotkey: newHotkey, completion: completion)
    }

    private func handleHotkeyRecordingFlags(event: NSEvent, completion: @escaping (Hotkey) -> Void) {
        guard isListeningForHotkey else { return }

        let currentFlags = event.modifierFlags.carbonFlags

        if currentFlags != 0 {
            // 修饰键按下，记录
            recordedModifiers = currentFlags
        } else if recordedModifiers != 0 {
            // 修饰键释放，创建纯修饰键热键
            let newHotkey = Hotkey(
                keyCode: 0xFFFF,
                modifiers: recordedModifiers,
                isModifierOnly: true
            )
            finishHotkeyRecording(hotkey: newHotkey, completion: completion)
        }
    }

    private func finishHotkeyRecording(hotkey: Hotkey, completion: @escaping (Hotkey) -> Void) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.currentHotkey = hotkey
            self.saveHotkey()
            self.stopListeningForNewHotkey()
            self.register()
            completion(hotkey)
        }
    }

    func stopListeningForNewHotkey() {
        isListeningForHotkey = false
        recordedModifiers = 0
        if let monitor = hotkeyRecordingMonitor {
            NSEvent.removeMonitor(monitor)
            hotkeyRecordingMonitor = nil
        }
        if let monitor = hotkeyRecordingFlagsMonitor {
            NSEvent.removeMonitor(monitor)
            hotkeyRecordingFlagsMonitor = nil
        }
    }

    private func saveHotkey() {
        if let data = try? JSONEncoder().encode(currentHotkey) {
            UserDefaults.standard.set(data, forKey: "vhisper.hotkey")
        }
    }

    private func loadHotkey() {
        if let data = UserDefaults.standard.data(forKey: "vhisper.hotkey"),
           let hotkey = try? JSONDecoder().decode(Hotkey.self, from: data) {
            currentHotkey = hotkey
        }
    }
}

extension NSEvent.ModifierFlags {
    var carbonFlags: UInt32 {
        var flags: UInt32 = 0
        if contains(.control) { flags |= UInt32(controlKey) }
        if contains(.option) { flags |= UInt32(optionKey) }
        if contains(.shift) { flags |= UInt32(shiftKey) }
        if contains(.command) { flags |= UInt32(cmdKey) }
        if contains(.function) { flags |= UInt32(NSEvent.ModifierFlags.function.rawValue) }
        return flags
    }
}

// MARK: - Vhisper Manager

@MainActor
class VhisperManager: ObservableObject {
    static let shared = VhisperManager()

    @Published var state: VhisperState = .idle
    @Published var lastResult: String = ""
    @Published var errorMessage: String?

    private var vhisper: Vhisper?

    enum VhisperState {
        case idle
        case recording
        case processing

        var description: String {
            switch self {
            case .idle: return "就绪"
            case .recording: return "录音中..."
            case .processing: return "处理中..."
            }
        }

        var icon: String {
            switch self {
            case .idle: return "mic"
            case .recording: return "mic.fill"
            case .processing: return "ellipsis.circle"
            }
        }
    }

    private init() {}

    func initialize(configJSON: String? = nil) {
        do {
            vhisper = try Vhisper(configJSON: configJSON)
            print("✅ Vhisper 初始化成功，版本: \(Vhisper.version)")
        } catch {
            errorMessage = "初始化失败: \(error.localizedDescription)"
            print("❌ Vhisper 初始化失败: \(error)")
        }
    }

    func startRecording() {
        guard let vhisper = vhisper else {
            errorMessage = "请先配置 API Key"
            return
        }

        guard state == .idle else { return }

        do {
            try vhisper.startRecording()
            state = .recording
            errorMessage = nil
            updateAppDelegateIcon(recording: true)
        } catch {
            errorMessage = "录音启动失败: \(error.localizedDescription)"
        }
    }

    func stopRecording() {
        guard let vhisper = vhisper, state == .recording else { return }

        state = .processing
        updateAppDelegateIcon(recording: false)

        Task {
            do {
                let result = try await vhisper.stopRecording()
                self.lastResult = result
                self.state = .idle
                self.errorMessage = nil

                insertText(result)
            } catch {
                self.state = .idle
                if case Vhisper.VhisperError.cancelled = error {
                    // 取消不算错误
                } else {
                    self.errorMessage = error.localizedDescription
                }
            }
        }
    }

    func cancel() {
        try? vhisper?.cancel()
        state = .idle
        updateAppDelegateIcon(recording: false)
    }

    func toggleRecording() {
        switch state {
        case .idle:
            startRecording()
        case .recording:
            stopRecording()
        case .processing:
            cancel()
        }
    }

    /// 确保辅助功能权限已授予（会触发系统弹窗）
    private func ensureAccessibility() -> Bool {
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
        return AXIsProcessTrustedWithOptions(options)
    }

    private func insertText(_ text: String) {
        guard !text.isEmpty else { return }

        print("📝 准备输入文本: \(text)")

        // 只检查权限状态，不弹窗（弹窗在设置页面手动触发）
        let trusted = AXIsProcessTrusted()
        print("📍 AXIsProcessTrusted: \(trusted)")

        // 使用 Espanso 风格的 CGEvent 输入（在主线程）
        DispatchQueue.main.async {
            self.sendUnicodeEventsEspansoStyle(text)
        }
    }

    /// Espanso 风格的 CGEvent Unicode 输入
    /// 参考: https://github.com/espanso/espanso/blob/dev/espanso-inject/src/mac/native.mm
    private func sendUnicodeEventsEspansoStyle(_ text: String) {
        // 关键点1: CGEventSource 用 nil (对应 Espanso 的 NULL)
        // 这样可以绕过某些系统限制

        // 关键点2: 检查并释放 Shift 键
        releaseShiftIfPressed()

        // 关键点3: 转换为 UTF-16 并分块处理（每块最多 20 字符）
        let utf16Chars = Array(text.utf16)
        let chunks = utf16Chars.chunked(into: 20)

        // 延迟参数（微秒）- Espanso 默认 1000
        let delayMicroseconds: useconds_t = 1000

        for chunk in chunks {
            var chars = chunk

            // 创建按键按下事件（source = nil）
            guard let keyDown = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: true) else {
                print("❌ 无法创建 keyDown 事件")
                continue
            }
            keyDown.keyboardSetUnicodeString(stringLength: chars.count, unicodeString: &chars)

            // 创建按键释放事件（source = nil）
            guard let keyUp = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: false) else {
                print("❌ 无法创建 keyUp 事件")
                continue
            }
            keyUp.keyboardSetUnicodeString(stringLength: chars.count, unicodeString: &chars)

            // 关键点4: 使用 kCGHIDEventTap 发送
            keyDown.post(tap: .cghidEventTap)

            // 关键点5: keyDown 和 keyUp 之间加延迟
            usleep(delayMicroseconds)

            keyUp.post(tap: .cghidEventTap)

            // 块之间也加延迟
            usleep(delayMicroseconds)
        }

        print("✅ 通过 CGEvent (Espanso 风格) 输入完成，共 \(chunks.count) 块")
    }

    /// 检查并释放 Shift 键（如果按下）
    /// Espanso 在发送前会先释放 Shift，避免字符变成大写
    private func releaseShiftIfPressed() {
        guard let checkEvent = CGEvent(source: nil) else { return }

        let shiftPressed = checkEvent.flags.contains(.maskShift)
        if shiftPressed {
            print("📍 检测到 Shift 键按下，先释放")

            // 发送 Shift 释放事件
            if let shiftUp = CGEvent(keyboardEventSource: nil, virtualKey: CGKeyCode(kVK_Shift), keyDown: false) {
                shiftUp.post(tap: .cghidEventTap)
                usleep(1000)
            }
        }
    }

    private func updateAppDelegateIcon(recording: Bool) {
        if let appDelegate = NSApp.delegate as? AppDelegate {
            appDelegate.updateStatusIcon(isRecording: recording)
        }
    }
}

// MARK: - Menu Bar View

struct MenuBarView: View {
    @ObservedObject var manager = VhisperManager.shared
    @ObservedObject var hotkeyManager = HotkeyManager.shared

    var body: some View {
        VStack(spacing: 12) {
            // 状态显示
            HStack {
                Image(systemName: manager.state.icon)
                    .font(.title2)
                    .foregroundColor(manager.state == .recording ? .red : .primary)
                    .symbolEffect(.pulse, isActive: manager.state == .recording)

                Text(manager.state.description)
                    .font(.headline)

                Spacer()

                Text(hotkeyManager.currentHotkey.displayString)
                    .font(.caption)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(Color.secondary.opacity(0.2))
                    .cornerRadius(4)
            }
            .padding(.top, 8)

            // 录音按钮
            Button(action: { manager.toggleRecording() }) {
                HStack {
                    Image(systemName: manager.state == .recording ? "stop.fill" : "mic.fill")
                    Text(manager.state == .recording ? "停止" : "开始录音")
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 6)
            }
            .buttonStyle(.borderedProminent)
            .tint(manager.state == .recording ? .red : .accentColor)
            .disabled(manager.state == .processing)

            // 最近结果
            if !manager.lastResult.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("最近结果:")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    Text(manager.lastResult)
                        .font(.callout)
                        .lineLimit(3)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .padding(8)
                .background(Color.secondary.opacity(0.1))
                .cornerRadius(6)
            }

            // 错误信息
            if let error = manager.errorMessage {
                Text(error)
                    .font(.caption)
                    .foregroundColor(.red)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .lineLimit(5)
                    .textSelection(.enabled)
            }

            Divider()

            // 底部按钮
            HStack {
                SettingsLink {
                    Text("设置")
                }
                .buttonStyle(.borderless)

                Spacer()

                Text("v\(Vhisper.version)")
                    .font(.caption)
                    .foregroundColor(.secondary)

                Spacer()

                Button("退出") {
                    NSApp.terminate(nil)
                }
                .buttonStyle(.borderless)
            }
            .padding(.bottom, 8)
        }
        .padding(.horizontal, 12)
        .frame(width: 260)
    }
}

// MARK: - Settings View

struct SettingsView: View {
    @ObservedObject var hotkeyManager = HotkeyManager.shared
    @AppStorage("vhisper.asr.provider") private var asrProvider = "Qwen"
    @AppStorage("vhisper.asr.apiKey") private var asrApiKey = ""
    @AppStorage("vhisper.llm.enabled") private var llmEnabled = false
    @State private var showingSaveConfirmation = false

    var body: some View {
        TabView {
            // 通用设置
            Form {
                Section("热键设置") {
                    HStack {
                        Text("录音热键")
                        Spacer()
                        Button(hotkeyManager.isListeningForHotkey ? "按下新热键..." : hotkeyManager.currentHotkey.displayString) {
                            hotkeyManager.startListeningForNewHotkey { _ in }
                        }
                        .buttonStyle(.bordered)
                    }

                    Text("按住热键开始录音，松开结束")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
            .formStyle(.grouped)
            .tabItem {
                Label("通用", systemImage: "gear")
            }

            // ASR 设置
            Form {
                Section("语音识别 (ASR)") {
                    Picker("服务商", selection: $asrProvider) {
                        Text("通义千问").tag("Qwen")
                        Text("DashScope").tag("DashScope")
                        Text("OpenAI Whisper").tag("OpenAIWhisper")
                        Text("FunASR (本地)").tag("FunAsr")
                    }

                    if asrProvider != "FunAsr" {
                        SecureField("API Key", text: $asrApiKey)
                            .textContentType(.password)
                    }

                    Button("保存并应用") {
                        reinitializeVhisper()
                        showingSaveConfirmation = true
                    }
                    .disabled(asrProvider != "FunAsr" && asrApiKey.isEmpty)
                }

                if showingSaveConfirmation {
                    Text("✅ 配置已保存")
                        .foregroundColor(.green)
                        .font(.caption)
                }

                Section("大语言模型 (LLM)") {
                    Toggle("启用文本优化", isOn: $llmEnabled)
                }
            }
            .formStyle(.grouped)
            .tabItem {
                Label("服务", systemImage: "cloud")
            }

            // 关于
            Form {
                Section("关于") {
                    LabeledContent("版本", value: Vhisper.version)
                    LabeledContent("Rust Core", value: "libvhisper_core")
                }

                Section("权限") {
                    HStack {
                        Text("麦克风")
                        Spacer()
                        if AVCaptureDevice.authorizationStatus(for: .audio) == .authorized {
                            Image(systemName: "checkmark.circle.fill")
                                .foregroundColor(.green)
                        } else {
                            Button("授权") {
                                NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")!)
                            }
                        }
                    }

                    HStack {
                        Text("辅助功能")
                        Spacer()
                        Button("检查") {
                            NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")!)
                        }
                    }
                }
            }
            .formStyle(.grouped)
            .tabItem {
                Label("关于", systemImage: "info.circle")
            }
        }
        .frame(width: 450, height: 300)
    }

    private func reinitializeVhisper() {
        let config = buildConfigJSON(provider: asrProvider, apiKey: asrApiKey)

        if let jsonData = try? JSONSerialization.data(withJSONObject: config),
           let jsonString = String(data: jsonData, encoding: .utf8) {
            print("📋 更新配置: \(jsonString)")
            VhisperManager.shared.initialize(configJSON: jsonString)
        }
    }

    /// 构建 Rust 期望的配置 JSON
    private func buildConfigJSON(provider: String, apiKey: String) -> [String: Any] {
        var asrConfig: [String: Any] = ["provider": provider]

        switch provider {
        case "Qwen":
            asrConfig["qwen"] = ["api_key": apiKey]
        case "DashScope":
            asrConfig["dashscope"] = ["api_key": apiKey]
        case "OpenAIWhisper":
            asrConfig["openai"] = ["api_key": apiKey]
        case "FunAsr":
            asrConfig["funasr"] = ["endpoint": "http://localhost:10096"]
        default:
            asrConfig["provider"] = "Qwen"
            asrConfig["qwen"] = ["api_key": apiKey]
        }

        return ["asr": asrConfig]
    }
}
