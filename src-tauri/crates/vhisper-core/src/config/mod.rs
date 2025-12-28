pub mod settings;
pub mod storage;

pub use settings::{AppConfig, HotkeyBinding, KeyCode};
pub use storage::{load_config, save_config};
