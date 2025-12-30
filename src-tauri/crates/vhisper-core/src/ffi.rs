//! FFI 层 - 为 Swift/ObjC 提供 C 接口
//!
//! # 内存管理约定
//! - 所有返回的字符串由调用方负责释放，使用 `vhisper_string_free`
//! - Handle 由 `vhisper_create` 创建，`vhisper_destroy` 销毁
//!
//! # 线程安全
//! - 所有函数都是线程安全的
//! - 回调会在后台线程调用，Swift 侧需要 dispatch 到主线程

use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;
use std::sync::{Arc, OnceLock};

use tokio::runtime::Runtime;
use tokio::sync::RwLock;

use crate::config::AppConfig;
use crate::pipeline::VoicePipeline;

// ============================================================================
// 全局 Runtime
// ============================================================================

/// 全局 tokio runtime，懒初始化
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        Runtime::new().expect("Failed to create tokio runtime")
    })
}

// ============================================================================
// Handle 定义
// ============================================================================

/// 不透明句柄，供 Swift 持有
pub struct VhisperHandle {
    pipeline: Arc<VoicePipeline>,
    config: Arc<RwLock<AppConfig>>,
}

// ============================================================================
// 回调类型
// ============================================================================

/// 结果回调函数类型
/// - context: 用户传入的上下文指针
/// - text: 成功时的文本结果（UTF-8），失败时为 NULL
/// - error: 失败时的错误信息（UTF-8），成功时为 NULL
pub type VhisperResultCallback =
    extern "C" fn(context: *mut c_void, text: *const c_char, error: *const c_char);

// ============================================================================
// FFI 函数
// ============================================================================

/// 创建 Vhisper 实例
///
/// # 参数
/// - config_json: JSON 格式的配置字符串，可以为 NULL（使用默认配置）
///
/// # 返回
/// - 成功返回 Handle 指针
/// - 失败返回 NULL
#[no_mangle]
pub extern "C" fn vhisper_create(config_json: *const c_char) -> *mut VhisperHandle {
    let config = if config_json.is_null() {
        AppConfig::default()
    } else {
        let c_str = unsafe { CStr::from_ptr(config_json) };
        match c_str.to_str() {
            Ok(json) => match serde_json::from_str(json) {
                Ok(cfg) => cfg,
                Err(e) => {
                    tracing::error!("Failed to parse config JSON: {}", e);
                    return ptr::null_mut();
                }
            },
            Err(e) => {
                tracing::error!("Invalid UTF-8 in config: {}", e);
                return ptr::null_mut();
            }
        }
    };

    let config_arc = Arc::new(RwLock::new(config));

    match VoicePipeline::new(config_arc.clone()) {
        Ok(pipeline) => {
            let handle = Box::new(VhisperHandle {
                pipeline: Arc::new(pipeline),
                config: config_arc,
            });
            Box::into_raw(handle)
        }
        Err(e) => {
            tracing::error!("Failed to create VoicePipeline: {}", e);
            ptr::null_mut()
        }
    }
}

/// 销毁 Vhisper 实例
///
/// # 安全
/// - handle 必须是 `vhisper_create` 返回的有效指针
/// - 调用后 handle 不可再使用
#[no_mangle]
pub extern "C" fn vhisper_destroy(handle: *mut VhisperHandle) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}

/// 获取当前状态
///
/// # 返回
/// - 0: 空闲 (Idle)
/// - 1: 录音中 (Recording)
/// - 2: 处理中 (Processing)
/// - -1: handle 无效
#[no_mangle]
pub extern "C" fn vhisper_get_state(handle: *mut VhisperHandle) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let handle = unsafe { &*handle };
    handle.pipeline.get_state() as i32
}

/// 开始录音
///
/// # 返回
/// - 0: 成功
/// - -1: handle 无效
/// - -2: 录音启动失败（可能正在录音或处理中）
#[no_mangle]
pub extern "C" fn vhisper_start_recording(handle: *mut VhisperHandle) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let handle = unsafe { &*handle };

    match handle.pipeline.start_recording() {
        Ok(_) => 0,
        Err(e) => {
            tracing::error!("Failed to start recording: {}", e);
            -2
        }
    }
}

/// 取消当前操作
///
/// - 录音中：停止录音，丢弃数据
/// - 处理中：标记取消，回调会返回 Cancelled 错误
/// - 空闲：无操作
///
/// # 返回
/// - 0: 成功
/// - -1: handle 无效
#[no_mangle]
pub extern "C" fn vhisper_cancel(handle: *mut VhisperHandle) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let handle = unsafe { &*handle };

    match handle.pipeline.cancel() {
        Ok(_) => 0,
        Err(e) => {
            tracing::error!("Failed to cancel: {}", e);
            -2
        }
    }
}

/// 停止录音并处理
///
/// 立即返回，结果通过回调通知
///
/// # 参数
/// - handle: Vhisper 实例
/// - callback: 结果回调函数
/// - context: 传递给回调的用户上下文
///
/// # 返回
/// - 0: 任务已提交
/// - -1: handle 无效
#[no_mangle]
pub extern "C" fn vhisper_stop_recording(
    handle: *mut VhisperHandle,
    callback: VhisperResultCallback,
    context: *mut c_void,
) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let handle = unsafe { &*handle };
    let pipeline = handle.pipeline.clone();

    // context 指针转为 usize 以满足 Send 约束
    let context_usize = context as usize;

    get_runtime().spawn(async move {
        let result = pipeline.stop_and_process().await;

        // 回调时才转换回指针
        let ctx = context_usize as *mut c_void;

        match result {
            Ok(text) => {
                let c_text = CString::new(text).unwrap_or_default();
                callback(ctx, c_text.as_ptr(), ptr::null());
            }
            Err(e) => {
                let error_msg = CString::new(e.to_string()).unwrap_or_default();
                callback(ctx, ptr::null(), error_msg.as_ptr());
            }
        }
    });

    0
}

/// 更新配置
///
/// # 参数
/// - handle: Vhisper 实例
/// - config_json: 新的 JSON 配置
///
/// # 返回
/// - 0: 成功
/// - -1: handle 无效
/// - -2: JSON 解析失败
#[no_mangle]
pub extern "C" fn vhisper_update_config(
    handle: *mut VhisperHandle,
    config_json: *const c_char,
) -> i32 {
    if handle.is_null() || config_json.is_null() {
        return -1;
    }

    let handle = unsafe { &*handle };

    let c_str = unsafe { CStr::from_ptr(config_json) };
    let json = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };

    let new_config: AppConfig = match serde_json::from_str(json) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::error!("Failed to parse config: {}", e);
            return -2;
        }
    };

    get_runtime().block_on(async {
        let mut config = handle.config.write().await;
        *config = new_config;
    });

    0
}

/// 释放由 FFI 返回的字符串
///
/// # 安全
/// - 只能释放由本库返回的字符串
#[no_mangle]
pub extern "C" fn vhisper_string_free(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            drop(CString::from_raw(s));
        }
    }
}

/// 获取版本号
#[no_mangle]
pub extern "C" fn vhisper_version() -> *const c_char {
    static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr() as *const c_char
}
