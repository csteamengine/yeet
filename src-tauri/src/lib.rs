mod clipboard;
mod database;
mod exclusions;
mod github_auth;
mod hotkey;
mod keyboard;
mod settings;
mod window;

use clipboard::ClipboardMonitor;
use database::Database;
use hotkey::HotkeyManager;
use settings::SettingsManager;

#[cfg(target_os = "macos")]
use window::{set_window_blur, HotkeyModeState, PanelHideGuard, PreviousAppState, WebviewWindowExt, MAIN_WINDOW_LABEL};

#[cfg(not(target_os = "macos"))]
use window::HotkeyModeState;

use window::{SelectedItemState, SettingsOpenState};

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager,
};

#[cfg(target_os = "macos")]
use tauri::ActivationPolicy;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build());

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        .setup(|app| {
            // Menu-bar-only on macOS.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(ActivationPolicy::Accessory);

            // Prompt for Accessibility permission (required for CGEvent paste simulation).
            #[cfg(target_os = "macos")]
            {
                use core_foundation::base::TCFType;
                use core_foundation::boolean::CFBoolean;
                use core_foundation::dictionary::CFDictionary;
                use core_foundation::string::CFString;

                extern "C" {
                    fn AXIsProcessTrustedWithOptions(options: core_foundation::base::CFTypeRef) -> bool;
                }

                let key = CFString::new("AXTrustedCheckOptionPrompt");
                let value = CFBoolean::true_value();
                let options = CFDictionary::from_CFType_pairs(&[(key, value)]);
                let trusted = unsafe { AXIsProcessTrustedWithOptions(options.as_CFTypeRef()) };
                log::info!("[accessibility] process trusted: {}", trusted);
            }

            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            let db = Database::new(app_data_dir.clone()).expect("Failed to initialize database");
            app.manage(db);

            let settings_manager = SettingsManager::new(app_data_dir.clone());
            let settings = settings_manager.get();
            app.manage(settings_manager);

            // Panel hotkey (default Cmd+Shift+V): opens history with cycling.
            let hotkey_manager = HotkeyManager::new();
            match hotkey_manager.register(&app.handle(), &settings.hotkey) {
                Ok(_) => log::info!("[hotkey] panel shortcut registered: {}", settings.hotkey),
                Err(e) => log::error!("[hotkey] panel shortcut FAILED ({}): {}", settings.hotkey, e),
            }
            app.manage(hotkey_manager);

            let clipboard_monitor = ClipboardMonitor::new();
            if let Some(db) = app.try_state::<Database>() {
                clipboard_monitor.init_last_hash(&db);
            }
            app.manage(clipboard_monitor);

            #[cfg(target_os = "macos")]
            app.manage(PreviousAppState::new());
            #[cfg(target_os = "macos")]
            app.manage(PanelHideGuard::new());
            app.manage(HotkeyModeState::new());
            app.manage(SelectedItemState::new());
            app.manage(SettingsOpenState::new());
            app.manage(github_auth::GitHubAuthState::new(app_data_dir.clone()));

            // Native clipboard poller — runs regardless of webview state.
            {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    log::info!("[clipboard-poll] thread started");
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(150));

                        #[cfg(target_os = "macos")]
                        {
                            use objc::{class, msg_send, sel, sel_impl};
                            use cocoa::base::id;
                            unsafe {
                                let pool: id = msg_send![class!(NSAutoreleasePool), new];
                                let _ = clipboard::capture_clipboard(&app_handle);
                                let _: () = msg_send![pool, drain];
                            }
                        }

                        #[cfg(not(target_os = "macos"))]
                        {
                            let _ = clipboard::capture_clipboard(&app_handle);
                        }
                    }
                });
            }

            // Modifier-release watcher for hotkey-mode paste-on-release (macOS).
            #[cfg(target_os = "macos")]
            {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    extern "C" {
                        fn CGEventSourceFlagsState(stateID: u32) -> u64;
                        fn CGEventSourceKeyState(stateID: u32, key: u16) -> bool;
                    }

                    const MASK_COMMAND: u64 = 0x100000;
                    const MASK_SHIFT: u64 = 0x20000;
                    const VK_ESCAPE: u16 = 53;
                    const VK_RETURN: u16 = 36;

                    let mut prev_return = false;

                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(30));

                        let is_active = app_handle
                            .try_state::<HotkeyModeState>()
                            .map_or(false, |s| s.is_active());

                        if !is_active {
                            prev_return = false;
                            continue;
                        }

                        let esc_pressed = unsafe { CGEventSourceKeyState(1, VK_ESCAPE) };

                        if esc_pressed {
                            if let Some(s) = app_handle.try_state::<HotkeyModeState>() {
                                s.exit();
                            }
                            if let Some(s) = app_handle.try_state::<SelectedItemState>() {
                                s.take();
                            }
                            let app = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = window::hide_window(app).await;
                            });
                            prev_return = false;
                            continue;
                        }

                        // Enter/Return — paste the highlighted item immediately.
                        let settings_open = app_handle
                            .try_state::<SettingsOpenState>()
                            .map_or(false, |s| s.is_open());
                        let return_now = unsafe { CGEventSourceKeyState(1, VK_RETURN) };
                        if return_now && !prev_return && !settings_open {
                            if let Some(state) = app_handle.try_state::<HotkeyModeState>() {
                                state.exit();
                            }
                            if let Some(sel) = app_handle.try_state::<SelectedItemState>() {
                                if let Some(item_id) = sel.take() {
                                    log::info!("[enter] pasting item: {}", item_id);
                                    let app = app_handle.clone();
                                    tauri::async_runtime::spawn(async move {
                                        if let Err(e) = clipboard::do_paste_and_simulate(app, item_id).await {
                                            log::warn!("paste on enter failed: {}", e);
                                        }
                                    });
                                }
                            }
                            prev_return = return_now;
                            continue;
                        }
                        prev_return = return_now;

                        let (cmd_held, shift_held) = unsafe {
                            let flags = CGEventSourceFlagsState(1);
                            (flags & MASK_COMMAND != 0, flags & MASK_SHIFT != 0)
                        };

                        if !cmd_held && !shift_held {
                            // Don't auto-dismiss while settings panel is open
                            let settings_open = app_handle
                                .try_state::<SettingsOpenState>()
                                .map_or(false, |s| s.is_open());
                            if settings_open {
                                continue;
                            }

                            std::thread::sleep(std::time::Duration::from_millis(50));
                            let esc_after = unsafe { CGEventSourceKeyState(1, VK_ESCAPE) };
                            if let Some(state) = app_handle.try_state::<HotkeyModeState>() {
                                if state.is_active() && !esc_after {
                                    state.exit();
                                    if let Some(sel) = app_handle.try_state::<SelectedItemState>() {
                                        if let Some(item_id) = sel.take() {
                                            log::info!("[modifier-release] pasting item: {}", item_id);
                                            let app = app_handle.clone();
                                            tauri::async_runtime::spawn(async move {
                                                if let Err(e) = clipboard::do_paste_and_simulate(app, item_id).await {
                                                    log::warn!("paste on release failed: {}", e);
                                                }
                                            });
                                        } else {
                                            let app = app_handle.clone();
                                            tauri::async_runtime::spawn(async move {
                                                let _ = window::hide_window(app).await;
                                            });
                                        }
                                    }
                                } else if esc_after && state.is_active() {
                                    state.exit();
                                    if let Some(sel) = app_handle.try_state::<SelectedItemState>() {
                                        sel.take();
                                    }
                                    let app = app_handle.clone();
                                    tauri::async_runtime::spawn(async move {
                                        let _ = window::hide_window(app).await;
                                    });
                                }
                            }
                        }
                    }
                });
            }

            // Convert main window to NSPanel + apply vibrancy.
            #[cfg(target_os = "macos")]
            {
                if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                    if let Err(e) = window.to_yeet_panel() {
                        log::warn!("Failed to initialize panel: {:?}", e);
                    }
                    if let Err(e) = set_window_blur(&window, true) {
                        log::warn!("Failed to apply vibrancy: {:?}", e);
                    }
                }
            }

            setup_tray(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            clipboard::check_clipboard,
            clipboard::get_clipboard_items,
            clipboard::delete_clipboard_item,
            clipboard::clear_history,
            clipboard::paste_item,
            clipboard::paste_and_simulate,
            clipboard::get_image_base64,
            window::show_window,
            window::hide_window,
            window::toggle_window,
            window::is_window_visible,
            window::enter_hotkey_mode,
            window::exit_hotkey_mode,
            window::set_selected_item,
            window::is_hotkey_mode_active,
            window::set_settings_open,
            window::are_modifiers_held,
            settings::get_settings,
            settings::update_settings,
            settings::set_hotkey,
            settings::set_theme,
            settings::add_excluded_app,
            settings::remove_excluded_app,
            settings::toggle_excluded_type,
            hotkey::register_hotkey,
            hotkey::validate_hotkey,
            exclusions::get_current_app,
            exclusions::check_app_excluded,
            github_auth::github_open_url,
            github_auth::github_start_device_flow,
            github_auth::github_poll_token,
            github_auth::github_cancel_polling,
            github_auth::github_get_token,
            github_auth::github_get_user,
            github_auth::github_logout,
            github_auth::check_for_updates,
            github_auth::download_and_install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let open_item = MenuItemBuilder::with_id("open", "Open Yeet").build(app)?;
    let settings_item = MenuItemBuilder::with_id("settings", "Settings").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&open_item)
        .separator()
        .item(&settings_item)
        .separator()
        .item(&quit_item)
        .build()?;

    let icon = Image::from_bytes(include_bytes!("../icons/icon.png"))
        .expect("Failed to load tray icon");

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = window::show_window(app).await;
                });
            }
            "settings" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = window::show_window(app.clone()).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.eval("window.__openSettings && window.__openSettings()");
                    }
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}
