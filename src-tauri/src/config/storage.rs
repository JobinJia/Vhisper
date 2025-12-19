use std::fs;
use std::path::PathBuf;

use crate::config::settings::AppConfig;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Config directory not found")]
    DirNotFound,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// 获取配置文件路径
fn get_config_path() -> Result<PathBuf, ConfigError> {
    let config_dir = dirs::config_dir().ok_or(ConfigError::DirNotFound)?;
    let app_dir = config_dir.join("com.vhisper.app");
    fs::create_dir_all(&app_dir)?;
    Ok(app_dir.join("config.json"))
}

/// 加载配置
pub fn load_config() -> Result<AppConfig, ConfigError> {
    let path = get_config_path()?;

    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let content = fs::read_to_string(&path)?;
    let config: AppConfig = serde_json::from_str(&content)?;

    Ok(config)
}

/// 保存配置
pub fn save_config(config: &AppConfig) -> Result<(), ConfigError> {
    let path = get_config_path()?;
    tracing::info!("Saving config to: {:?}", path);
    let content = serde_json::to_string_pretty(config)?;
    fs::write(&path, &content)?;
    tracing::info!("Config saved successfully");
    Ok(())
}
