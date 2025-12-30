//
//  VhisperBridge.swift
//  vhisper
//
//  Swift 封装层 - 提供类型安全的 API
//

import Foundation
import VhisperCore

/// Vhisper 核心封装
public final class Vhisper {

    // MARK: - Types

    /// 状态枚举
    public enum State: Int32 {
        case idle = 0
        case recording = 1
        case processing = 2
        case invalid = -1
    }

    /// 结果类型
    public enum Result {
        case success(String)
        case failure(Error)
        case cancelled
    }

    /// 错误类型
    public enum VhisperError: Error, LocalizedError {
        case invalidHandle
        case startFailed
        case configParseFailed
        case cancelled
        case processingFailed(String)

        public var errorDescription: String? {
            switch self {
            case .invalidHandle: return "Invalid Vhisper handle"
            case .startFailed: return "Failed to start recording"
            case .configParseFailed: return "Failed to parse config JSON"
            case .cancelled: return "Operation cancelled"
            case .processingFailed(let msg): return msg
            }
        }
    }

    // MARK: - Properties

    private var handle: OpaquePointer?

    /// 获取当前状态
    public var state: State {
        guard let h = handle else { return .invalid }
        return State(rawValue: vhisper_get_state(h)) ?? .invalid
    }

    /// 是否正在录音
    public var isRecording: Bool {
        return state == .recording
    }

    /// 是否正在处理
    public var isProcessing: Bool {
        return state == .processing
    }

    /// 是否空闲
    public var isIdle: Bool {
        return state == .idle
    }

    // MARK: - Lifecycle

    /// 初始化
    /// - Parameter configJSON: 可选的 JSON 配置字符串
    public init(configJSON: String? = nil) throws {
        if let json = configJSON {
            handle = json.withCString { vhisper_create($0) }
        } else {
            handle = vhisper_create(nil)
        }

        guard handle != nil else {
            throw VhisperError.invalidHandle
        }
    }

    deinit {
        if let h = handle {
            vhisper_destroy(h)
        }
    }

    // MARK: - Recording Control

    /// 开始录音
    public func startRecording() throws {
        guard let h = handle else { throw VhisperError.invalidHandle }

        let result = vhisper_start_recording(h)
        if result != 0 {
            throw VhisperError.startFailed
        }
    }

    /// 停止录音并处理（回调版本）
    /// - Parameter completion: 完成回调，在后台线程调用
    public func stopRecording(completion: @escaping (Result) -> Void) {
        guard let h = handle else {
            completion(.failure(VhisperError.invalidHandle))
            return
        }

        let context = CallbackContext(completion: completion)
        let contextPtr = Unmanaged.passRetained(context).toOpaque()

        vhisper_stop_recording(h, { ctx, text, error in
            guard let ctx = ctx else { return }

            let context = Unmanaged<CallbackContext>.fromOpaque(ctx).takeRetainedValue()

            if let errorPtr = error {
                let errorMsg = String(cString: errorPtr)
                if errorMsg.lowercased().contains("cancel") {
                    context.completion(.cancelled)
                } else {
                    context.completion(.failure(VhisperError.processingFailed(errorMsg)))
                }
            } else if let textPtr = text {
                let result = String(cString: textPtr)
                context.completion(.success(result))
            } else {
                context.completion(.failure(VhisperError.processingFailed("Unknown error")))
            }
        }, contextPtr)
    }

    /// 取消当前操作
    public func cancel() throws {
        guard let h = handle else { throw VhisperError.invalidHandle }
        _ = vhisper_cancel(h)
    }

    // MARK: - Configuration

    /// 更新配置
    /// - Parameter configJSON: 新的 JSON 配置
    public func updateConfig(_ configJSON: String) throws {
        guard let h = handle else { throw VhisperError.invalidHandle }

        let result = configJSON.withCString { vhisper_update_config(h, $0) }
        if result == -2 {
            throw VhisperError.configParseFailed
        } else if result != 0 {
            throw VhisperError.invalidHandle
        }
    }

    // MARK: - Static

    /// 获取版本号
    public static var version: String {
        guard let ptr = vhisper_version() else { return "unknown" }
        return String(cString: ptr)
    }
}

// MARK: - Callback Context

private class CallbackContext {
    let completion: (Vhisper.Result) -> Void

    init(completion: @escaping (Vhisper.Result) -> Void) {
        self.completion = completion
    }
}

// MARK: - Async/Await Extension

extension Vhisper {
    /// 停止录音并处理（async 版本）
    @MainActor
    public func stopRecording() async throws -> String {
        return try await withCheckedThrowingContinuation { continuation in
            stopRecording { result in
                DispatchQueue.main.async {
                    switch result {
                    case .success(let text):
                        continuation.resume(returning: text)
                    case .failure(let error):
                        continuation.resume(throwing: error)
                    case .cancelled:
                        continuation.resume(throwing: VhisperError.cancelled)
                    }
                }
            }
        }
    }
}
