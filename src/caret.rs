//! Cross-platform caret position detection
//! Windows: Uses GetGUIThreadInfo to get text cursor position
//! macOS: Uses mouse position as fallback (Accessibility API requires permissions)

#[cfg(target_os = "windows")]
mod windows_impl {
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
        GetSystemMetrics, GUITHREADINFO, GUI_CARETBLINKING,
        SM_CXSCREEN, SM_CYSCREEN,
    };

    pub fn get_screen_size() -> (i32, i32) {
        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);
            (width, height)
        }
    }

    pub fn is_our_process_foreground() -> bool {
        unsafe {
            let foreground = GetForegroundWindow();
            if foreground.0.is_null() {
                return false;
            }

            let mut process_id: u32 = 0;
            GetWindowThreadProcessId(foreground, Some(&mut process_id));

            process_id == std::process::id()
        }
    }

    pub fn get_caret_position() -> (i32, i32) {
        if let Some(pos) = get_caret_from_gui_thread() {
            return pos;
        }
        get_mouse_position()
    }

    fn get_caret_from_gui_thread() -> Option<(i32, i32)> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }

            let thread_id = GetWindowThreadProcessId(hwnd, None);
            if thread_id == 0 {
                return None;
            }

            let mut gui_info = GUITHREADINFO {
                cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                flags: Default::default(),
                hwndActive: HWND::default(),
                hwndFocus: HWND::default(),
                hwndCapture: HWND::default(),
                hwndMenuOwner: HWND::default(),
                hwndMoveSize: HWND::default(),
                hwndCaret: HWND::default(),
                rcCaret: RECT::default(),
            };

            if GetGUIThreadInfo(thread_id, &mut gui_info).is_err() {
                return None;
            }

            if gui_info.hwndCaret.0.is_null() {
                return None;
            }

            if !gui_info.flags.contains(GUI_CARETBLINKING) {
                return None;
            }

            let mut point = POINT {
                x: gui_info.rcCaret.left,
                y: gui_info.rcCaret.bottom,
            };

            if ClientToScreen(gui_info.hwndCaret, &mut point).as_bool() {
                Some((point.x, point.y))
            } else {
                None
            }
        }
    }

    fn get_mouse_position() -> (i32, i32) {
        unsafe {
            let mut point = POINT::default();
            if GetCursorPos(&mut point).is_ok() {
                (point.x, point.y)
            } else {
                (0, 0)
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use core_graphics::display::CGDisplay;
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    pub fn get_screen_size() -> (i32, i32) {
        let display = CGDisplay::main();
        let bounds = display.bounds();
        (bounds.size.width as i32, bounds.size.height as i32)
    }

    pub fn is_our_process_foreground() -> bool {
        // macOS 下简化实现，总是返回 false 避免误判
        false
    }

    pub fn get_caret_position() -> (i32, i32) {
        // macOS 获取光标位置需要 Accessibility 权限，这里使用鼠标位置作为替代
        get_mouse_position()
    }

    fn get_mouse_position() -> (i32, i32) {
        if let Ok(source) = CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
            if let Ok(event) = CGEvent::new(source) {
                let location = event.location();
                return (location.x as i32, location.y as i32);
            }
        }
        (0, 0)
    }
}

// 公共接口
pub fn get_screen_size() -> (i32, i32) {
    #[cfg(target_os = "windows")]
    return windows_impl::get_screen_size();

    #[cfg(target_os = "macos")]
    return macos_impl::get_screen_size();

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    (1920, 1080)
}

pub fn calculate_popup_position(
    cursor_x: i32,
    cursor_y: i32,
    popup_width: i32,
    popup_height: i32,
) -> (i32, i32) {
    let (screen_width, screen_height) = get_screen_size();

    let mut x = cursor_x - popup_width / 2;
    let mut y = cursor_y - popup_height - 10;

    if x < 0 {
        x = 0;
    }
    if x + popup_width > screen_width {
        x = screen_width - popup_width;
    }

    if y < 0 {
        y = cursor_y + 20;
    }
    if y + popup_height > screen_height {
        y = screen_height - popup_height;
    }

    (x, y)
}

pub fn is_our_process_foreground() -> bool {
    #[cfg(target_os = "windows")]
    return windows_impl::is_our_process_foreground();

    #[cfg(target_os = "macos")]
    return macos_impl::is_our_process_foreground();

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    false
}

pub fn get_caret_position() -> (i32, i32) {
    #[cfg(target_os = "windows")]
    return windows_impl::get_caret_position();

    #[cfg(target_os = "macos")]
    return macos_impl::get_caret_position();

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    (0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_caret_position() {
        let (x, y) = get_caret_position();
        assert!(x >= -10000 && x <= 10000);
        assert!(y >= -10000 && y <= 10000);
    }
}
