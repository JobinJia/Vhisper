pub mod audio;
pub mod asr;
pub mod commands;
pub mod config;
pub mod hotkey;
pub mod llm;
pub mod output;
pub mod pipeline;
pub mod tray;

use std::sync::{Arc, OnceLock};
use tauri::{Manager, RunEvent, WindowEvent};
use tokio::sync::RwLock;

pub use config::settings::AppConfig;
pub use pipeline::VoicePipeline;

/// 全局 Pipeline 实例
static VOICE_PIPELINE: OnceLock<Arc<VoicePipeline>> = OnceLock::new();

/// 获取全局 Pipeline
pub fn get_pipeline() -> Option<Arc<VoicePipeline>> {
    VOICE_PIPELINE.get().cloned()
}

/// 应用全局状态
pub struct AppState {
    pub config: Arc<RwLock<AppConfig>>,
    pub is_recording: Arc<RwLock<bool>>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            is_recording: Arc::new(RwLock::new(false)),
        }
    }
}

/// 初始化应用
pub fn run() {
    // 设置 panic hook 捕获所有 panic
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("!!! PANIC DETECTED !!!");
        eprintln!("{}", panic_info);
        if let Some(location) = panic_info.location() {
            eprintln!("Location: {}:{}:{}", location.file(), location.line(), location.column());
        }
    }));

    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tracing::info!("Starting Vhisper...");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(move |app| {
            // 加载配置
            let config = config::storage::load_config()
                .unwrap_or_else(|_| AppConfig::default());

            let config_arc = Arc::new(RwLock::new(config.clone()));

            // 初始化 VoicePipeline
            match VoicePipeline::new(config_arc.clone()) {
                Ok(pipeline) => {
                    let _ = VOICE_PIPELINE.set(Arc::new(pipeline));
                    tracing::info!("VoicePipeline initialized");
                }
                Err(e) => {
                    tracing::error!("Failed to initialize VoicePipeline: {}", e);
                }
            }

            // 初始化应用状态
            let state = AppState {
                config: config_arc,
                is_recording: Arc::new(RwLock::new(false)),
            };
            app.manage(state);

            // 设置系统托盘（必须保持 TrayIcon 存活，否则点击无效）
            let tray_icon = tray::setup_tray(app.handle())?;
            app.manage(tray_icon);

            // 启动全局快捷键监听
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                if let Err(e) = hotkey::start_listener(app_handle) {
                    tracing::error!("Failed to start hotkey listener: {}", e);
                }
            });

            // macOS: 设置为 Accessory 应用 (只显示托盘图标)
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            tracing::info!("vhisper initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::config::get_config,
            commands::config::save_config,
            commands::audio::start_recording,
            commands::audio::stop_recording,
            commands::test::test_dashscope_api,
            commands::test::test_openai_api,
            commands::test::test_funasr_api,
            commands::test::test_ollama_api,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match &event {
                // 拦截窗口关闭事件，隐藏窗口而不是退出应用
                RunEvent::WindowEvent {
                    label,
                    event: WindowEvent::CloseRequested { api, .. },
                    ..
                } => {
                    tracing::info!("Window {} close requested, hiding instead", label);
                    api.prevent_close();
                    if let Some(window) = app_handle.get_webview_window(label) {
                        let _ = window.hide();
                    }
                }
                RunEvent::WindowEvent {
                    label,
                    event: WindowEvent::Destroyed,
                    ..
                } => {
                    tracing::warn!("!!! Window {} destroyed !!!", label);
                }
                // 阻止应用退出（除非是从托盘菜单退出）
                RunEvent::ExitRequested { api, code, .. } => {
                    tracing::warn!("!!! Exit requested with code: {:?} !!!", code);
                    // 打印调用栈
                    let backtrace = std::backtrace::Backtrace::capture();
                    tracing::warn!("Backtrace:\n{}", backtrace);
                    // 只有当 code 是 Some(0) 时才允许退出（托盘菜单的正常退出）
                    // 其他情况阻止退出
                    if code.is_none() {
                        tracing::info!("Preventing exit (code is None)");
                        api.prevent_exit();
                    } else {
                        tracing::warn!("Allowing exit (code is {:?})", code);
                    }
                }
                RunEvent::Exit => {
                    tracing::warn!("!!! RunEvent::Exit - Application is exiting !!!");
                }
                _ => {}
            }
        });
}
