//! Windows Caret position detection
//! Uses GetGUIThreadInfo to get the text cursor position, falls back to mouse position

use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
    GUITHREADINFO, GUI_CARETBLINKING,
};

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
