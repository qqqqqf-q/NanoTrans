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
    pub prompt_settings: &'static str,
    pub prompt_preset: &'static str,
    pub prompt_add: &'static str,
    pub prompt_delete: &'static str,
    pub prompt_name: &'static str,
    pub prompt_system: &'static str,
    pub prompt_user: &'static str,
    pub prompt_vars: &'static str,
    pub cancel: &'static str,
    pub save: &'static str,
    pub ui_language: &'static str,
    pub hotkey_log_title: &'static str,
    pub hotkey_log_enable: &'static str,
    pub hotkey_log_hint: &'static str,

    // Popup window
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
    prompt_settings: "Prompt Settings",
    prompt_preset: "Preset",
    prompt_add: "Add",
    prompt_delete: "Delete",
    prompt_name: "Preset Name",
    prompt_system: "System Template",
    prompt_user: "User Template",
    prompt_vars: "Vars: {{target_lang_name}} {{target_lang_code}} {{text}}",
    cancel: "Cancel",
    save: "Save",
    ui_language: "UI Language",
    hotkey_log_title: "Local Logs",
    hotkey_log_enable: "Enable hotkey log",
    hotkey_log_hint: "Write hotkey debug logs to a local file",

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
    prompt_settings: "提示词设置",
    prompt_preset: "预设",
    prompt_add: "新增",
    prompt_delete: "删除",
    prompt_name: "预设名称",
    prompt_system: "System 模板",
    prompt_user: "User 模板",
    prompt_vars: "可用变量：{{target_lang_name}} {{target_lang_code}} {{text}}",
    cancel: "取消",
    save: "保存",
    ui_language: "界面语言",
    hotkey_log_title: "本地日志",
    hotkey_log_enable: "启用热键日志",
    hotkey_log_hint: "仅写入本地调试日志，不会上报",

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
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Globalization::GetUserDefaultUILanguage;
        let lang_id = unsafe { GetUserDefaultUILanguage() };
        // Chinese: 0x0804 (Simplified), 0x0404 (Traditional)
        if lang_id == 0x0804 || lang_id == 0x0404 || (lang_id & 0xFF) == 0x04 {
            return Lang::Zh;
        }
    }

    #[cfg(target_os = "macos")]
    {
        use core_foundation::base::TCFType;
        use core_foundation::string::CFString;
        use core_foundation::array::{CFArray, CFArrayRef};

        extern "C" {
            fn CFLocaleCopyPreferredLanguages() -> CFArrayRef;
        }

        unsafe {
            let languages = CFArray::<CFString>::wrap_under_create_rule(CFLocaleCopyPreferredLanguages());
            if languages.len() > 0 {
                if let Some(lang) = languages.get(0) {
                    let lang_str = lang.to_string();
                    if lang_str.starts_with("zh") {
                        return Lang::Zh;
                    }
                }
            }
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
