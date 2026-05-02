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
                if event.state != ShortcutState::Pressed {
                    return;
                }

                let app = app_clone.clone();

                let in_hotkey_mode = app
                    .try_state::<HotkeyModeState>()
                    .map_or(false, |s| s.is_active());

                if in_hotkey_mode {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.eval("window.__cycleNext && window.__cycleNext()");
                    }
                    return;
                }

                let is_opening = {
                    #[cfg(target_os = "macos")]
                    {
                        app.get_webview_panel(crate::window::MAIN_WINDOW_LABEL)
                            .map_or(true, |p| !p.is_visible())
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        true
                    }
                };

                let sticky = app
                    .try_state::<SettingsManager>()
                    .map_or(false, |m| m.get().sticky_mode);

                if is_opening && !sticky {
                    if let Some(hotkey_state) = app.try_state::<HotkeyModeState>() {
                        hotkey_state.enter();
                    }
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
                }

                tauri::async_runtime::spawn(async move {
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
