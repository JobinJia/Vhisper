use tauri::State;

use crate::hotkey;
use crate::{AppConfig, AppState};

/// 获取当前配置
#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = state.config.read().await;
    Ok(config.clone())
}

/// 保存配置
#[tauri::command]
pub async fn save_config(state: State<'_, AppState>, config: AppConfig) -> Result<(), String> {
    // 保存到文件
    vhisper_core::save_config(&config).map_err(|e| e.to_string())?;

    // 获取新的快捷键绑定
    let new_binding = config.hotkey.binding.clone();

    // 更新内存中的配置
    let mut current_config = state.config.write().await;
    *current_config = config;

    // 触发快捷键热重载
    hotkey::reload_hotkey(new_binding);

    tracing::info!("Config saved and hotkey reloaded");
    Ok(())
}
