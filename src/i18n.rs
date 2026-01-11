//! Internationalization (I18N) support
//! Provides UI text translations for Chinese and English

use crate::config::UILanguage;
use once_cell::sync::Lazy;
use std::sync::RwLock;

/// Current active language
static CURRENT_LANG: Lazy<RwLock<Lang>> = Lazy::new(|| RwLock::new(Lang::En));

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lang {
    En,
    Zh,
}

/// All translatable UI strings
pub struct Texts {
    // Settings window
    pub settings_title: &'static str,
    pub global_hotkey: &'static str,
    pub hotkey_placeholder: &'static str,
    pub hotkey_recording: &'static str,
    pub translation_provider: &'static str,
    pub provider_settings: &'static str,
    pub google_no_config: &'static str,
    pub deepl_settings: &'static str,
    pub api_key: &'static str,
    pub api_key_placeholder: &'static str,
    pub deepl_hint: &'static str,
    pub api_settings: &'static str,
    pub api_base_url: &'static str,
    pub model: &'static str,
    pub model_placeholder: &'static str,
    pub cancel: &'static str,
    pub save: &'static str,
    pub ui_language: &'static str,

    // Popup window
    pub provider_label: &'static str,
    pub translating: &'static str,
    pub copy: &'static str,
    pub apply: &'static str,
    pub hint_apply: &'static str,

    // Tray menu
    pub tray_settings: &'static str,
    pub tray_exit: &'static str,
}

const TEXTS_EN: Texts = Texts {
    settings_title: "Settings",
    global_hotkey: "Global Hotkey",
    hotkey_placeholder: "Click and press keys...",
    hotkey_recording: "Press hotkey...",
    translation_provider: "Translation Provider",
    provider_settings: "Provider Settings",
    google_no_config: "Google Translate - no config needed",
    deepl_settings: "DeepL Settings",
    api_key: "API Key",
    api_key_placeholder: "Enter your API key",
    deepl_hint: "Get your free API key at deepl.com/pro-api",
    api_settings: "API Settings",
    api_base_url: "API Base URL",
    model: "Model",
    model_placeholder: "e.g., gpt-4o-mini",
    cancel: "Cancel",
    save: "Save",
    ui_language: "UI Language",

    provider_label: "Provider:",
    translating: "Translating...",
    copy: "Copy",
    apply: "Apply",
    hint_apply: "Click result or press Enter to apply",

    tray_settings: "Settings",
    tray_exit: "Exit",
};

const TEXTS_ZH: Texts = Texts {
    settings_title: "设置",
    global_hotkey: "全局快捷键",
    hotkey_placeholder: "点击后按下快捷键...",
    hotkey_recording: "请按下快捷键...",
    translation_provider: "翻译服务",
    provider_settings: "服务设置",
    google_no_config: "Google 翻译 - 无需配置",
    deepl_settings: "DeepL 设置",
    api_key: "API 密钥",
    api_key_placeholder: "输入您的 API 密钥",
    deepl_hint: "在 deepl.com/pro-api 获取免费密钥",
    api_settings: "API 设置",
    api_base_url: "API 地址",
    model: "模型",
    model_placeholder: "例如 gpt-4o-mini",
    cancel: "取消",
    save: "保存",
    ui_language: "界面语言",

    provider_label: "服务:",
    translating: "翻译中...",
    copy: "复制",
    apply: "应用",
    hint_apply: "点击结果或按回车应用",

    tray_settings: "设置",
    tray_exit: "退出",
};

/// Initialize language from config
pub fn init(ui_lang: &UILanguage) {
    let lang = match ui_lang {
        UILanguage::En => Lang::En,
        UILanguage::Zh => Lang::Zh,
        UILanguage::Auto => detect_system_language(),
    };
    set_language(lang);
}

/// Detect system language
fn detect_system_language() -> Lang {
    #[cfg(windows)]
    {
        use windows::Win32::Globalization::GetUserDefaultUILanguage;
        let lang_id = unsafe { GetUserDefaultUILanguage() };
        // Chinese: 0x0804 (Simplified), 0x0404 (Traditional)
        if lang_id == 0x0804 || lang_id == 0x0404 || (lang_id & 0xFF) == 0x04 {
            return Lang::Zh;
        }
    }
    Lang::En
}

/// Set current language
pub fn set_language(lang: Lang) {
    if let Ok(mut current) = CURRENT_LANG.write() {
        *current = lang;
    }
}

/// Get current language
pub fn current_language() -> Lang {
    CURRENT_LANG.read().map(|l| *l).unwrap_or(Lang::En)
}

/// Get translated texts for current language
pub fn t() -> &'static Texts {
    match current_language() {
        Lang::En => &TEXTS_EN,
        Lang::Zh => &TEXTS_ZH,
    }
}

/// Get language index for UI (0=Auto, 1=English, 2=Chinese)
pub fn language_to_index(lang: &UILanguage) -> i32 {
    match lang {
        UILanguage::Auto => 0,
        UILanguage::En => 1,
        UILanguage::Zh => 2,
    }
}

/// Get UILanguage from index
pub fn index_to_language(index: i32) -> UILanguage {
    match index {
        1 => UILanguage::En,
        2 => UILanguage::Zh,
        _ => UILanguage::Auto,
    }
}
