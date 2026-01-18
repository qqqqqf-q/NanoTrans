//! Cross-platform keyboard input simulation and monitoring
//! Windows: Uses SendInput API and Low-Level Keyboard Hook
//! macOS: Uses CGEvent APIs

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use once_cell::sync::Lazy;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

const KEY_DELAY_MS: u64 = 10;

static CTRL_V_DETECTED: AtomicBool = AtomicBool::new(false);
static HOTKEY_CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);
static CAPTURED_HOTKEY: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

pub fn start_hotkey_capture() {
    HOTKEY_CAPTURE_ACTIVE.store(true, Ordering::SeqCst);
    *CAPTURED_HOTKEY.lock().unwrap() = None;
    log_hotkey("start capture");
}

pub fn stop_hotkey_capture() {
    HOTKEY_CAPTURE_ACTIVE.store(false, Ordering::SeqCst);
    log_hotkey("stop capture");
}

pub fn get_captured_hotkey() -> Option<String> {
    CAPTURED_HOTKEY.lock().unwrap().take()
}

pub fn check_ctrl_v_pressed() -> bool {
    CTRL_V_DETECTED.swap(false, Ordering::SeqCst)
}

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

// Windows 实现
#[cfg(target_os = "windows")]
mod platform_impl {
    use super::*;
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

    static CTRL_PRESSED: AtomicBool = AtomicBool::new(false);
    static HOTKEY_CAPTURE_CTRL: AtomicBool = AtomicBool::new(false);
    static HOTKEY_CAPTURE_ALT: AtomicBool = AtomicBool::new(false);
    static HOTKEY_CAPTURE_SHIFT: AtomicBool = AtomicBool::new(false);
    static HOTKEY_CAPTURE_WIN: AtomicBool = AtomicBool::new(false);

    pub fn poll_hotkey_capture() -> Option<String> {
        if !super::HOTKEY_CAPTURE_ACTIVE.load(Ordering::SeqCst) {
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

                super::HOTKEY_CAPTURE_ACTIVE.store(false, Ordering::SeqCst);
                *super::CAPTURED_HOTKEY.lock().unwrap() = Some(hotkey.clone());
                super::log_hotkey(&format!("captured via poll {}", hotkey));
                return Some(hotkey);
            }
        }

        None
    }

    fn is_pressed(vk: u16) -> bool {
        unsafe { ((GetAsyncKeyState(vk as i32) as u16) & 0x8000) != 0 }
    }

    const HOTKEY_CANDIDATES: &[u16] = &[
        0x30,0x31,0x32,0x33,0x34,0x35,0x36,0x37,0x38,0x39,
        0x41,0x42,0x43,0x44,0x45,0x46,0x47,0x48,0x49,0x4A,0x4B,0x4C,0x4D,0x4E,0x4F,0x50,0x51,0x52,0x53,0x54,0x55,0x56,0x57,0x58,0x59,0x5A,
        0x70,0x71,0x72,0x73,0x74,0x75,0x76,0x77,0x78,0x79,0x7A,0x7B,
        0x20,0x0D,0x09,0x08,0x2E,0x2D,0x24,0x23,0x21,0x22,0x25,0x26,0x27,0x28,
    ];

    fn vk_to_name(kb: &KBDLLHOOKSTRUCT) -> Option<String> {
        let vk = kb.vkCode as u16;
        if let Some(name) = common_key_name(vk) {
            return Some(name.to_string());
        }

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

        Some(format!("VK{:02X}", vk))
    }

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
            0x20 => Some("Space"), 0x0D => Some("Enter"), 0x09 => Some("Tab"),
            0x08 => Some("Backspace"), 0x2E => Some("Delete"), 0x2D => Some("Insert"),
            0x24 => Some("Home"), 0x23 => Some("End"), 0x21 => Some("PageUp"),
            0x22 => Some("PageDown"), 0x25 => Some("Left"), 0x26 => Some("Up"),
            0x27 => Some("Right"), 0x28 => Some("Down"),
            _ => None,
        }
    }

    fn is_modifier_key(vk: u16) -> bool {
        matches!(vk, 0x10 | 0x11 | 0x12 | 0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 | 0xA5 | 0x5B | 0x5C)
    }

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

            if super::HOTKEY_CAPTURE_ACTIVE.load(Ordering::SeqCst) {
                match vk_code {
                    0x10 | 0xA0 | 0xA1 => {
                        if is_keydown || is_keyup {
                            HOTKEY_CAPTURE_SHIFT.store(is_keydown, Ordering::SeqCst);
                        }
                    }
                    0x11 | 0xA2 | 0xA3 => {
                        if is_keydown || is_keyup {
                            HOTKEY_CAPTURE_CTRL.store(is_keydown, Ordering::SeqCst);
                        }
                    }
                    0x12 | 0xA4 | 0xA5 => {
                        if is_keydown || is_keyup {
                            HOTKEY_CAPTURE_ALT.store(is_keydown, Ordering::SeqCst);
                        }
                    }
                    0x5B | 0x5C => {
                        if is_keydown || is_keyup {
                            HOTKEY_CAPTURE_WIN.store(is_keydown, Ordering::SeqCst);
                        }
                    }
                    _ => {}
                }

                if is_keydown && vk_code == VK_ESCAPE.0 {
                    super::HOTKEY_CAPTURE_ACTIVE.store(false, Ordering::SeqCst);
                    *super::CAPTURED_HOTKEY.lock().unwrap() = Some(String::new());
                    super::log_hotkey("cancel capture (Esc)");
                    return LRESULT(1);
                }

                if is_keydown && !is_modifier_key(vk_code) {
                    if vk_code == VK_TAB.0 {
                        return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
                    }

                    let has_ctrl = HOTKEY_CAPTURE_CTRL.load(Ordering::SeqCst);
                    let has_alt = HOTKEY_CAPTURE_ALT.load(Ordering::SeqCst);
                    let has_shift = HOTKEY_CAPTURE_SHIFT.load(Ordering::SeqCst);
                    let has_win = HOTKEY_CAPTURE_WIN.load(Ordering::SeqCst);

                    if has_ctrl || has_alt || has_shift || has_win {
                        let mut hotkey = String::new();
                        if has_ctrl { hotkey.push_str("Ctrl+"); }
                        if has_alt { hotkey.push_str("Alt+"); }
                        if has_shift { hotkey.push_str("Shift+"); }
                        if has_win { hotkey.push_str("Win+"); }

                        if let Some(key_name) = vk_to_name(kb_struct) {
                            hotkey.push_str(&key_name);
                            *super::CAPTURED_HOTKEY.lock().unwrap() = Some(hotkey.clone());
                            super::HOTKEY_CAPTURE_ACTIVE.store(false, Ordering::SeqCst);
                            super::log_hotkey(&format!("captured {}", hotkey));
                            return LRESULT(1);
                        }
                    }
                }
            }

            if vk_code == VK_CONTROL.0 || vk_code == 0xA2 || vk_code == 0xA3 {
                CTRL_PRESSED.store(is_keydown, Ordering::SeqCst);
            }

            if is_keydown && vk_code == VK_V.0 {
                if CTRL_PRESSED.load(Ordering::SeqCst) {
                    super::CTRL_V_DETECTED.store(true, Ordering::SeqCst);
                }
            }
        }

        CallNextHookEx(HHOOK::default(), code, wparam, lparam)
    }

    pub fn start_keyboard_monitor() {
        thread::spawn(|| {
            unsafe {
                let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0);
                if hook.is_ok() {
                    let mut msg = std::mem::zeroed();
                    while windows::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                        windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
                    }
                }
            }
        });
    }

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

    fn send_inputs(inputs: &[INPUT]) {
        unsafe {
            SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    pub fn send_ctrl_c() {
        let inputs = [
            create_key_input(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
            create_key_input(VK_C, KEYBD_EVENT_FLAGS(0)),
            create_key_input(VK_C, KEYEVENTF_KEYUP),
            create_key_input(VK_CONTROL, KEYEVENTF_KEYUP),
        ];
        send_inputs(&inputs);
        thread::sleep(Duration::from_millis(KEY_DELAY_MS));
    }

    pub fn send_ctrl_v() {
        let inputs = [
            create_key_input(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
            create_key_input(VK_V, KEYBD_EVENT_FLAGS(0)),
            create_key_input(VK_V, KEYEVENTF_KEYUP),
            create_key_input(VK_CONTROL, KEYEVENTF_KEYUP),
        ];
        send_inputs(&inputs);
        thread::sleep(Duration::from_millis(KEY_DELAY_MS));
    }
}

// macOS 实现
#[cfg(target_os = "macos")]
mod platform_impl {
    use super::*;
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    pub fn poll_hotkey_capture() -> Option<String> {
        // macOS 下暂不支持轮询式热键捕获，返回 None
        None
    }

    pub fn start_keyboard_monitor() {
        // macOS 下键盘监控需要 Accessibility 权限
        // 这里提供简化实现，实际使用时需要用户授权
    }

    pub fn send_ctrl_c() {
        send_key_combo(8, CGEventFlags::CGEventFlagCommand);
    }

    pub fn send_ctrl_v() {
        send_key_combo(9, CGEventFlags::CGEventFlagCommand);
    }

    fn send_key_combo(keycode: u16, flags: CGEventFlags) {
        if let Ok(source) = CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
            if let Ok(event_down) = CGEvent::new_keyboard_event(source.clone(), keycode, true) {
                event_down.set_flags(flags);
                event_down.post(CGEventTapLocation::HID);
            }

            thread::sleep(Duration::from_millis(KEY_DELAY_MS));

            if let Ok(event_up) = CGEvent::new_keyboard_event(source, keycode, false) {
                event_up.post(CGEventTapLocation::HID);
            }

            thread::sleep(Duration::from_millis(KEY_DELAY_MS));
        }
    }
}

// 公共接口
pub fn poll_hotkey_capture() -> Option<String> {
    platform_impl::poll_hotkey_capture()
}

pub fn start_keyboard_monitor() {
    platform_impl::start_keyboard_monitor();
}

pub fn send_ctrl_c() {
    platform_impl::send_ctrl_c();
}

pub fn send_ctrl_v() {
    platform_impl::send_ctrl_v();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hotkey_capture() {
        start_hotkey_capture();
        assert!(HOTKEY_CAPTURE_ACTIVE.load(Ordering::SeqCst));
        stop_hotkey_capture();
        assert!(!HOTKEY_CAPTURE_ACTIVE.load(Ordering::SeqCst));
    }
}
