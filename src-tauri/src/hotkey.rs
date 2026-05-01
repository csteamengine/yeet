use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[cfg(target_os = "macos")]
use tauri_nspanel::ManagerExt;

use crate::database::Database;
use crate::settings::SettingsManager;
use crate::window::{HotkeyModeState, SelectedItemState};

pub struct HotkeyManager {
    current_shortcut: std::sync::Mutex<Option<Shortcut>>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self {
            current_shortcut: std::sync::Mutex::new(None),
        }
    }

    pub fn register<R: Runtime>(&self, app: &AppHandle<R>, hotkey: &str) -> Result<(), String> {
        let shortcut: Shortcut = hotkey.parse().map_err(|e| format!("{:?}", e))?;

        // Unregister existing shortcut if any
        self.unregister(app)?;

        let app_clone = app.clone();

        app.global_shortcut()
            .on_shortcut(shortcut.clone(), move |_app, _shortcut, event| {
                // Only handle key press, not key release
                if event.state != ShortcutState::Pressed {
                    return;
                }

                let app = app_clone.clone();
                tauri::async_runtime::spawn(async move {
                    // Check if we're already in hotkey mode (user cycling through items)
                    let in_hotkey_mode = if let Some(hotkey_state) = app.try_state::<HotkeyModeState>() {
                        hotkey_state.is_active()
                    } else {
                        false
                    };

                    if in_hotkey_mode {
                        // While in hotkey mode, treat the shortcut as a cycle action.
                        // This is a fallback for cases where the global shortcut isn't
                        // unregistered quickly enough to let V keydown reach the webview.
                        let _ = app.emit("hotkey-cycle", ());
                        return;
                    }

                    // Check if window is currently hidden (opening mode)
                    let is_opening = {
                        #[cfg(target_os = "macos")]
                        {
                            if let Ok(panel) = app.get_webview_panel(crate::window::MAIN_WINDOW_LABEL) {
                                !panel.is_visible()
                            } else {
                                true
                            }
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            true
                        }
                    };

                    // Enter hotkey mode and emit event BEFORE showing window
                    // Only when opening (not when closing)
                    // Skip hotkey mode if sticky_mode is enabled — the panel
                    // stays open without modifier tracking.
                    let sticky = app
                        .try_state::<SettingsManager>()
                        .map_or(false, |m| m.get().sticky_mode);

                    if is_opening && !sticky {
                        // Enter backend hotkey mode to prevent auto-hide while modifiers held
                        if let Some(hotkey_state) = app.try_state::<HotkeyModeState>() {
                            hotkey_state.enter();
                        }
                        // Set initial selected item to the most recent clipboard item
                        let first_item_id = app
                            .try_state::<Database>()
                            .and_then(|db| db.get_items(1, 0, None).ok())
                            .and_then(|items| items.into_iter().next())
                            .map(|item| item.id);
                        if let Some(id) = first_item_id {
                            if let Some(selected_state) = app.try_state::<SelectedItemState>() {
                                selected_state.set(id);
                            }
                        }
                        let _ = app.emit("hotkey-mode-started", ());
                        // Global shortcut will be unregistered by the polling thread
                        // (on the is_active && !was_active transition) so V keydown
                        // events reach the webview for cycling.
                    }

                    // Toggle window visibility
                    let _ = crate::window::toggle_window(app).await;
                });
            })
            .map_err(|e| e.to_string())?;

        *self.current_shortcut.lock().unwrap() = Some(shortcut);

        Ok(())
    }

    pub fn unregister<R: Runtime>(&self, app: &AppHandle<R>) -> Result<(), String> {
        let mut current = self.current_shortcut.lock().unwrap();

        if let Some(shortcut) = current.take() {
            app.global_shortcut()
                .unregister(shortcut)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }
}

#[tauri::command]
pub async fn register_hotkey<R: Runtime>(
    app: AppHandle<R>,
    hotkey_manager: tauri::State<'_, HotkeyManager>,
    hotkey: String,
) -> Result<(), String> {
    hotkey_manager.register(&app, &hotkey)
}

#[tauri::command]
pub async fn validate_hotkey(hotkey: String) -> Result<bool, String> {
    // Validate the hotkey format
    let result: Result<Shortcut, _> = hotkey.parse();
    Ok(result.is_ok())
}

/// Paste-shortcut manager. Registers plain Cmd+V (or Ctrl+V on non-mac) as a
/// global shortcut so we can silently paste the latest Yeet history item
/// in place of whatever currently sits on the system clipboard.
///
/// Coordinates with itself: while the handler is running, the shortcut is
/// unregistered so the simulated Cmd+V keystroke passes through to the
/// focused app instead of re-triggering us.
pub struct PasteHotkeyManager {
    current_shortcut: std::sync::Mutex<Option<Shortcut>>,
}

impl PasteHotkeyManager {
    pub fn new() -> Self {
        Self {
            current_shortcut: std::sync::Mutex::new(None),
        }
    }

    pub fn register<R: Runtime>(&self, app: &AppHandle<R>, hotkey: &str) -> Result<(), String> {
        let shortcut: Shortcut = hotkey.parse().map_err(|e| format!("{:?}", e))?;
        self.unregister(app)?;

        let app_clone = app.clone();
        app.global_shortcut()
            .on_shortcut(shortcut.clone(), move |_app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }

                // If the panel is open / hotkey-mode active, the existing
                // panel hotkey flow owns paste behavior — let the user
                // operate the panel without us hijacking V.
                if let Some(state) = app_clone.try_state::<HotkeyModeState>() {
                    if state.is_active() {
                        return;
                    }
                }

                let app = app_clone.clone();
                tauri::async_runtime::spawn(async move {
                    // Unregister so our own simulated Cmd+V doesn't loop back
                    // into this handler.
                    if let Some(mgr) = app.try_state::<PasteHotkeyManager>() {
                        let _ = mgr.unregister(&app);
                    }

                    if let Err(e) = crate::clipboard::paste_latest_silent(app.clone()).await {
                        log::warn!("paste_latest_silent failed: {}", e);
                    }

                    // Small delay so the simulated keystroke flushes before
                    // we re-register, avoiding a self-trigger if the user is
                    // still holding Cmd+V.
                    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;

                    if let Some(mgr) = app.try_state::<PasteHotkeyManager>() {
                        #[cfg(target_os = "macos")]
                        let _ = mgr.register(&app, "Command+V");
                        #[cfg(not(target_os = "macos"))]
                        let _ = mgr.register(&app, "Control+V");
                    }
                });
            })
            .map_err(|e| e.to_string())?;

        *self.current_shortcut.lock().unwrap() = Some(shortcut);
        Ok(())
    }

    pub fn unregister<R: Runtime>(&self, app: &AppHandle<R>) -> Result<(), String> {
        let mut current = self.current_shortcut.lock().unwrap();
        if let Some(shortcut) = current.take() {
            app.global_shortcut()
                .unregister(shortcut)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
