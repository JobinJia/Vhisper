/**
 * vhisper_core.h
 * Vhisper Core FFI - Swift/ObjC 接口
 */

#ifndef VHISPER_CORE_H
#define VHISPER_CORE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// ============================================================================
// 类型定义
// ============================================================================

/// 不透明句柄
typedef struct VhisperHandle VhisperHandle;

/// 结果回调函数
/// @param context 用户传入的上下文指针
/// @param text 成功时的文本结果（UTF-8），失败时为 NULL
/// @param error 失败时的错误信息（UTF-8），成功时为 NULL
typedef void (*VhisperResultCallback)(void *context, const char *text, const char *error);

/// 流式识别事件类型
typedef enum {
    VHISPER_EVENT_PARTIAL = 0,  // 中间结果
    VHISPER_EVENT_FINAL = 1,    // 最终结果
    VHISPER_EVENT_ERROR = 2     // 错误
} VhisperStreamingEventType;

/// 流式识别回调函数
/// @param context 用户传入的上下文指针
/// @param event_type 事件类型
/// @param text 已确认的文本（UTF-8），可能为 NULL
/// @param stash 暂定文本（UTF-8），仅 Partial 事件有效，其他为 NULL
/// @param error 错误信息（UTF-8），仅 Error 事件有效，其他为 NULL
typedef void (*VhisperStreamingCallback)(void *context,
                                          int32_t event_type,
                                          const char *text,
                                          const char *stash,
                                          const char *error);

// ============================================================================
// 生命周期
// ============================================================================

/// 创建 Vhisper 实例
/// @param config_json JSON 格式的配置字符串，可以为 NULL（使用默认配置）
/// @return 成功返回 Handle 指针，失败返回 NULL
VhisperHandle *vhisper_create(const char *config_json);

/// 销毁 Vhisper 实例
/// @param handle 由 vhisper_create 返回的句柄
void vhisper_destroy(VhisperHandle *handle);

// ============================================================================
// 状态查询
// ============================================================================

/// 获取当前状态
/// @param handle Vhisper 实例
/// @return 0=Idle, 1=Recording, 2=Processing, -1=handle无效
int32_t vhisper_get_state(VhisperHandle *handle);

// ============================================================================
// 录音控制
// ============================================================================

/// 开始录音
/// @param handle Vhisper 实例
/// @return 0=成功, -1=handle无效, -2=启动失败
int32_t vhisper_start_recording(VhisperHandle *handle);

/// 停止录音并处理（异步）
/// @param handle Vhisper 实例
/// @param callback 结果回调函数
/// @param context 传递给回调的用户上下文
/// @return 0=任务已提交, -1=handle无效
int32_t vhisper_stop_recording(VhisperHandle *handle,
                                VhisperResultCallback callback,
                                void *context);

/// 取消当前操作
/// @param handle Vhisper 实例
/// @return 0=成功, -1=handle无效, -2=取消失败
int32_t vhisper_cancel(VhisperHandle *handle);

// ============================================================================
// 流式识别
// ============================================================================

/// 开始流式录音和识别
/// 立即返回，识别结果通过回调持续通知
/// @param handle Vhisper 实例
/// @param callback 流式事件回调函数
/// @param context 传递给回调的用户上下文
/// @return 0=成功启动, -1=handle无效, -2=启动失败
int32_t vhisper_start_streaming(VhisperHandle *handle,
                                 VhisperStreamingCallback callback,
                                 void *context);

/// 停止流式录音
/// 提交当前音频缓冲区，回调会收到 Final 事件
/// @param handle Vhisper 实例
/// @return 0=成功, -1=handle无效
int32_t vhisper_stop_streaming(VhisperHandle *handle);

/// 取消流式识别
/// 停止录音并丢弃数据，不会触发 Final 回调
/// @param handle Vhisper 实例
/// @return 0=成功, -1=handle无效
int32_t vhisper_cancel_streaming(VhisperHandle *handle);

/// 检查是否在流式模式
/// @param handle Vhisper 实例
/// @return 1=流式模式, 0=非流式模式, -1=handle无效
int32_t vhisper_is_streaming(VhisperHandle *handle);

// ============================================================================
// 配置
// ============================================================================

/// 更新配置
/// @param handle Vhisper 实例
/// @param config_json 新的 JSON 配置
/// @return 0=成功, -1=handle无效, -2=JSON解析失败
int32_t vhisper_update_config(VhisperHandle *handle, const char *config_json);

// ============================================================================
// 工具函数
// ============================================================================

/// 释放由 FFI 返回的字符串
/// @param s 由本库返回的字符串指针
void vhisper_string_free(char *s);

/// 获取版本号
/// @return 版本字符串（静态，无需释放）
const char *vhisper_version(void);

#ifdef __cplusplus
}
#endif

#endif // VHISPER_CORE_H
