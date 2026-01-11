//! Global hotkey registration and handling
//! Uses global-hotkey crate for cross-platform hotkey support

use anyhow::Result;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};

/// Default hotkey: Alt + Q
pub const DEFAULT_HOTKEY: &str = "Alt+Q";

/// Hotkey manager wrapper
pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    translate_hotkey: HotKey,
    translate_hotkey_id: u32,
    current_hotkey: String,
}

impl HotkeyManager {
    /// Create a new hotkey manager with the specified hotkey string
    pub fn new(hotkey_str: &str) -> Result<Self> {
        let manager = GlobalHotKeyManager::new()?;

        let hotkey = parse_hotkey(hotkey_str)?;
        let hotkey_id = hotkey.id();

        manager.register(hotkey)?;

        Ok(Self {
            manager,
            translate_hotkey: hotkey,
            translate_hotkey_id: hotkey_id,
            current_hotkey: hotkey_str.to_lowercase(),
        })
    }

    /// Check if the event matches our translate hotkey
    pub fn is_translate_hotkey(&self, event: &GlobalHotKeyEvent) -> bool {
        event.id == self.translate_hotkey_id
    }

    /// Update the hotkey binding
    pub fn update_hotkey(&mut self, hotkey_str: &str) -> Result<()> {
        let normalized = hotkey_str.to_lowercase();
        // Already bound, skip churn
        if normalized == self.current_hotkey {
            return Ok(());
        }

        let new_hotkey = parse_hotkey(hotkey_str)?;
        // Register new first to avoid losing old binding on failure
        self.manager.register(new_hotkey)?;
        // Safe to drop old one now
        self.manager.unregister(self.translate_hotkey)?;

        self.translate_hotkey_id = new_hotkey.id();
        self.translate_hotkey = new_hotkey;
        self.current_hotkey = normalized;

        Ok(())
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        let _ = self.manager.unregister(self.translate_hotkey);
    }
}

/// Parse a hotkey string like "Alt+Q" or "Ctrl+Shift+T" into a HotKey
pub fn parse_hotkey(hotkey_str: &str) -> Result<HotKey> {
    let parts: Vec<&str> = hotkey_str.split('+').map(|s| s.trim()).collect();

    if parts.is_empty() {
        anyhow::bail!("Empty hotkey string");
    }

    let mut modifiers = Modifiers::empty();
    let mut key_code: Option<Code> = None;

    for part in parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "alt" => modifiers |= Modifiers::ALT,
            "shift" => modifiers |= Modifiers::SHIFT,
            "win" | "super" | "meta" => modifiers |= Modifiers::META,
            key => {
                key_code = Some(parse_key_code(key)?);
            }
        }
    }

    if modifiers.is_empty() {
        anyhow::bail!("Hotkey must include at least one modifier");
    }

    let code = key_code.ok_or_else(|| anyhow::anyhow!("No key specified in hotkey"))?;

    Ok(HotKey::new(Some(modifiers), code))
}

/// Parse a single key code string
fn parse_key_code(key: &str) -> Result<Code> {
    let code = match key.to_lowercase().as_str() {
        // Letters
        "a" => Code::KeyA,
        "b" => Code::KeyB,
        "c" => Code::KeyC,
        "d" => Code::KeyD,
        "e" => Code::KeyE,
        "f" => Code::KeyF,
        "g" => Code::KeyG,
        "h" => Code::KeyH,
        "i" => Code::KeyI,
        "j" => Code::KeyJ,
        "k" => Code::KeyK,
        "l" => Code::KeyL,
        "m" => Code::KeyM,
        "n" => Code::KeyN,
        "o" => Code::KeyO,
        "p" => Code::KeyP,
        "q" => Code::KeyQ,
        "r" => Code::KeyR,
        "s" => Code::KeyS,
        "t" => Code::KeyT,
        "u" => Code::KeyU,
        "v" => Code::KeyV,
        "w" => Code::KeyW,
        "x" => Code::KeyX,
        "y" => Code::KeyY,
        "z" => Code::KeyZ,

        // Numbers
        "0" => Code::Digit0,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,

        // Function keys
        "f1" => Code::F1,
        "f2" => Code::F2,
        "f3" => Code::F3,
        "f4" => Code::F4,
        "f5" => Code::F5,
        "f6" => Code::F6,
        "f7" => Code::F7,
        "f8" => Code::F8,
        "f9" => Code::F9,
        "f10" => Code::F10,
        "f11" => Code::F11,
        "f12" => Code::F12,

        // Special keys
        "space" => Code::Space,
        "enter" | "return" => Code::Enter,
        "tab" => Code::Tab,
        "escape" | "esc" => Code::Escape,
        "backspace" => Code::Backspace,
        "delete" | "del" => Code::Delete,
        "insert" | "ins" => Code::Insert,
        "home" => Code::Home,
        "end" => Code::End,
        "pageup" | "pgup" => Code::PageUp,
        "pagedown" | "pgdn" => Code::PageDown,

        // Arrow keys
        "up" => Code::ArrowUp,
        "down" => Code::ArrowDown,
        "left" => Code::ArrowLeft,
        "right" => Code::ArrowRight,

        _ => anyhow::bail!("Unknown key: {}", key),
    };

    Ok(code)
}

/// Get the global hotkey event receiver
pub fn hotkey_event_receiver() -> crossbeam_channel::Receiver<GlobalHotKeyEvent> {
    GlobalHotKeyEvent::receiver().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hotkey() {
        let hotkey = parse_hotkey("Alt+Q").unwrap();
        assert!(hotkey.id() > 0);

        let hotkey2 = parse_hotkey("Ctrl+Shift+T").unwrap();
        assert!(hotkey2.id() > 0);
    }

    #[test]
    fn test_parse_key_code() {
        assert!(parse_key_code("a").is_ok());
        assert!(parse_key_code("F1").is_ok());
        assert!(parse_key_code("space").is_ok());
        assert!(parse_key_code("invalid").is_err());
    }
}
