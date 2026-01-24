//! NanoTrans - Lightweight Cross-Platform Translation Assistant
//! Main entry point and event loop

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod caret;
mod clipboard;
mod config;
mod hotkey;
mod i18n;
mod input;
mod translate;
mod tray;

use anyhow::Result;
use config::{Config, PromptPreset};
use hotkey::HotkeyManager;
use slint::{ComponentHandle, LogicalSize, ModelRc, PhysicalPosition, SharedString, VecModel};
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

// 与 popup.slint 的默认尺寸保持一致
const POPUP_WIDTH: f32 = 380.0;
const POPUP_HEIGHT: f32 = 220.0;

fn main() -> Result<()> {
    init_macos_font();
    // Load configuration
    let mut config = Config::load().unwrap_or_default();
    input::set_hotkey_log_enabled(config.hotkey_log_enabled);

    // Initialize i18n
    i18n::init(&config.ui_language);

    // Prepare hotkey manager (fallback to default on invalid config)
    let hotkey_manager_inner = match HotkeyManager::new(&config.hotkey) {
        Ok(manager) => manager,
        Err(e) => {
            eprintln!(
                "注册全局快捷键失败({})，回退到默认 {}",
                e,
                hotkey::DEFAULT_HOTKEY
            );
            config.hotkey = hotkey::DEFAULT_HOTKEY.to_string();
            if let Err(save_err) = config.save() {
                eprintln!("写入默认快捷键失败: {}", save_err);
            }
            HotkeyManager::new(&config.hotkey)?
        }
    };

    // Create shared state
    let shared_state = Arc::new(Mutex::new(SharedState {
        config: config.clone(),
        original_clipboard: None,
        popup_shown_at: None,
    }));

    // Create the translation popup window
    let popup = TranslatePopup::new()?;
    apply_macos_font_family_popup(&popup);
    popup.hide()?;

    // Set i18n texts for popup
    set_popup_i18n_texts(&popup);

    // Create system tray
    let _tray = tray::create_tray()?;

    // Register global hotkey
    let hotkey_manager = Arc::new(Mutex::new(hotkey_manager_inner));

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

    // Settings window state
    let settings_window: Rc<RefCell<Option<SettingsWindow>>> = Rc::new(RefCell::new(None));

    // Handle open settings from popup
    let shared_state_settings = Arc::clone(&shared_state);
    let settings_window_popup = Rc::clone(&settings_window);
    let hotkey_manager_popup = Arc::clone(&hotkey_manager);
    popup.on_open_settings({
        move || {
            open_settings_window(&shared_state_settings, &settings_window_popup, &hotkey_manager_popup);
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
    let settings_window_capture = Rc::clone(&settings_window);
    let shared_state_menu = Arc::clone(&shared_state);
    let hotkey_manager_menu = Arc::clone(&hotkey_manager);
    let popup_weak_ctrlv = popup_weak.clone();
    #[cfg(target_os = "macos")]
    let monitor_error_rx = input::keyboard_monitor_error_receiver();

    // 启动键盘监控（监控 Ctrl+V）
    input::start_keyboard_monitor();

    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(50), move || {
        // Check for hotkey events
        let hotkey_rx = hotkey::hotkey_event_receiver();
        if let Ok(event) = hotkey_rx.try_recv() {
            if let Ok(manager) = hotkey_manager_timer.lock() {
                if manager.is_translate_hotkey(&event) {
                    handle_translate_hotkey(&popup_weak_timer, &shared_state_timer, &rt_timer);
                }
            }
        }

        // Check for menu events
        let menu_rx = tray::menu_event_receiver();
        if let Ok(event) = menu_rx.try_recv() {
            match tray::handle_menu_event(&event) {
                tray::MenuAction::OpenSettings => {
                    open_settings_window(&shared_state_menu, &settings_window_timer, &hotkey_manager_menu);
                }
                tray::MenuAction::Exit => std::process::exit(0),
                tray::MenuAction::None => {}
            }
        }

        // 检测 Ctrl+V，用户粘贴后自动关闭窗口
        if input::check_ctrl_v_pressed() {
            if let Some(popup) = popup_weak_ctrlv.upgrade() {
                if popup.window().is_visible() {
                    popup.hide().ok();
                }
            }
        }

        // Check for captured hotkey in settings window
        if let Some(ref win) = *settings_window_capture.borrow() {
            if win.get_hotkey_recording() {
                if let Some(polled) = input::poll_hotkey_capture() {
                    win.set_hotkey_recording(false);
                    apply_captured_hotkey(win, &hotkey_manager_timer, &shared_state_timer, &polled);
                }
                if let Some(captured) = input::get_captured_hotkey() {
                    win.set_hotkey_recording(false);
                    apply_captured_hotkey(win, &hotkey_manager_timer, &shared_state_timer, &captured);
                }
            }
        }

        #[cfg(target_os = "macos")]
        if let Ok(reason) = monitor_error_rx.try_recv() {
            show_macos_permission_alert_once(&reason);
        }
    });

    // 使用 run_event_loop_until_quit 让程序在所有窗口关闭后继续运行
    // 只有托盘菜单的 Exit 或调用 quit_event_loop() 才会退出
    slint::run_event_loop_until_quit()?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn init_macos_font() {
    if std::env::var_os("SLINT_DEFAULT_FONT").is_some() {
        return;
    }
    if let Some(path) = select_macos_font_path() {
        std::env::set_var("SLINT_DEFAULT_FONT", path);
    }
}

#[cfg(not(target_os = "macos"))]
fn init_macos_font() {}

#[cfg(target_os = "macos")]
fn select_macos_font_path() -> Option<&'static str> {
    let candidates = [
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
    ];
    candidates
        .iter()
        .copied()
        .find(|path| std::path::Path::new(path).exists())
}

#[cfg(target_os = "macos")]
fn apply_macos_font_family_popup(component: &TranslatePopup) {
    if let Some(font_family) = select_macos_font_family() {
        component.global::<crate::Theme>().set_font_family(SharedString::from(font_family));
    }
}

#[cfg(target_os = "macos")]
fn apply_macos_font_family_settings(component: &SettingsWindow) {
    if let Some(font_family) = select_macos_font_family() {
        component.global::<crate::Theme>().set_font_family(SharedString::from(font_family));
    }
}

#[cfg(target_os = "macos")]
fn select_macos_font_family() -> Option<&'static str> {
    if std::path::Path::new("/System/Library/Fonts/Hiragino Sans GB.ttc").exists() {
        return Some("Hiragino Sans GB");
    }
    if std::path::Path::new("/System/Library/Fonts/STHeiti Medium.ttc").exists()
        || std::path::Path::new("/System/Library/Fonts/STHeiti Light.ttc").exists()
    {
        return Some("STHeiti");
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn apply_macos_font_family_popup(_component: &TranslatePopup) {}

#[cfg(not(target_os = "macos"))]
fn apply_macos_font_family_settings(_component: &SettingsWindow) {}

/// Open the settings window
fn open_settings_window(
    shared_state: &Arc<Mutex<SharedState>>,
    settings_window: &Rc<RefCell<Option<SettingsWindow>>>,
    hotkey_manager: &Arc<Mutex<HotkeyManager>>,
) {
    struct PromptPresetDraft {
        presets: Vec<PromptPreset>,
        selected: usize,
    }

    fn sync_prompt_preset_ui(win: &SettingsWindow, draft: &PromptPresetDraft) {
        let names: Vec<SharedString> = draft.presets.iter().map(|p| SharedString::from(&p.name)).collect();
        win.set_prompt_preset_names(ModelRc::new(VecModel::from(names)));
        win.set_prompt_preset_index(draft.selected as i32);
        if let Some(preset) = draft.presets.get(draft.selected) {
            win.set_prompt_preset_name(SharedString::from(&preset.name));
            win.set_prompt_system_template(SharedString::from(&preset.system_template));
            win.set_prompt_user_template(SharedString::from(&preset.user_template));
            win.set_prompt_preset_deletable(!preset.is_preset);
        } else {
            win.set_prompt_preset_deletable(false);
        }
    }

    fn update_selected_preset_from_ui(win: &SettingsWindow, draft: &mut PromptPresetDraft) {
        let Some(preset) = draft.presets.get_mut(draft.selected) else { return; };
        let name = win.get_prompt_preset_name().to_string();
        if !name.trim().is_empty() {
            preset.name = name;
        }
        preset.system_template = win.get_prompt_system_template().to_string();
        let user_template = win.get_prompt_user_template().to_string();
        preset.user_template = if user_template.trim().is_empty() {
            "{{text}}".to_string()
        } else {
            user_template
        };
    }

    fn next_custom_preset(draft: &PromptPresetDraft) -> PromptPreset {
        let mut idx = 1usize;
        let id = loop {
            let candidate = format!("custom-{}", idx);
            if draft.presets.iter().all(|p| p.id != candidate) {
                break candidate;
            }
            idx += 1;
        };
        PromptPreset {
            id,
            name: format!("自定义 {}", idx),
            system_template: String::new(),
            user_template: "{{text}}".to_string(),
            is_preset: false,
        }
    }

    if settings_window.borrow().is_some() {
        if let Some(ref win) = *settings_window.borrow() {
            win.set_hotkey_recording(false);
            input::stop_hotkey_capture();
            win.show().ok();
            return;
        }
    }

    let win = match SettingsWindow::new() {
        Ok(w) => w,
        Err(e) => { eprintln!("Failed to create settings: {}", e); return; }
    };
    apply_macos_font_family_settings(&win);

    win.set_hotkey_recording(false);
    input::stop_hotkey_capture();

    // 以磁盘为准，避免内存配置与文件不一致
    if let Ok(latest) = Config::load() {
        if let Ok(mut state) = shared_state.lock() {
            state.config = latest;
        }
    }

    // Set i18n texts
    set_settings_i18n_texts(&win);

    // Load config into UI
    let (provider_idx, lang_idx, prompt_presets, active_prompt_id, provider_names) = {
        let state = shared_state.lock().unwrap();
        let config = &state.config;

        win.set_hotkey(SharedString::from(&config.hotkey));
        win.set_hotkey_log_enabled(config.hotkey_log_enabled);

        let idx = config
            .provider_index(&config.active_provider_id)
            .unwrap_or(0)
            .min(config.providers.len().saturating_sub(1));

        if let Some(p) = config.providers.get(idx) {
            win.set_api_key(SharedString::from(&p.api_key));
            win.set_api_base(SharedString::from(&p.api_base));
            win.set_model(SharedString::from(&p.model));
        }

        let provider_names: Vec<SharedString> = config
            .providers
            .iter()
            .map(|p| SharedString::from(&p.name))
            .collect();
        let lang_index = i18n::language_to_index(&config.ui_language);
        (
            idx as i32,
            lang_index,
            config.prompt_presets.clone(),
            config.active_prompt_preset_id.clone(),
            provider_names,
        )
    };

    // Set provider list
    win.set_provider_names(ModelRc::new(VecModel::from(provider_names)));
    // 必须在设置 provider_names 之后再设置 provider_index，
    // 因为 ComboBox 在设置 model 时可能会重置 current-index
    win.set_provider_index(provider_idx);

    // Set language list and index
    let language_names: Vec<SharedString> = vec![
        "Auto".into(), "English".into(), "中文".into(),
    ];
    win.set_language_names(ModelRc::new(VecModel::from(language_names)));
    win.set_language_index(lang_idx);

    // Prompt preset draft (kept local until Save)
    let prompt_presets = if prompt_presets.is_empty() {
        Config::default().prompt_presets
    } else {
        prompt_presets
    };
    let selected_prompt_idx = prompt_presets
        .iter()
        .position(|p| p.id == active_prompt_id)
        .unwrap_or(0)
        .min(prompt_presets.len().saturating_sub(1));
    let prompt_draft = Rc::new(RefCell::new(PromptPresetDraft { presets: prompt_presets, selected: selected_prompt_idx }));
    sync_prompt_preset_ui(&win, &prompt_draft.borrow());

    // 防止 ComboBox 在下一拍把 index 复位到 0（Slint 内部行为）
    // 这里强制同步一次，确保 UI 与配置一致
    let win_sync = win.as_weak();
    let provider_idx_sync = provider_idx;
    let lang_idx_sync = lang_idx;
    let prompt_idx_sync = selected_prompt_idx as i32;
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(w) = win_sync.upgrade() {
            if w.get_provider_index() != provider_idx_sync {
                w.set_provider_index(provider_idx_sync);
            }
            if w.get_language_index() != lang_idx_sync {
                w.set_language_index(lang_idx_sync);
            }
            if w.get_prompt_preset_index() != prompt_idx_sync {
                w.set_prompt_preset_index(prompt_idx_sync);
            }
        }
    });

    // 自动保存（延迟写盘）
    let autosave_timer = Rc::new(slint::Timer::default());
    let autosave_timer_save = Rc::clone(&autosave_timer);
    let shared_state_autosave = Arc::clone(shared_state);
    let schedule_autosave: Rc<dyn Fn()> = Rc::new(move || {
        autosave_timer_save.stop();
        let shared_state = Arc::clone(&shared_state_autosave);
        autosave_timer_save.start(slint::TimerMode::SingleShot, Duration::from_millis(450), move || {
            if let Ok(state) = shared_state.lock() {
                if let Err(e) = state.config.save() {
                    eprintln!("自动保存配置失败: {}", e);
                }
            }
        });
    });

    let current_provider_index = Rc::new(RefCell::new(provider_idx));
    let apply_ui_to_state: Rc<dyn Fn(&SettingsWindow)> = {
        let shared_state = Arc::clone(shared_state);
        let prompt_draft = Rc::clone(&prompt_draft);
        let current_provider_index = Rc::clone(&current_provider_index);
        Rc::new(move |w: &SettingsWindow| {
            let mut config = {
                let state = shared_state.lock().unwrap();
                state.config.clone()
            };

            config.hotkey = w.get_hotkey().to_string();
            config.hotkey_log_enabled = w.get_hotkey_log_enabled();
            config.ui_language = i18n::index_to_language(w.get_language_index());

            let idx = (*current_provider_index.borrow()).max(0) as usize;
            if let Some(p) = config.providers.get_mut(idx) {
                p.api_key = w.get_api_key().to_string();
                p.api_base = w.get_api_base().to_string();
                p.model = w.get_model().to_string();
                config.active_provider_id = p.id.clone();
            }

            {
                let mut draft = prompt_draft.borrow_mut();
                update_selected_preset_from_ui(w, &mut draft);
                config.prompt_presets = draft.presets.clone();
                if let Some(active) = config.prompt_presets.get(draft.selected) {
                    config.active_prompt_preset_id = active.id.clone();
                }
                config.normalize();
            }

            let hotkey_log_enabled = config.hotkey_log_enabled;
            if let Ok(mut state) = shared_state.lock() {
                state.config = config;
            }
            input::set_hotkey_log_enabled(hotkey_log_enabled);
        })
    };

    // Handle provider selection
    let shared_state_sel = Arc::clone(shared_state);
    let win_weak = win.as_weak();
    let current_provider_index_sel = Rc::clone(&current_provider_index);
    let schedule_autosave_sel = Rc::clone(&schedule_autosave);
    let apply_ui_to_state_sel = Rc::clone(&apply_ui_to_state);
    win.on_provider_selected(move |index| {
        if let Some(w) = win_weak.upgrade() {
            let selected_name = index.to_string();
            let prev_idx = (*current_provider_index_sel.borrow()).max(0) as usize;

            let new_idx = {
                let state = shared_state_sel.lock().unwrap();
                state
                    .config
                    .providers
                    .iter()
                    .position(|p| p.name == selected_name)
                    .unwrap_or(0)
            };

            if let Ok(mut state) = shared_state_sel.lock() {
                if let Some(prev) = state.config.providers.get_mut(prev_idx) {
                    prev.api_key = w.get_api_key().to_string();
                    prev.api_base = w.get_api_base().to_string();
                    prev.model = w.get_model().to_string();
                }
                if let Some(next) = state.config.providers.get(new_idx) {
                    w.set_api_key(SharedString::from(&next.api_key));
                    w.set_api_base(SharedString::from(&next.api_base));
                    w.set_model(SharedString::from(&next.model));
                }
            }

            *current_provider_index_sel.borrow_mut() = new_idx as i32;
            if w.get_provider_index() != new_idx as i32 {
                w.set_provider_index(new_idx as i32);
            }
            apply_ui_to_state_sel(&w);
            schedule_autosave_sel();
        }
    });

    // Handle language selection (preview)
    let win_weak_lang = win.as_weak();
    let schedule_autosave_lang = Rc::clone(&schedule_autosave);
    let apply_ui_to_state_lang = Rc::clone(&apply_ui_to_state);
    win.on_language_selected(move |name| {
        let index = match name.as_str() {
            "Auto" => 0,
            "English" => 1,
            "中文" => 2,
            _ => 0,
        };
        let new_lang = i18n::index_to_language(index);
        i18n::init(&new_lang);
        if let Some(w) = win_weak_lang.upgrade() {
            if w.get_language_index() != index {
                w.set_language_index(index);
            }
            set_settings_i18n_texts(&w);
            apply_ui_to_state_lang(&w);
            schedule_autosave_lang();
        }
    });

    // Handle hotkey capture - just start capture mode
    let win_weak_hotkey = win.as_weak();
    win.on_start_hotkey_capture(move || {
        if let Some(w) = win_weak_hotkey.upgrade() {
            w.set_hotkey_recording(true);
            input::start_hotkey_capture();
        }
    });

    // Handle prompt preset selection / add / delete (draft only)
    let win_weak_prompt = win.as_weak();
    let prompt_draft_sel = Rc::clone(&prompt_draft);
    let schedule_autosave_preset = Rc::clone(&schedule_autosave);
    let apply_ui_to_state_preset = Rc::clone(&apply_ui_to_state);
    win.on_prompt_preset_selected(move |name| {
        if let Some(w) = win_weak_prompt.upgrade() {
            let mut draft = prompt_draft_sel.borrow_mut();
            update_selected_preset_from_ui(&w, &mut draft);
            let selected_name = name.to_string();
            if let Some(idx) = draft.presets.iter().position(|p| p.name == selected_name) {
                draft.selected = idx;
            }
            sync_prompt_preset_ui(&w, &draft);
            apply_ui_to_state_preset(&w);
            schedule_autosave_preset();
        }
    });

    let win_weak_prompt_add = win.as_weak();
    let prompt_draft_add = Rc::clone(&prompt_draft);
    let schedule_autosave_add = Rc::clone(&schedule_autosave);
    let apply_ui_to_state_add = Rc::clone(&apply_ui_to_state);
    win.on_add_prompt_preset(move || {
        if let Some(w) = win_weak_prompt_add.upgrade() {
            let mut draft = prompt_draft_add.borrow_mut();
            update_selected_preset_from_ui(&w, &mut draft);
            let new_preset = next_custom_preset(&draft);
            draft.presets.push(new_preset);
            draft.selected = draft.presets.len().saturating_sub(1);
            sync_prompt_preset_ui(&w, &draft);
            apply_ui_to_state_add(&w);
            schedule_autosave_add();
        }
    });

    let win_weak_prompt_del = win.as_weak();
    let prompt_draft_del = Rc::clone(&prompt_draft);
    let schedule_autosave_del = Rc::clone(&schedule_autosave);
    let apply_ui_to_state_del = Rc::clone(&apply_ui_to_state);
    win.on_delete_prompt_preset(move || {
        if let Some(w) = win_weak_prompt_del.upgrade() {
            let mut draft = prompt_draft_del.borrow_mut();
            if draft.presets.len() <= 1 {
                return;
            }
            if let Some(current) = draft.presets.get(draft.selected) {
                if current.is_preset {
                    return;
                }
            }
            let remove_idx = draft.selected;
            draft.presets.remove(remove_idx);
            if draft.selected >= draft.presets.len() {
                draft.selected = draft.presets.len().saturating_sub(1);
            }
            sync_prompt_preset_ui(&w, &draft);
            apply_ui_to_state_del(&w);
            schedule_autosave_del();
        }
    });

    // Handle settings changed (auto-save)
    let win_weak_changed = win.as_weak();
    let schedule_autosave_changed = Rc::clone(&schedule_autosave);
    let apply_ui_to_state_changed = Rc::clone(&apply_ui_to_state);
    win.on_settings_changed(move || {
        if let Some(w) = win_weak_changed.upgrade() {
            apply_ui_to_state_changed(&w);
            schedule_autosave_changed();
        }
    });

    // Handle apply button (flush auto-save now)
    let win_weak_apply = win.as_weak();
    let shared_state_apply = Arc::clone(shared_state);
    let autosave_timer_apply = Rc::clone(&autosave_timer);
    let apply_ui_to_state_apply = Rc::clone(&apply_ui_to_state);
    win.on_apply_api_settings(move || {
        if let Some(w) = win_weak_apply.upgrade() {
            autosave_timer_apply.stop();
            apply_ui_to_state_apply(&w);
            if let Ok(state) = shared_state_apply.lock() {
                if let Err(e) = state.config.save() {
                    eprintln!("写入配置失败: {}", e);
                }
            }
        }
    });

    // Handle cancel
    let settings_window_cancel = Rc::clone(settings_window);
    let win_weak_cancel = win.as_weak();
    win.on_cancel_settings(move || {
        input::stop_hotkey_capture();
        if let Some(w) = win_weak_cancel.upgrade() {
            w.set_hotkey_recording(false);
            w.hide().ok();
        }
        *settings_window_cancel.borrow_mut() = None;
    });

    win.show().ok();
    *settings_window.borrow_mut() = Some(win);
}

fn popup_physical_size(popup: &TranslatePopup) -> (i32, i32) {
    let mut size = popup.window().size();
    if size.width == 0 || size.height == 0 {
        popup.window().set_size(LogicalSize::new(POPUP_WIDTH, POPUP_HEIGHT));
        size = popup.window().size();
    }
    (size.width as i32, size.height as i32)
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

    let (cursor_x, cursor_y) = caret::get_caret_position();

    if let Some(popup) = popup_weak.upgrade() {
        popup.set_source_text(SharedString::from(&selected_text));
        popup.set_translated_text(SharedString::new());
        popup.set_error_message(SharedString::new());
        popup.set_loading(true);

        // 计算窗口位置：居中于鼠标上方，并确保不超出屏幕
        let (popup_width, popup_height) = popup_physical_size(&popup);
        let (x, y) = caret::calculate_popup_position(cursor_x, cursor_y, popup_width, popup_height);
        popup.window().set_position(PhysicalPosition::new(x, y));
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

fn apply_captured_hotkey(
    win: &SettingsWindow,
    hotkey_manager: &Arc<Mutex<HotkeyManager>>,
    shared_state: &Arc<Mutex<SharedState>>,
    hotkey: &str,
) {
    if hotkey.is_empty() {
        return;
    }
    let previous = win.get_hotkey().to_string();
    let hotkey_result = hotkey_manager
        .lock()
        .map_err(|e| format!("hotkey manager unavailable: {}", e))
        .and_then(|mut mgr| mgr.update_hotkey(hotkey).map_err(|e| e.to_string()));

    if let Err(err) = hotkey_result {
        eprintln!("预览更新全局快捷键失败: {}", err);
        win.set_hotkey(SharedString::from(&previous));
        return;
    }

    win.set_hotkey(SharedString::from(hotkey));

    if let Ok(mut state) = shared_state.lock() {
        state.config.hotkey = hotkey.to_string();
        if let Err(e) = state.config.save() {
            eprintln!("写入配置失败: {}", e);
        }
    }
}

#[cfg(target_os = "macos")]
fn show_macos_permission_alert_once(reason: &str) {
    use std::sync::Once;

    static SHOWN: Once = Once::new();
    let reason = reason.to_string();
    SHOWN.call_once(|| {
        show_macos_permission_alert(&reason);
    });
}

#[cfg(target_os = "macos")]
fn show_macos_permission_alert(reason: &str) {
    use cocoa::appkit::NSApp;
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSAutoreleasePool, NSString};
    use objc::{class, msg_send, sel, sel_impl};
    use objc::runtime::YES;

    const ALERT_INPUT_MONITOR: i64 = 1000;
    const ALERT_ACCESSIBILITY: i64 = 1001;

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let app = NSApp();
        let _: () = msg_send![app, activateIgnoringOtherApps: YES];

        let alert: id = msg_send![class!(NSAlert), alloc];
        let alert: id = msg_send![alert, init];

        let title = NSString::alloc(nil).init_str("需要系统权限");
        let info = NSString::alloc(nil).init_str(
            "NanoTrans 无法建立键盘监听，快捷键不可用。\n请在 系统设置 > 隐私与安全性 > 输入监控 与 辅助功能 中允许 NanoTrans，然后重启应用。"
        );
        let _: () = msg_send![alert, setMessageText: title];
        let _: () = msg_send![alert, setInformativeText: info];

        let btn_input = NSString::alloc(nil).init_str("打开输入监控");
        let btn_access = NSString::alloc(nil).init_str("打开辅助功能");
        let btn_later = NSString::alloc(nil).init_str("稍后");
        let _: id = msg_send![alert, addButtonWithTitle: btn_input];
        let _: id = msg_send![alert, addButtonWithTitle: btn_access];
        let _: id = msg_send![alert, addButtonWithTitle: btn_later];

        let response: i64 = msg_send![alert, runModal];
        if response == ALERT_INPUT_MONITOR {
            open_system_settings("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent");
        } else if response == ALERT_ACCESSIBILITY {
            open_system_settings("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility");
        }
    }

    if !reason.is_empty() {
        eprintln!("keyboard monitor error: {}", reason);
    }
}

#[cfg(target_os = "macos")]
fn open_system_settings(url: &str) {
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSAutoreleasePool, NSString};
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let ns_url_str = NSString::alloc(nil).init_str(url);
        let ns_url: id = msg_send![class!(NSURL), URLWithString: ns_url_str];
        if ns_url == nil {
            return;
        }
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let _: bool = msg_send![workspace, openURL: ns_url];
    }
}

/// Set i18n texts for popup window
fn set_popup_i18n_texts(popup: &TranslatePopup) {
    let t = i18n::t();
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
    win.set_i18n_hotkey_placeholder(SharedString::from(t.hotkey_placeholder));
    win.set_i18n_hotkey_recording(SharedString::from(t.hotkey_recording));
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
    win.set_i18n_apply(SharedString::from(t.apply));
    win.set_i18n_prompt_settings(SharedString::from(t.prompt_settings));
    win.set_i18n_prompt_preset(SharedString::from(t.prompt_preset));
    win.set_i18n_prompt_add(SharedString::from(t.prompt_add));
    win.set_i18n_prompt_delete(SharedString::from(t.prompt_delete));
    win.set_i18n_prompt_name(SharedString::from(t.prompt_name));
    win.set_i18n_prompt_system(SharedString::from(t.prompt_system));
    win.set_i18n_prompt_user(SharedString::from(t.prompt_user));
    win.set_i18n_prompt_vars(SharedString::from(t.prompt_vars));
    win.set_i18n_cancel(SharedString::from(t.cancel));
    win.set_i18n_language(SharedString::from(t.ui_language));
    win.set_i18n_hotkey_log_title(SharedString::from(t.hotkey_log_title));
    win.set_i18n_hotkey_log_enable(SharedString::from(t.hotkey_log_enable));
    win.set_i18n_hotkey_log_hint(SharedString::from(t.hotkey_log_hint));
}
