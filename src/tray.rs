//! System tray functionality
//! Creates a system tray icon with right-click menu

use anyhow::Result;
use image::ImageReader;
use std::io::Cursor;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

// 嵌入图标文件
#[cfg(target_os = "windows")]
const ICON_BYTES: &[u8] = include_bytes!("../assets/icons/icon.ico");
#[cfg(target_os = "macos")]
const ICON_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tray.png"));

/// Menu item IDs
pub const MENU_SETTINGS: &str = "settings";
pub const MENU_EXIT: &str = "exit";

/// Create the system tray icon and menu
pub fn create_tray() -> Result<TrayIcon> {
    // macOS 需要在主线程初始化托盘
    #[cfg(target_os = "macos")]
    {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            // 确保只初始化一次
        });
    }

    // Create menu items
    let menu = Menu::new();

    let settings_item = MenuItem::with_id(MENU_SETTINGS, "Settings", true, None);
    let separator = PredefinedMenuItem::separator();
    let exit_item = MenuItem::with_id(MENU_EXIT, "Exit", true, None);

    menu.append(&settings_item)?;
    menu.append(&separator)?;
    menu.append(&exit_item)?;

    // Create tray icon
    let icon = create_default_icon();

    let mut builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("NanoTrans - Translation Assistant")
        .with_icon(icon);

    #[cfg(target_os = "macos")]
    {
        builder = builder.with_icon_as_template(true);
    }

    let tray = builder.build()?;

    Ok(tray)
}

/// Create a simple default icon (16x16 blue square with "T")
fn create_default_icon() -> tray_icon::Icon {
    // 从嵌入的 png 文件加载图标
    let img = ImageReader::new(Cursor::new(ICON_BYTES))
        .with_guessed_format()
        .expect("Failed to guess icon format")
        .decode()
        .expect("Failed to decode icon");

    // 缩放到 32x32 用于托盘显示
    let img = img.resize(32, 32, image::imageops::FilterType::Lanczos3);
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    tray_icon::Icon::from_rgba(rgba.into_raw(), width, height)
        .expect("Failed to create tray icon")
}

/// Handle menu events
pub fn handle_menu_event(event: &MenuEvent) -> MenuAction {
    match event.id.0.as_str() {
        MENU_SETTINGS => MenuAction::OpenSettings,
        MENU_EXIT => MenuAction::Exit,
        _ => MenuAction::None,
    }
}

/// Actions that can be triggered from the tray menu
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    OpenSettings,
    Exit,
    None,
}

/// Get the menu event receiver
pub fn menu_event_receiver() -> &'static crossbeam_channel::Receiver<MenuEvent> {
    MenuEvent::receiver()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_default_icon() {
        let icon = create_default_icon();
        // Just verify it doesn't panic
        drop(icon);
    }
}
