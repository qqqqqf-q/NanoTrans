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
#[cfg(target_os = "macos")]
static ACTIVE_HOTKEY: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));
#[cfg(target_os = "macos")]
static HOTKEY_EVENT_CHANNEL: Lazy<(
    crossbeam_channel::Sender<()>,
    crossbeam_channel::Receiver<()>,
)> = Lazy::new(|| crossbeam_channel::unbounded());
#[cfg(target_os = "macos")]
static MONITOR_ERROR_CHANNEL: Lazy<(
    crossbeam_channel::Sender<String>,
    crossbeam_channel::Receiver<String>,
)> = Lazy::new(|| crossbeam_channel::unbounded());
#[cfg(target_os = "macos")]
static MONITOR_ERROR_REPORTED: AtomicBool = AtomicBool::new(false);

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

#[cfg(target_os = "macos")]
pub fn set_active_hotkey(hotkey: &str) -> anyhow::Result<()> {
    let normalized = normalize_hotkey_string(hotkey)?;
    *ACTIVE_HOTKEY.lock().unwrap() = Some(normalized);
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn hotkey_event_receiver() -> crossbeam_channel::Receiver<()> {
    HOTKEY_EVENT_CHANNEL.1.clone()
}

#[cfg(target_os = "macos")]
pub fn keyboard_monitor_error_receiver() -> crossbeam_channel::Receiver<String> {
    MONITOR_ERROR_CHANNEL.1.clone()
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

#[cfg(target_os = "macos")]
fn normalize_hotkey_string(hotkey: &str) -> anyhow::Result<String> {
    let mut has_cmd = false;
    let mut has_ctrl = false;
    let mut has_alt = false;
    let mut has_shift = false;
    let mut key_name: Option<&'static str> = None;

    for part in hotkey.split('+') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.to_lowercase().as_str() {
            "cmd" | "command" => has_cmd = true,
            "ctrl" | "control" => has_ctrl = true,
            "alt" | "option" | "opt" => has_alt = true,
            "shift" => has_shift = true,
            key => {
                if key_name.is_some() {
                    anyhow::bail!("Hotkey contains multiple main keys");
                }
                key_name = normalize_key_name(key);
                if key_name.is_none() {
                    anyhow::bail!("Unknown key: {}", part);
                }
            }
        }
    }

    if key_name.is_none() {
        anyhow::bail!("Hotkey missing main key");
    }
    if !(has_cmd || has_ctrl || has_alt || has_shift) {
        anyhow::bail!("Hotkey must include at least one modifier");
    }

    let mut out = String::new();
    if has_cmd { out.push_str("Cmd+"); }
    if has_ctrl { out.push_str("Ctrl+"); }
    if has_alt { out.push_str("Alt+"); }
    if has_shift { out.push_str("Shift+"); }
    out.push_str(key_name.unwrap());
    Ok(out)
}

#[cfg(target_os = "macos")]
fn report_keyboard_monitor_error(message: &str) {
    if MONITOR_ERROR_REPORTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = MONITOR_ERROR_CHANNEL.0.send(message.to_string());
}

#[cfg(target_os = "macos")]
fn normalize_key_name(key: &str) -> Option<&'static str> {
    match key.to_lowercase().as_str() {
        "a" => Some("A"), "b" => Some("B"), "c" => Some("C"), "d" => Some("D"),
        "e" => Some("E"), "f" => Some("F"), "g" => Some("G"), "h" => Some("H"),
        "i" => Some("I"), "j" => Some("J"), "k" => Some("K"), "l" => Some("L"),
        "m" => Some("M"), "n" => Some("N"), "o" => Some("O"), "p" => Some("P"),
        "q" => Some("Q"), "r" => Some("R"), "s" => Some("S"), "t" => Some("T"),
        "u" => Some("U"), "v" => Some("V"), "w" => Some("W"), "x" => Some("X"),
        "y" => Some("Y"), "z" => Some("Z"),
        "0" => Some("0"), "1" => Some("1"), "2" => Some("2"), "3" => Some("3"),
        "4" => Some("4"), "5" => Some("5"), "6" => Some("6"), "7" => Some("7"),
        "8" => Some("8"), "9" => Some("9"),
        "f1" => Some("F1"), "f2" => Some("F2"), "f3" => Some("F3"),
        "f4" => Some("F4"), "f5" => Some("F5"), "f6" => Some("F6"),
        "f7" => Some("F7"), "f8" => Some("F8"), "f9" => Some("F9"),
        "f10" => Some("F10"), "f11" => Some("F11"), "f12" => Some("F12"),
        "space" | "spacebar" => Some("Space"),
        "enter" | "return" => Some("Enter"),
        "tab" => Some("Tab"),
        "escape" | "esc" => Some("Escape"),
        "backspace" => Some("Backspace"),
        "delete" | "del" => Some("Delete"),
        "home" => Some("Home"),
        "end" => Some("End"),
        "pageup" | "pgup" => Some("PageUp"),
        "pagedown" | "pgdn" => Some("PageDown"),
        "left" => Some("Left"),
        "right" => Some("Right"),
        "up" => Some("Up"),
        "down" => Some("Down"),
        _ => None,
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
                let module = windows::Win32::System::LibraryLoader::GetModuleHandleW(None);
                let hook = SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(keyboard_hook_proc),
                    module.unwrap_or_default(),
                    0,
                );
                if hook.is_ok() {
                    let mut msg = std::mem::zeroed();
                    while windows::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                        windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
                    }
                } else {
                    super::log_hotkey("keyboard hook failed");
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
    use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
    use core_graphics::event::{
        CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions,
        CGEventTapPlacement, CGEventType, EventField,
    };
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    pub fn poll_hotkey_capture() -> Option<String> {
        None
    }

    fn is_modifier_key(keycode: u16) -> bool {
        matches!(
            keycode,
            54 | 55 | 56 | 60 | 57 | 58 | 61 | 59 | 62 | 63
        )
    }

    fn keycode_to_name(keycode: u16) -> Option<&'static str> {
        match keycode {
            0 => Some("A"),
            1 => Some("S"),
            2 => Some("D"),
            3 => Some("F"),
            4 => Some("H"),
            5 => Some("G"),
            6 => Some("Z"),
            7 => Some("X"),
            8 => Some("C"),
            9 => Some("V"),
            11 => Some("B"),
            12 => Some("Q"),
            13 => Some("W"),
            14 => Some("E"),
            15 => Some("R"),
            16 => Some("Y"),
            17 => Some("T"),
            18 => Some("1"),
            19 => Some("2"),
            20 => Some("3"),
            21 => Some("4"),
            22 => Some("6"),
            23 => Some("5"),
            25 => Some("9"),
            26 => Some("7"),
            28 => Some("8"),
            29 => Some("0"),
            31 => Some("O"),
            32 => Some("U"),
            34 => Some("I"),
            35 => Some("P"),
            36 => Some("Enter"),
            37 => Some("L"),
            38 => Some("J"),
            40 => Some("K"),
            45 => Some("N"),
            46 => Some("M"),
            48 => Some("Tab"),
            49 => Some("Space"),
            51 => Some("Backspace"),
            53 => Some("Escape"),
            96 => Some("F5"),
            97 => Some("F6"),
            98 => Some("F7"),
            99 => Some("F3"),
            100 => Some("F8"),
            101 => Some("F9"),
            103 => Some("F11"),
            109 => Some("F10"),
            111 => Some("F12"),
            115 => Some("Home"),
            116 => Some("PageUp"),
            117 => Some("Delete"),
            118 => Some("F4"),
            119 => Some("End"),
            120 => Some("F2"),
            121 => Some("PageDown"),
            122 => Some("F1"),
            123 => Some("Left"),
            124 => Some("Right"),
            125 => Some("Down"),
            126 => Some("Up"),
            _ => None,
        }
    }

    pub fn start_keyboard_monitor() {
        thread::spawn(|| {
            let tap = CGEventTap::new(
                CGEventTapLocation::Session,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::ListenOnly,
                vec![CGEventType::KeyDown],
                |_proxy, _event_type, event| {
                    let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                    let flags = event.get_flags();
                    let capture_active = super::HOTKEY_CAPTURE_ACTIVE.load(Ordering::SeqCst);

                    if capture_active {
                        if keycode == 53 {
                            super::HOTKEY_CAPTURE_ACTIVE.store(false, Ordering::SeqCst);
                            *super::CAPTURED_HOTKEY.lock().unwrap() = Some(String::new());
                            super::log_hotkey("cancel capture (Esc)");
                            return None;
                        }

                        if !is_modifier_key(keycode) {
                            let has_cmd = flags.contains(CGEventFlags::CGEventFlagCommand);
                            let has_ctrl = flags.contains(CGEventFlags::CGEventFlagControl);
                            let has_alt = flags.contains(CGEventFlags::CGEventFlagAlternate);
                            let has_shift = flags.contains(CGEventFlags::CGEventFlagShift);

                            if has_cmd || has_ctrl || has_alt || has_shift {
                                if let Some(key_name) = keycode_to_name(keycode) {
                                    let mut hotkey = String::new();
                                    if has_cmd { hotkey.push_str("Cmd+"); }
                                    if has_ctrl { hotkey.push_str("Ctrl+"); }
                                    if has_alt { hotkey.push_str("Alt+"); }
                                    if has_shift { hotkey.push_str("Shift+"); }
                                    hotkey.push_str(key_name);

                                    super::HOTKEY_CAPTURE_ACTIVE.store(false, Ordering::SeqCst);
                                    *super::CAPTURED_HOTKEY.lock().unwrap() = Some(hotkey.clone());
                                    super::log_hotkey(&format!("captured {}", hotkey));
                                }
                            }
                        }
                    } else {
                        let active = super::ACTIVE_HOTKEY.lock().unwrap();
                        if let Some(active_hotkey) = active.as_deref() {
                            if !is_modifier_key(keycode) {
                                let has_cmd = flags.contains(CGEventFlags::CGEventFlagCommand);
                                let has_ctrl = flags.contains(CGEventFlags::CGEventFlagControl);
                                let has_alt = flags.contains(CGEventFlags::CGEventFlagAlternate);
                                let has_shift = flags.contains(CGEventFlags::CGEventFlagShift);

                                if has_cmd || has_ctrl || has_alt || has_shift {
                                    if let Some(key_name) = keycode_to_name(keycode) {
                                        let mut hotkey = String::new();
                                        if has_cmd { hotkey.push_str("Cmd+"); }
                                        if has_ctrl { hotkey.push_str("Ctrl+"); }
                                        if has_alt { hotkey.push_str("Alt+"); }
                                        if has_shift { hotkey.push_str("Shift+"); }
                                        hotkey.push_str(key_name);
                                        if hotkey == active_hotkey {
                                            let _ = super::HOTKEY_EVENT_CHANNEL.0.send(());
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if keycode == 9 {
                        if flags.contains(CGEventFlags::CGEventFlagCommand)
                            || flags.contains(CGEventFlags::CGEventFlagControl)
                        {
                            super::CTRL_V_DETECTED.store(true, Ordering::SeqCst);
                        }
                    }
                    None
                },
            );

            let tap = match tap {
                Ok(tap) => tap,
                Err(err) => {
                    let message = format!("keyboard monitor failed: {:?}", err);
                    super::log_hotkey(&message);
                    super::report_keyboard_monitor_error(&message);
                    return;
                }
            };

            let loop_source = match tap.mach_port.create_runloop_source(0) {
                Ok(source) => source,
                Err(err) => {
                    let message = format!("keyboard runloop source failed: {:?}", err);
                    super::log_hotkey(&message);
                    super::report_keyboard_monitor_error(&message);
                    return;
                }
            };

            let current = CFRunLoop::get_current();
            unsafe {
                current.add_source(&loop_source, kCFRunLoopCommonModes);
            }
            tap.enable();
            CFRunLoop::run_current();
        });
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
