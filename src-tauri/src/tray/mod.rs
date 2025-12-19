use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

#[derive(Debug, thiserror::Error)]
pub enum TrayError {
    #[error("Failed to setup tray: {0}")]
    Setup(String),
}

// 嵌入图标
const ICON_BYTES: &[u8] = include_bytes!("../../icons/icon.png");

/// 设置系统托盘，返回 TrayIcon 对象（必须保持存活）
pub fn setup_tray(app: &AppHandle) -> Result<TrayIcon, TrayError> {
    // 创建菜单项
    let settings_item = MenuItem::with_id(app, "settings", "设置...", true, None::<&str>)
        .map_err(|e| TrayError::Setup(e.to_string()))?;

    let separator = PredefinedMenuItem::separator(app)
        .map_err(|e| TrayError::Setup(e.to_string()))?;

    let quit_item = MenuItem::with_id(app, "quit", "退出 Vhisper", true, Some("CmdOrCtrl+Q"))
        .map_err(|e| TrayError::Setup(e.to_string()))?;

    // 创建菜单
    let menu = Menu::with_items(app, &[&settings_item, &separator, &quit_item])
        .map_err(|e| TrayError::Setup(e.to_string()))?;

    // 从 PNG 解码图标
    let icon = load_icon_from_png(ICON_BYTES)
        .map_err(|e| TrayError::Setup(format!("Failed to load icon: {}", e)))?;

    tracing::info!("Creating tray icon with menu...");

    // 创建托盘图标
    let tray = TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(true)  // macOS: 使用模板图标
        .menu(&menu)
        .show_menu_on_left_click(true)  // 左键点击显示菜单
        .tooltip("vhisper - 语音输入")
        .on_tray_icon_event(|_tray, event| {
            tracing::info!("Tray icon event: {:?}", event);

            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                tracing::info!("Left click on tray icon");
                // 左键点击时菜单会自动显示（因为 show_menu_on_left_click(true)）
            }
        })
        .on_menu_event(|app, event| {
            tracing::info!("Menu event: {:?}", event.id);
            match event.id.as_ref() {
                "settings" => {
                    tracing::info!("Settings menu clicked");
                    // 显示主窗口
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    tracing::info!("Quit menu clicked");
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)
        .map_err(|e| TrayError::Setup(e.to_string()))?;

    tracing::info!("System tray initialized successfully, tray id: {:?}", tray.id());
    Ok(tray)
}

/// 从 PNG 数据加载图标
fn load_icon_from_png(png_data: &[u8]) -> Result<Image<'static>, String> {
    let decoder = png::Decoder::new(png_data);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;

    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;

    // 确保是 RGBA 格式
    let rgba = if info.color_type == png::ColorType::Rgba {
        buf[..info.buffer_size()].to_vec()
    } else if info.color_type == png::ColorType::Rgb {
        // RGB 转 RGBA
        let rgb = &buf[..info.buffer_size()];
        let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
        for chunk in rgb.chunks(3) {
            rgba.extend_from_slice(chunk);
            rgba.push(255);
        }
        rgba
    } else {
        return Err(format!("Unsupported color type: {:?}", info.color_type));
    };

    Ok(Image::new_owned(rgba, info.width, info.height))
}
