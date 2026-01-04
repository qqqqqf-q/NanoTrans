//! Keyboard input simulation and monitoring
//! Uses Windows SendInput API to simulate keystrokes
//! Uses Low-Level Keyboard Hook to monitor Ctrl+V

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_C, VK_CONTROL, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, HHOOK, KBDLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WM_KEYDOWN,
};

/// Small delay between key events to ensure proper registration
const KEY_DELAY_MS: u64 = 10;

/// Global flag to track if Ctrl+V was detected
static CTRL_V_DETECTED: AtomicBool = AtomicBool::new(false);

/// Global flag to track if Ctrl is currently pressed
static CTRL_PRESSED: AtomicBool = AtomicBool::new(false);

/// Check and clear the Ctrl+V detected flag
pub fn check_ctrl_v_pressed() -> bool {
    CTRL_V_DETECTED.swap(false, Ordering::SeqCst)
}

/// Low-level keyboard hook procedure
unsafe extern "system" fn keyboard_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let kb_struct = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = kb_struct.vkCode as u16;

        // Track Ctrl key state (VK_CONTROL=0x11, VK_LCONTROL=0xA2, VK_RCONTROL=0xA3)
        if vk_code == VK_CONTROL.0 || vk_code == 0xA2 || vk_code == 0xA3 {
            if wparam.0 as u32 == WM_KEYDOWN {
                CTRL_PRESSED.store(true, Ordering::SeqCst);
            } else {
                CTRL_PRESSED.store(false, Ordering::SeqCst);
            }
        }

        // Detect Ctrl+V
        if wparam.0 as u32 == WM_KEYDOWN && vk_code == VK_V.0 {
            if CTRL_PRESSED.load(Ordering::SeqCst) {
                CTRL_V_DETECTED.store(true, Ordering::SeqCst);
            }
        }
    }

    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

/// Start the keyboard hook in a background thread
pub fn start_keyboard_monitor() {
    thread::spawn(|| {
        unsafe {
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0);
            if hook.is_ok() {
                // Message loop to keep the hook alive
                let mut msg = std::mem::zeroed();
                while windows::Win32::UI::WindowsAndMessaging::GetMessageW(
                    &mut msg,
                    None,
                    0,
                    0,
                ).as_bool()
                {
                    windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                    windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
                }
            }
        }
    });
}

/// Create a keyboard input event
fn create_key_input(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Send a sequence of keyboard inputs
fn send_inputs(inputs: &[INPUT]) {
    unsafe {
        SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Simulate Ctrl+C keystroke to copy selected text
pub fn send_ctrl_c() {
    let inputs = [
        // Ctrl down
        create_key_input(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
        // C down
        create_key_input(VK_C, KEYBD_EVENT_FLAGS(0)),
        // C up
        create_key_input(VK_C, KEYEVENTF_KEYUP),
        // Ctrl up
        create_key_input(VK_CONTROL, KEYEVENTF_KEYUP),
    ];

    send_inputs(&inputs);

    // Small delay to ensure clipboard is updated
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));
}

/// Simulate Ctrl+V keystroke to paste clipboard content
pub fn send_ctrl_v() {
    let inputs = [
        // Ctrl down
        create_key_input(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
        // V down
        create_key_input(VK_V, KEYBD_EVENT_FLAGS(0)),
        // V up
        create_key_input(VK_V, KEYEVENTF_KEYUP),
        // Ctrl up
        create_key_input(VK_CONTROL, KEYEVENTF_KEYUP),
    ];

    send_inputs(&inputs);

    // Small delay to ensure paste completes
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));
}

/// Simulate Ctrl+A keystroke to select all text (useful for some scenarios)
#[allow(dead_code)]
pub fn send_ctrl_a() {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_A;

    let inputs = [
        create_key_input(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
        create_key_input(VK_A, KEYBD_EVENT_FLAGS(0)),
        create_key_input(VK_A, KEYEVENTF_KEYUP),
        create_key_input(VK_CONTROL, KEYEVENTF_KEYUP),
    ];

    send_inputs(&inputs);
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_key_input() {
        let input = create_key_input(VK_C, KEYBD_EVENT_FLAGS(0));
        assert_eq!(input.r#type, INPUT_KEYBOARD);
    }
}
