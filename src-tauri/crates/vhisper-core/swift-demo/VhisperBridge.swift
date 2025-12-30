import Foundation
import VhisperCore

/// Swift 封装层 - 提供类型安全的 API
public final class Vhisper {

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

    private var handle: OpaquePointer?

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

    /// 获取当前状态
    public var state: State {
        guard let h = handle else { return .invalid }
        return State(rawValue: vhisper_get_state(h)) ?? .invalid
    }

    /// 是否正在录音
    public var isRecording: Bool {
        return state == .recording
    }

    /// 开始录音
    public func startRecording() throws {
        guard let h = handle else { throw VhisperError.invalidHandle }

        let result = vhisper_start_recording(h)
        if result != 0 {
            throw VhisperError.startFailed
        }
    }

    /// 停止录音并处理（异步）
    /// - Parameter completion: 完成回调，在后台线程调用
    public func stopRecording(completion: @escaping (Result) -> Void) {
        guard let h = handle else {
            completion(.failure(VhisperError.invalidHandle))
            return
        }

        // 创建回调上下文
        let context = CallbackContext(completion: completion)
        let contextPtr = Unmanaged.passRetained(context).toOpaque()

        // 调用 FFI
        vhisper_stop_recording(h, { ctx, text, error in
            guard let ctx = ctx else { return }

            let context = Unmanaged<CallbackContext>.fromOpaque(ctx).takeRetainedValue()

            if let errorPtr = error {
                let errorMsg = String(cString: errorPtr)
                if errorMsg.contains("Cancelled") || errorMsg.contains("cancelled") {
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

    /// 获取版本号
    public static var version: String {
        guard let ptr = vhisper_version() else { return "unknown" }
        return String(cString: ptr)
    }
}

// MARK: - 回调上下文

private class CallbackContext {
    let completion: (Vhisper.Result) -> Void

    init(completion: @escaping (Vhisper.Result) -> Void) {
        self.completion = completion
    }
}

// MARK: - Async/Await 扩展

@available(macOS 10.15, *)
extension Vhisper {
    /// 停止录音并处理（async）
    public func stopRecording() async throws -> String {
        return try await withCheckedThrowingContinuation { continuation in
            stopRecording { result in
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
