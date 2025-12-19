use tauri::State;

use crate::config::{self, AppConfig};
use crate::AppState;

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
    config::storage::save_config(&config).map_err(|e| e.to_string())?;

    // 更新内存中的配置
    let mut current_config = state.config.write().await;
    *current_config = config;

    tracing::info!("Config saved and updated");
    Ok(())
}
