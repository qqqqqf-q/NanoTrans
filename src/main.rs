//! NanoTrans - Lightweight Windows Translation Assistant
//! Main entry point and event loop

#![windows_subsystem = "windows"]

mod caret;
mod clipboard;
mod config;
mod hotkey;
mod i18n;
mod input;
mod translate;
mod tray;

use anyhow::Result;
use config::Config;
use hotkey::HotkeyManager;
use slint::{ComponentHandle, ModelRc, PhysicalPosition, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use translate::Translator;

slint::include_modules!();

/// Application state
struct SharedState {
    config: Config,
    original_clipboard: Option<String>,
    popup_shown_at: Option<std::time::Instant>,  // 窗口显示时间，用于防止立即关闭
}

fn main() -> Result<()> {
    // Load configuration
    let config = Config::load().unwrap_or_default();

    // Initialize i18n
    i18n::init(&config.ui_language);

    // Create shared state
    let shared_state = Arc::new(Mutex::new(SharedState {
        config: config.clone(),
        original_clipboard: None,
        popup_shown_at: None,
    }));

    // Create the translation popup window
    let popup = TranslatePopup::new()?;
    popup.hide()?;

    // Set i18n texts for popup
    set_popup_i18n_texts(&popup);

    // Set up provider list for popup
    let provider_names: Vec<SharedString> = config.providers.iter()
        .map(|p| SharedString::from(&p.name))
        .collect();
    popup.set_provider_list(ModelRc::new(VecModel::from(provider_names)));

    // Set current provider index
    let provider_idx = config.provider_index(&config.active_provider_id).unwrap_or(0) as i32;
    popup.set_current_provider_index(provider_idx);

    // Create system tray
    let _tray = tray::create_tray()?;

    // Register global hotkey
    let hotkey_manager = Arc::new(HotkeyManager::new(&config.hotkey)?);

    // Create async runtime
    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?
    );

    // Clone for callbacks
    let popup_weak = popup.as_weak();

    // Handle apply translation
    let shared_state_apply = Arc::clone(&shared_state);
    popup.on_apply_translation({
        let popup_weak = popup_weak.clone();
        move || {
            if let Some(popup) = popup_weak.upgrade() {
                let translated = popup.get_translated_text().to_string();
                if !translated.is_empty() {
                    let original = shared_state_apply.lock().unwrap().original_clipboard.clone();

                    // 先隐藏窗口，让焦点回到原来的应用程序
                    popup.hide().ok();

                    // 在后台线程中执行粘贴操作，等待焦点切换完成
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(150));
                        let _ = clipboard::paste_and_restore(&translated, original);
                    });
                }
            }
        }
    });

    // Handle close popup
    let shared_state_close = Arc::clone(&shared_state);
    popup.on_close_popup({
        let popup_weak = popup_weak.clone();
        move || {
            if let Some(popup) = popup_weak.upgrade() {
                let original = shared_state_close.lock().unwrap().original_clipboard.clone();
                if let Some(text) = original {
                    let _ = clipboard::simple::set_text(&text);
                }
                popup.hide().ok();
            }
        }
    });

    // Handle copy result
    popup.on_copy_result({
        let popup_weak = popup_weak.clone();
        move || {
            if let Some(popup) = popup_weak.upgrade() {
                let translated = popup.get_translated_text().to_string();
                if !translated.is_empty() {
                    let _ = clipboard::simple::set_text(&translated);
                }
            }
        }
    });

    // Handle provider changed in popup
    let shared_state_provider = Arc::clone(&shared_state);
    popup.on_provider_changed({
        move |index| {
            let mut state = shared_state_provider.lock().unwrap();
            if let Some(provider) = state.config.providers.get(index as usize) {
                state.config.active_provider_id = provider.id.clone();
                let _ = state.config.save();
            }
        }
    });

    // Settings window state
    let settings_window: Rc<RefCell<Option<SettingsWindow>>> = Rc::new(RefCell::new(None));

    // Handle open settings from popup
    let shared_state_settings = Arc::clone(&shared_state);
    let settings_window_popup = Rc::clone(&settings_window);
    popup.on_open_settings({
        move || {
            open_settings_window(&shared_state_settings, &settings_window_popup);
        }
    });

    // Handle window drag
    popup.on_drag_window({
        let popup_weak = popup_weak.clone();
        move |delta_x, delta_y| {
            if let Some(popup) = popup_weak.upgrade() {
                let current_pos = popup.window().position();
                popup.window().set_position(PhysicalPosition::new(
                    current_pos.x + delta_x,
                    current_pos.y + delta_y,
                ));
            }
        }
    });

    // Set up timer to poll for events
    let popup_weak_timer = popup_weak.clone();
    let shared_state_timer = Arc::clone(&shared_state);
    let hotkey_manager_timer = Arc::clone(&hotkey_manager);
    let rt_timer = Arc::clone(&rt);
    let settings_window_timer = Rc::clone(&settings_window);
    let shared_state_menu = Arc::clone(&shared_state);
    let popup_weak_focus = popup_weak.clone();
    let shared_state_focus = Arc::clone(&shared_state);

    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(50), move || {
        // Check for hotkey events
        let hotkey_rx = hotkey::hotkey_event_receiver();
        if let Ok(event) = hotkey_rx.try_recv() {
            if hotkey_manager_timer.is_translate_hotkey(&event) {
                handle_translate_hotkey(&popup_weak_timer, &shared_state_timer, &rt_timer);
            }
        }

        // Check for menu events
        let menu_rx = tray::menu_event_receiver();
        if let Ok(event) = menu_rx.try_recv() {
            match tray::handle_menu_event(&event) {
                tray::MenuAction::OpenSettings => {
                    open_settings_window(&shared_state_menu, &settings_window_timer);
                }
                tray::MenuAction::Exit => std::process::exit(0),
                tray::MenuAction::None => {}
            }
        }

        // 注意：Slint 在所有窗口隐藏后会退出事件循环
        // 因此不能使用自动焦点检测来隐藏窗口
        // 用户需要手动关闭窗口（点击 X 或按 Escape）
    });

    // 使用 run_event_loop_until_quit 让程序在所有窗口关闭后继续运行
    // 只有托盘菜单的 Exit 或调用 quit_event_loop() 才会退出
    slint::run_event_loop_until_quit()?;
    Ok(())
}

/// Open the settings window
fn open_settings_window(
    shared_state: &Arc<Mutex<SharedState>>,
    settings_window: &Rc<RefCell<Option<SettingsWindow>>>,
) {
    if settings_window.borrow().is_some() {
        if let Some(ref win) = *settings_window.borrow() {
            win.show().ok();
            return;
        }
    }

    let win = match SettingsWindow::new() {
        Ok(w) => w,
        Err(e) => { eprintln!("Failed to create settings: {}", e); return; }
    };

    // Set i18n texts
    set_settings_i18n_texts(&win);

    // Load config into UI
    let (provider_idx, provider_type, lang_idx) = {
        let state = shared_state.lock().unwrap();
        let config = &state.config;

        win.set_hotkey(SharedString::from(&config.hotkey));

        let idx = config.provider_index(&config.active_provider_id).unwrap_or(0);
        let ptype = get_provider_type_index(idx);

        if let Some(p) = config.providers.get(idx) {
            win.set_api_key(SharedString::from(&p.api_key));
            win.set_api_base(SharedString::from(&p.api_base));
            win.set_model(SharedString::from(&p.model));
        }

        let lang_index = i18n::language_to_index(&config.ui_language);
        (idx as i32, ptype, lang_index)
    };

    // Set provider list
    let provider_names: Vec<SharedString> = vec![
        "Google Translate".into(), "DeepL".into(), "Zhipu GLM".into(),
        "OpenAI".into(), "Anthropic".into(), "Custom".into(),
    ];
    win.set_provider_names(ModelRc::new(VecModel::from(provider_names)));
    win.set_provider_index(provider_idx);
    win.set_provider_type(provider_type);

    // Set language list and index
    let language_names: Vec<SharedString> = vec![
        "Auto".into(), "English".into(), "中文".into(),
    ];
    win.set_language_names(ModelRc::new(VecModel::from(language_names)));
    win.set_language_index(lang_idx);

    // Handle provider selection
    let shared_state_sel = Arc::clone(shared_state);
    let win_weak = win.as_weak();
    win.on_provider_selected(move |index| {
        if let Some(w) = win_weak.upgrade() {
            let state = shared_state_sel.lock().unwrap();
            if let Some(p) = state.config.providers.get(index as usize) {
                w.set_api_key(SharedString::from(&p.api_key));
                w.set_api_base(SharedString::from(&p.api_base));
                w.set_model(SharedString::from(&p.model));
                w.set_provider_type(get_provider_type_index(index as usize));
            }
        }
    });

    // Handle language selection (preview)
    let win_weak_lang = win.as_weak();
    win.on_language_selected(move |index| {
        let new_lang = i18n::index_to_language(index);
        i18n::init(&new_lang);
        if let Some(w) = win_weak_lang.upgrade() {
            set_settings_i18n_texts(&w);
        }
    });

    // Handle save
    let shared_state_save = Arc::clone(shared_state);
    let settings_window_save = Rc::clone(settings_window);
    let win_weak_save = win.as_weak();
    win.on_save_settings(move || {
        if let Some(w) = win_weak_save.upgrade() {
            let mut state = shared_state_save.lock().unwrap();
            state.config.hotkey = w.get_hotkey().to_string();

            let idx = w.get_provider_index() as usize;
            if let Some(p) = state.config.providers.get_mut(idx) {
                p.api_key = w.get_api_key().to_string();
                p.api_base = w.get_api_base().to_string();
                p.model = w.get_model().to_string();
                state.config.active_provider_id = p.id.clone();
            }

            // Save language setting
            state.config.ui_language = i18n::index_to_language(w.get_language_index());

            let _ = state.config.save();
            w.hide().ok();
        }
        *settings_window_save.borrow_mut() = None;
    });

    // Handle cancel
    let settings_window_cancel = Rc::clone(settings_window);
    let shared_state_cancel = Arc::clone(shared_state);
    let win_weak_cancel = win.as_weak();
    win.on_cancel_settings(move || {
        // Restore original language on cancel
        let state = shared_state_cancel.lock().unwrap();
        i18n::init(&state.config.ui_language);
        drop(state);

        if let Some(w) = win_weak_cancel.upgrade() { w.hide().ok(); }
        *settings_window_cancel.borrow_mut() = None;
    });

    win.show().ok();
    *settings_window.borrow_mut() = Some(win);
}

/// Get provider type index for UI (0=google, 1=deepl, 2+=llm)
fn get_provider_type_index(provider_idx: usize) -> i32 {
    match provider_idx {
        0 => 0,  // Google
        1 => 1,  // DeepL
        _ => provider_idx as i32,  // LLM providers
    }
}

/// Handle the translate hotkey press
fn handle_translate_hotkey(
    popup_weak: &slint::Weak<TranslatePopup>,
    shared_state: &Arc<Mutex<SharedState>>,
    rt: &Arc<tokio::runtime::Runtime>,
) {
    let original_clipboard = clipboard::simple::get_text().ok();
    std::thread::sleep(Duration::from_millis(50));
    input::send_ctrl_c();
    std::thread::sleep(Duration::from_millis(100));

    let selected_text = match clipboard::simple::get_text() {
        Ok(text) => text,
        Err(_) => return,
    };

    if selected_text.is_empty() { return; }
    if let Some(ref orig) = original_clipboard {
        if &selected_text == orig { return; }
    }

    shared_state.lock().unwrap().original_clipboard = original_clipboard;

    let (x, y) = caret::get_caret_position();

    if let Some(popup) = popup_weak.upgrade() {
        popup.set_source_text(SharedString::from(&selected_text));
        popup.set_translated_text(SharedString::new());
        popup.set_error_message(SharedString::new());
        popup.set_loading(true);
        popup.window().set_position(PhysicalPosition::new(x, y + 20));
        popup.show().ok();

        // 记录窗口显示时间，用于焦点检测保护期
        shared_state.lock().unwrap().popup_shown_at = Some(std::time::Instant::now());

        let popup_weak_t = popup_weak.clone();
        let config = shared_state.lock().unwrap().config.clone();
        let text = selected_text.clone();

        rt.spawn(async move {
            let translator = Translator::new(config);
            let result = translator.translate(&text).await;

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(popup) = popup_weak_t.upgrade() {
                    popup.set_loading(false);
                    match result {
                        Ok(r) => {
                            let translated = r.translated_text.clone();
                            popup.set_translated_text(SharedString::from(r.translated_text));
                            // 翻译完成后自动复制到剪贴板，用户可直接 Ctrl+V
                            let _ = clipboard::simple::set_text(&translated);
                        }
                        Err(e) => popup.set_error_message(SharedString::from(e.to_string())),
                    }
                }
            });
        });
    }
}

/// Set i18n texts for popup window
fn set_popup_i18n_texts(popup: &TranslatePopup) {
    let t = i18n::t();
    popup.set_i18n_provider_label(SharedString::from(t.provider_label));
    popup.set_i18n_translating(SharedString::from(t.translating));
    popup.set_i18n_copy(SharedString::from(t.copy));
    popup.set_i18n_apply(SharedString::from(t.apply));
    popup.set_i18n_hint(SharedString::from(t.hint_apply));
}

/// Set i18n texts for settings window
fn set_settings_i18n_texts(win: &SettingsWindow) {
    let t = i18n::t();
    win.set_i18n_title(SharedString::from(t.settings_title));
    win.set_i18n_hotkey(SharedString::from(t.global_hotkey));
    win.set_i18n_provider(SharedString::from(t.translation_provider));
    win.set_i18n_provider_settings(SharedString::from(t.provider_settings));
    win.set_i18n_google_hint(SharedString::from(t.google_no_config));
    win.set_i18n_deepl_settings(SharedString::from(t.deepl_settings));
    win.set_i18n_api_key(SharedString::from(t.api_key));
    win.set_i18n_api_key_placeholder(SharedString::from(t.api_key_placeholder));
    win.set_i18n_deepl_hint(SharedString::from(t.deepl_hint));
    win.set_i18n_api_settings(SharedString::from(t.api_settings));
    win.set_i18n_api_base(SharedString::from(t.api_base_url));
    win.set_i18n_model(SharedString::from(t.model));
    win.set_i18n_model_placeholder(SharedString::from(t.model_placeholder));
    win.set_i18n_cancel(SharedString::from(t.cancel));
    win.set_i18n_save(SharedString::from(t.save));
    win.set_i18n_language(SharedString::from(t.ui_language));
}

