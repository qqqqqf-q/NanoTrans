//! Keyboard input simulation and monitoring
//! Uses Windows SendInput API to simulate keystrokes
//! Uses Low-Level Keyboard Hook to monitor Ctrl+V and capture hotkeys

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use once_cell::sync::Lazy;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyNameTextW, MapVirtualKeyW, MAPVK_VK_TO_VSC, SendInput, INPUT,
    INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_C,
    VK_CONTROL, VK_ESCAPE, VK_TAB, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, HHOOK, KBDLLHOOKSTRUCT, KBDLLHOOKSTRUCT_FLAGS,
    LLKHF_EXTENDED, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

/// Small delay between key events to ensure proper registration
const KEY_DELAY_MS: u64 = 10;

/// Global flag to track if Ctrl+V was detected
static CTRL_V_DETECTED: AtomicBool = AtomicBool::new(false);

/// Global flag to track if Ctrl is currently pressed
static CTRL_PRESSED: AtomicBool = AtomicBool::new(false);

/// Hotkey capture state
static HOTKEY_CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);
static HOTKEY_CAPTURE_CTRL: AtomicBool = AtomicBool::new(false);
static HOTKEY_CAPTURE_ALT: AtomicBool = AtomicBool::new(false);
static HOTKEY_CAPTURE_SHIFT: AtomicBool = AtomicBool::new(false);
static HOTKEY_CAPTURE_WIN: AtomicBool = AtomicBool::new(false);
static CAPTURED_HOTKEY: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

/// Start hotkey capture mode
pub fn start_hotkey_capture() {
    HOTKEY_CAPTURE_ACTIVE.store(true, Ordering::SeqCst);
    HOTKEY_CAPTURE_CTRL.store(false, Ordering::SeqCst);
    HOTKEY_CAPTURE_ALT.store(false, Ordering::SeqCst);
    HOTKEY_CAPTURE_SHIFT.store(false, Ordering::SeqCst);
    HOTKEY_CAPTURE_WIN.store(false, Ordering::SeqCst);
    *CAPTURED_HOTKEY.lock().unwrap() = None;
    log_hotkey("start capture");
}

/// Stop hotkey capture mode
pub fn stop_hotkey_capture() {
    HOTKEY_CAPTURE_ACTIVE.store(false, Ordering::SeqCst);
    log_hotkey("stop capture");
}

/// Check if a hotkey was captured and return it
pub fn get_captured_hotkey() -> Option<String> {
    CAPTURED_HOTKEY.lock().unwrap().take()
}

/// Convert virtual key code and scan code to a displayable key name
fn vk_to_name(kb: &KBDLLHOOKSTRUCT) -> Option<String> {
    let vk = kb.vkCode as u16;
    if let Some(name) = common_key_name(vk) {
        return Some(name.to_string());
    }

    // Use WinAPI to resolve remaining keys, ensures Alt 组合键也能拿到文字
    let scan_code = unsafe { MapVirtualKeyW(vk.into(), MAPVK_VK_TO_VSC) };
    if scan_code == 0 {
        return None;
    }

    let mut lparam = (scan_code << 16) as i32;
    if kb.flags.contains(LLKHF_EXTENDED) {
        lparam |= 1 << 24;
    }

    let mut buffer = [0u16; 64];
    let len = unsafe { GetKeyNameTextW(lparam, &mut buffer) };
    if len > 0 {
        let mut s = String::from_utf16_lossy(&buffer[..len as usize]);
        s.retain(|c| c != '\u{0}');
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    // Last resort: return VK code so我们至少能结束录制
    Some(format!("VK{:02X}", vk))
}

/// Poll current keyboard state to detect a hotkey while capture is active
pub fn poll_hotkey_capture() -> Option<String> {
    if !HOTKEY_CAPTURE_ACTIVE.load(Ordering::SeqCst) {
        return None;
    }

    let has_ctrl = is_pressed(0x11) || is_pressed(0xA2) || is_pressed(0xA3);
    let has_alt = is_pressed(0x12) || is_pressed(0xA4) || is_pressed(0xA5);
    let has_shift = is_pressed(0x10) || is_pressed(0xA0) || is_pressed(0xA1);
    let has_win = is_pressed(0x5B) || is_pressed(0x5C);

    if !(has_ctrl || has_alt || has_shift || has_win) {
        return None;
    }

    for &vk in HOTKEY_CANDIDATES.iter() {
        if is_pressed(vk) && !is_modifier_key(vk) {
            let fake_kb = KBDLLHOOKSTRUCT {
                vkCode: vk as u32,
                scanCode: unsafe { MapVirtualKeyW(vk.into(), MAPVK_VK_TO_VSC) },
                flags: KBDLLHOOKSTRUCT_FLAGS(0),
                time: 0,
                dwExtraInfo: 0,
            };
            let name = vk_to_name(&fake_kb).unwrap_or_else(|| format!("VK{:02X}", vk));
            let mut hotkey = String::new();
            if has_ctrl { hotkey.push_str("Ctrl+"); }
            if has_alt { hotkey.push_str("Alt+"); }
            if has_shift { hotkey.push_str("Shift+"); }
            if has_win { hotkey.push_str("Win+"); }
            hotkey.push_str(&name);

            HOTKEY_CAPTURE_ACTIVE.store(false, Ordering::SeqCst);
            *CAPTURED_HOTKEY.lock().unwrap() = Some(hotkey.clone());
            log_hotkey(&format!("captured via poll {}", hotkey));
            return Some(hotkey);
        }
    }

    None
}

/// Quick check for key pressed using GetAsyncKeyState
fn is_pressed(vk: u16) -> bool {
    unsafe { ((GetAsyncKeyState(vk as i32) as u16) & 0x8000) != 0 }
}

/// Candidate keys to look for during polling capture
const HOTKEY_CANDIDATES: &[u16] = &[
    0x30,0x31,0x32,0x33,0x34,0x35,0x36,0x37,0x38,0x39, // 0-9
    0x41,0x42,0x43,0x44,0x45,0x46,0x47,0x48,0x49,0x4A,0x4B,0x4C,0x4D,0x4E,0x4F,0x50,0x51,0x52,0x53,0x54,0x55,0x56,0x57,0x58,0x59,0x5A, // A-Z
    0x70,0x71,0x72,0x73,0x74,0x75,0x76,0x77,0x78,0x79,0x7A,0x7B, // F1-F12
    0x20,0x0D,0x09,0x08,0x2E,0x2D,0x24,0x23,0x21,0x22,0x25,0x26,0x27,0x28, // specials and arrows
];

/// Common keys we want固定命名
fn common_key_name(vk: u16) -> Option<&'static str> {
    match vk {
        0x41 => Some("A"), 0x42 => Some("B"), 0x43 => Some("C"), 0x44 => Some("D"),
        0x45 => Some("E"), 0x46 => Some("F"), 0x47 => Some("G"), 0x48 => Some("H"),
        0x49 => Some("I"), 0x4A => Some("J"), 0x4B => Some("K"), 0x4C => Some("L"),
        0x4D => Some("M"), 0x4E => Some("N"), 0x4F => Some("O"), 0x50 => Some("P"),
        0x51 => Some("Q"), 0x52 => Some("R"), 0x53 => Some("S"), 0x54 => Some("T"),
        0x55 => Some("U"), 0x56 => Some("V"), 0x57 => Some("W"), 0x58 => Some("X"),
        0x59 => Some("Y"), 0x5A => Some("Z"),
        0x30 => Some("0"), 0x31 => Some("1"), 0x32 => Some("2"), 0x33 => Some("3"),
        0x34 => Some("4"), 0x35 => Some("5"), 0x36 => Some("6"), 0x37 => Some("7"),
        0x38 => Some("8"), 0x39 => Some("9"),
        0x70 => Some("F1"), 0x71 => Some("F2"), 0x72 => Some("F3"), 0x73 => Some("F4"),
        0x74 => Some("F5"), 0x75 => Some("F6"), 0x76 => Some("F7"), 0x77 => Some("F8"),
        0x78 => Some("F9"), 0x79 => Some("F10"), 0x7A => Some("F11"), 0x7B => Some("F12"),
        0x20 => Some("Space"),
        0x0D => Some("Enter"),
        0x09 => Some("Tab"),
        0x08 => Some("Backspace"),
        0x2E => Some("Delete"),
        0x2D => Some("Insert"),
        0x24 => Some("Home"),
        0x23 => Some("End"),
        0x21 => Some("PageUp"),
        0x22 => Some("PageDown"),
        0x25 => Some("Left"),
        0x26 => Some("Up"),
        0x27 => Some("Right"),
        0x28 => Some("Down"),
        _ => None,
    }
}

/// Check if a key is a modifier key
fn is_modifier_key(vk: u16) -> bool {
    matches!(vk,
        0x10 | 0x11 | 0x12 |
        0xA0 | 0xA1 |
        0xA2 | 0xA3 |
        0xA4 | 0xA5 |
        0x5B | 0x5C
    )
}

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
        let msg = wparam.0 as u32;
        let is_keydown = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let is_keyup = msg == WM_KEYUP || msg == WM_SYSKEYUP;

        // Handle hotkey capture mode
        if HOTKEY_CAPTURE_ACTIVE.load(Ordering::SeqCst) {
            // Trace every event while active
            log_hotkey(&format!(
                "event vk {:X} msg {} ctrl={} alt={} shift={} win={}",
                vk_code,
                msg,
                HOTKEY_CAPTURE_CTRL.load(Ordering::SeqCst),
                HOTKEY_CAPTURE_ALT.load(Ordering::SeqCst),
                HOTKEY_CAPTURE_SHIFT.load(Ordering::SeqCst),
                HOTKEY_CAPTURE_WIN.load(Ordering::SeqCst),
            ));

            // Track modifier states
            match vk_code {
                0x10 | 0xA0 | 0xA1 => { // VK_SHIFT, VK_LSHIFT, VK_RSHIFT
                    if is_keydown || is_keyup {
                        HOTKEY_CAPTURE_SHIFT.store(is_keydown, Ordering::SeqCst);
                        log_hotkey(&format!("shift {}", if is_keydown { "down" } else { "up" }));
                    }
                }
                0x11 | 0xA2 | 0xA3 => { // VK_CONTROL, VK_LCONTROL, VK_RCONTROL
                    if is_keydown || is_keyup {
                        HOTKEY_CAPTURE_CTRL.store(is_keydown, Ordering::SeqCst);
                        log_hotkey(&format!("ctrl {}", if is_keydown { "down" } else { "up" }));
                    }
                }
                0x12 | 0xA4 | 0xA5 => { // VK_MENU (Alt), VK_LMENU, VK_RMENU
                    if is_keydown || is_keyup {
                        HOTKEY_CAPTURE_ALT.store(is_keydown, Ordering::SeqCst);
                        log_hotkey(&format!("alt {}", if is_keydown { "down" } else { "up" }));
                    }
                }
                0x5B | 0x5C => { // VK_LWIN, VK_RWIN
                    if is_keydown || is_keyup {
                        HOTKEY_CAPTURE_WIN.store(is_keydown, Ordering::SeqCst);
                        log_hotkey(&format!("win {}", if is_keydown { "down" } else { "up" }));
                    }
                }
                _ => {}
            }

            // Check for Escape to cancel
            if is_keydown && vk_code == VK_ESCAPE.0 {
                HOTKEY_CAPTURE_ACTIVE.store(false, Ordering::SeqCst);
                *CAPTURED_HOTKEY.lock().unwrap() = Some(String::new()); // Empty string signals cancel
                log_hotkey("cancel capture (Esc)");
                return LRESULT(1); // Block the key
            }

            // If a non-modifier key is pressed with at least one modifier
            if is_keydown && !is_modifier_key(vk_code) {
                // Ignore Tab作为热键，避免 Alt+Tab 被误捕获
                if vk_code == VK_TAB.0 {
                    log_hotkey("skip Tab key");
                    return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
                }

                let has_ctrl = HOTKEY_CAPTURE_CTRL.load(Ordering::SeqCst);
                let has_alt = HOTKEY_CAPTURE_ALT.load(Ordering::SeqCst);
                let has_shift = HOTKEY_CAPTURE_SHIFT.load(Ordering::SeqCst);
                let has_win = HOTKEY_CAPTURE_WIN.load(Ordering::SeqCst);

                // Need at least one modifier
                if has_ctrl || has_alt || has_shift || has_win {
                    let mut hotkey = String::new();
                    if has_ctrl { hotkey.push_str("Ctrl+"); }
                    if has_alt { hotkey.push_str("Alt+"); }
                    if has_shift { hotkey.push_str("Shift+"); }
                    if has_win { hotkey.push_str("Win+"); }

                    if let Some(key_name) = vk_to_name(kb_struct) {
                        hotkey.push_str(&key_name);
                        *CAPTURED_HOTKEY.lock().unwrap() = Some(hotkey.clone());
                        HOTKEY_CAPTURE_ACTIVE.store(false, Ordering::SeqCst);
                        log_hotkey(&format!("captured {}", hotkey));
                        return LRESULT(1); // Block the key
                    } else {
                        log_hotkey(&format!(
                            "name lookup failed for vk {:X}, sc {:X}",
                            vk_code,
                            unsafe { MapVirtualKeyW(vk_code.into(), MAPVK_VK_TO_VSC) }
                        ));
                    }
                } else {
                    log_hotkey(&format!("ignored {} without modifier", vk_code));
                }
            }

            // Log activity when capture is active
            if is_keydown {
                log_hotkey(&format!(
                    "keydown vk {:X} ctrl={} alt={} shift={} win={}",
                    vk_code,
                    HOTKEY_CAPTURE_CTRL.load(Ordering::SeqCst),
                    HOTKEY_CAPTURE_ALT.load(Ordering::SeqCst),
                    HOTKEY_CAPTURE_SHIFT.load(Ordering::SeqCst),
                    HOTKEY_CAPTURE_WIN.load(Ordering::SeqCst),
                ));
            }
        }

        // Track Ctrl key state for Ctrl+V detection
        if vk_code == VK_CONTROL.0 || vk_code == 0xA2 || vk_code == 0xA3 {
            CTRL_PRESSED.store(is_keydown, Ordering::SeqCst);
        }

        // Detect Ctrl+V
        if is_keydown && vk_code == VK_V.0 {
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
                    let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
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

/// Append a simple line to hotkey debug log (best-effort, ignore errors)
fn log_hotkey(msg: &str) {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    if let Some(mut path) = dirs::config_dir() {
        path.push("NanoTrans");
        let _ = std::fs::create_dir_all(&path);
        path.push("hotkey.log");
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "[{}] {}", ts, msg);
        }
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
