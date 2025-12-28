use tauri::{AppHandle, Emitter, State};

use crate::output;
use crate::{get_pipeline, AppState};

/// 开始录音
#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut is_recording = state.is_recording.write().await;
    if *is_recording {
        return Ok(());
    }

    if let Some(pipeline) = get_pipeline() {
        pipeline.start_recording().map_err(|e| e.to_string())?;
        *is_recording = true;
        let _ = app.emit("recording-started", ());
        tracing::info!("Recording started via command");
    }

    Ok(())
}

/// 停止录音并处理
#[tauri::command]
pub async fn stop_recording(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut is_recording = state.is_recording.write().await;
    if !*is_recording {
        return Ok(());
    }

    *is_recording = false;
    let _ = app.emit("recording-stopped", ());

    if let Some(pipeline) = get_pipeline() {
        let config = state.config.read().await;
        match pipeline.stop_and_process().await {
            Ok(text) => {
                // 输出文本到当前应用
                if !text.is_empty() {
                    if let Err(e) = output::output_text(
                        &text,
                        config.output.restore_clipboard,
                        config.output.paste_delay_ms,
                        None,
                    ) {
                        tracing::error!("Text output failed: {}", e);
                    }
                }
                let _ = app.emit("processing-complete", ());
                tracing::info!("Recording processed via command");
            }
            Err(e) => {
                let error_msg = e.to_string();
                let _ = app.emit("processing-error", &error_msg);
                return Err(error_msg);
            }
        }
    }

    Ok(())
}
