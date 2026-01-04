//! System tray functionality
//! Creates a system tray icon with right-click menu

use anyhow::Result;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

/// Menu item IDs
pub const MENU_SETTINGS: &str = "settings";
pub const MENU_EXIT: &str = "exit";

/// Create the system tray icon and menu
pub fn create_tray() -> Result<TrayIcon> {
    // Create menu items
    let menu = Menu::new();

    let settings_item = MenuItem::with_id(MENU_SETTINGS, "Settings", true, None);
    let separator = PredefinedMenuItem::separator();
    let exit_item = MenuItem::with_id(MENU_EXIT, "Exit", true, None);

    menu.append(&settings_item)?;
    menu.append(&separator)?;
    menu.append(&exit_item)?;

    // Create tray icon
    // Using a simple embedded icon (16x16 RGBA)
    let icon = create_default_icon();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("NanoTrans - Translation Assistant")
        .with_icon(icon)
        .build()?;

    Ok(tray)
}

/// Create a simple default icon (16x16 blue square with "T")
fn create_default_icon() -> tray_icon::Icon {
    const SIZE: usize = 32;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];

    // Fill with a nice blue color
    for y in 0..SIZE {
        for x in 0..SIZE {
            let idx = (y * SIZE + x) * 4;

            // Create rounded rectangle effect
            let margin = 2;
            let in_bounds = x >= margin && x < SIZE - margin && y >= margin && y < SIZE - margin;

            if in_bounds {
                // Blue background
                rgba[idx] = 66;      // R
                rgba[idx + 1] = 133; // G
                rgba[idx + 2] = 244; // B
                rgba[idx + 3] = 255; // A

                // Draw "T" letter in white
                let center_x = SIZE / 2;
                let t_top = 6;
                let t_bottom = SIZE - 6;
                let t_width = 12;
                let bar_height = 4;
                let stem_width = 4;

                // Top bar of T
                if y >= t_top && y < t_top + bar_height {
                    if x >= center_x - t_width / 2 && x < center_x + t_width / 2 {
                        rgba[idx] = 255;
                        rgba[idx + 1] = 255;
                        rgba[idx + 2] = 255;
                    }
                }
                // Stem of T
                else if y >= t_top + bar_height && y < t_bottom {
                    if x >= center_x - stem_width / 2 && x < center_x + stem_width / 2 {
                        rgba[idx] = 255;
                        rgba[idx + 1] = 255;
                        rgba[idx + 2] = 255;
                    }
                }
            } else {
                // Transparent outside
                rgba[idx + 3] = 0;
            }
        }
    }

    tray_icon::Icon::from_rgba(rgba, SIZE as u32, SIZE as u32)
        .expect("Failed to create icon")
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
