//! Keyboard input simulation
//! Uses Windows SendInput API to simulate Ctrl+C and Ctrl+V keystrokes

use std::thread;
use std::time::Duration;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_C, VK_CONTROL, VK_V,
};

/// Small delay between key events to ensure proper registration
const KEY_DELAY_MS: u64 = 10;

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
