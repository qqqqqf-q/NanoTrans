//! Windows Caret position detection
//! Uses GetGUIThreadInfo to get the text cursor position, falls back to mouse position

use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
    GetSystemMetrics, GUITHREADINFO, GUI_CARETBLINKING,
    SM_CXSCREEN, SM_CYSCREEN,
};

/// Get screen dimensions (width, height)
pub fn get_screen_size() -> (i32, i32) {
    unsafe {
        let width = GetSystemMetrics(SM_CXSCREEN);
        let height = GetSystemMetrics(SM_CYSCREEN);
        (width, height)
    }
}

/// Calculate optimal popup position
/// Returns (x, y) position that:
/// - Centers the popup horizontally above the cursor
/// - Ensures the popup stays within screen bounds
pub fn calculate_popup_position(
    cursor_x: i32,
    cursor_y: i32,
    popup_width: i32,
    popup_height: i32,
) -> (i32, i32) {
    let (screen_width, screen_height) = get_screen_size();

    // 窗口水平居中于鼠标位置
    let mut x = cursor_x - popup_width / 2;
    // 窗口显示在鼠标上方（留 10px 间距）
    let mut y = cursor_y - popup_height - 10;

    // 检查左边界
    if x < 0 {
        x = 0;
    }
    // 检查右边界
    if x + popup_width > screen_width {
        x = screen_width - popup_width;
    }

    // 检查上边界：如果上方空间不够，则显示在鼠标下方
    if y < 0 {
        y = cursor_y + 20;
    }
    // 检查下边界
    if y + popup_height > screen_height {
        y = screen_height - popup_height;
    }

    (x, y)
}

/// Check if our process owns the foreground window
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

/// Get the current caret (text cursor) position in screen coordinates
/// Falls back to mouse cursor position if caret cannot be detected
pub fn get_caret_position() -> (i32, i32) {
    // Try to get caret position first
    if let Some(pos) = get_caret_from_gui_thread() {
        return pos;
    }

    // Fallback to mouse cursor position
    get_mouse_position()
}

/// Attempt to get caret position using GetGUIThreadInfo
fn get_caret_from_gui_thread() -> Option<(i32, i32)> {
    unsafe {
        // Get the foreground window
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }

        // Get the thread ID of the foreground window
        let thread_id = GetWindowThreadProcessId(hwnd, None);
        if thread_id == 0 {
            return None;
        }

        // Initialize GUITHREADINFO structure
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

        // Get GUI thread info
        if GetGUIThreadInfo(thread_id, &mut gui_info).is_err() {
            return None;
        }

        // Check if there's an active caret
        if gui_info.hwndCaret.0.is_null() {
            return None;
        }

        // Check if caret is blinking (active)
        if !gui_info.flags.contains(GUI_CARETBLINKING) {
            return None;
        }

        // Get caret position (bottom-left of caret rect for popup placement)
        let mut point = POINT {
            x: gui_info.rcCaret.left,
            y: gui_info.rcCaret.bottom,
        };

        // Convert from client coordinates to screen coordinates
        if ClientToScreen(gui_info.hwndCaret, &mut point).as_bool() {
            Some((point.x, point.y))
        } else {
            None
        }
    }
}

/// Get mouse cursor position as fallback
fn get_mouse_position() -> (i32, i32) {
    unsafe {
        let mut point = POINT::default();
        if GetCursorPos(&mut point).is_ok() {
            (point.x, point.y)
        } else {
            // Ultimate fallback: center of screen (shouldn't happen)
            (0, 0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_caret_position() {
        // This will likely return mouse position in test environment
        let (x, y) = get_caret_position();
        // Just verify it doesn't panic and returns reasonable values
        assert!(x >= -10000 && x <= 10000);
        assert!(y >= -10000 && y <= 10000);
    }
}
