use tauri::{Emitter, Manager, Runtime, WebviewWindow};
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "macos")]
use std::sync::Mutex;

#[cfg(target_os = "macos")]
use tauri_nspanel::{
    objc_id::ShareId,
    panel_delegate,
    raw_nspanel::RawNSPanel,
    ManagerExt, WebviewWindowExt as NsPanelExt,
};

#[cfg(target_os = "macos")]
use cocoa::base::id;

pub const MAIN_WINDOW_LABEL: &str = "main";

/// Guards against re-entrant panel hide (order_out triggers windowDidResignKey)
pub struct PanelHideGuard {
    is_hiding: AtomicBool,
}

impl PanelHideGuard {
    pub fn new() -> Self {
        Self {
            is_hiding: AtomicBool::new(false),
        }
    }

    pub fn set_hiding(&self) {
        self.is_hiding.store(true, Ordering::SeqCst);
    }

    pub fn clear_hiding(&self) {
        self.is_hiding.store(false, Ordering::SeqCst);
    }
}

/// Tracks whether we're in hotkey mode (modifiers held after Cmd+Shift+V)
/// When active, the panel should NOT auto-hide on focus loss
pub struct HotkeyModeState {
    is_active: AtomicBool,
}

impl HotkeyModeState {
    pub fn new() -> Self {
        Self {
            is_active: AtomicBool::new(false),
        }
    }

    pub fn enter(&self) {
        log::info!("[HotkeyMode] Entering hotkey mode (backend)");
        self.is_active.store(true, Ordering::SeqCst);
    }

    pub fn exit(&self) {
        log::info!("[HotkeyMode] Exiting hotkey mode (backend)");
        self.is_active.store(false, Ordering::SeqCst);
    }

    #[allow(dead_code)] // Used in panel delegate closure
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::SeqCst)
    }
}

/// Tracks whether the settings panel is open (synced from frontend).
/// The modifier-release polling thread checks this to avoid dismissing
/// the window while the user is editing settings.
pub struct SettingsOpenState {
    is_open: AtomicBool,
}

impl SettingsOpenState {
    pub fn new() -> Self {
        Self {
            is_open: AtomicBool::new(false),
        }
    }

    pub fn set(&self, open: bool) {
        self.is_open.store(open, Ordering::SeqCst);
    }

    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::SeqCst)
    }
}

/// Stores the ID of the currently selected clipboard item (for hotkey mode paste)
pub struct SelectedItemState {
    id: std::sync::Mutex<Option<String>>,
}

impl SelectedItemState {
    pub fn new() -> Self {
        Self {
            id: std::sync::Mutex::new(None),
        }
    }

    pub fn set(&self, id: String) {
        *self.id.lock().unwrap() = Some(id);
    }

    pub fn take(&self) -> Option<String> {
        self.id.lock().unwrap().take()
    }
}

/// Stores the previously focused application so we can restore focus to it
#[cfg(target_os = "macos")]
pub struct PreviousAppState {
    app: Mutex<Option<id>>,
}

#[cfg(target_os = "macos")]
impl PreviousAppState {
    pub fn new() -> Self {
        Self {
            app: Mutex::new(None),
        }
    }

    /// Capture the currently frontmost application (before we show our window)
    pub fn capture(&self) {
        use objc::{msg_send, sel, sel_impl, class};
        unsafe {
            let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
            let frontmost: id = msg_send![workspace, frontmostApplication];
            if !frontmost.is_null() {
                // Retain the new reference to prevent deallocation
                let _: id = msg_send![frontmost, retain];
                let mut guard = self.app.lock().unwrap();
                // Release old reference if any
                if let Some(old) = guard.take() {
                    let _: () = msg_send![old, release];
                }
                *guard = Some(frontmost);
            }
        }
    }

    /// Restore focus to the previously captured application
    pub fn restore(&self) {
        use objc::{msg_send, sel, sel_impl};
        let app = self.app.lock().unwrap().take();
        if let Some(prev_app) = app {
            unsafe {
                let _: () = msg_send![prev_app, activateWithOptions: 1u64]; // NSApplicationActivateIgnoringOtherApps = 1
                // Balance the retain from capture
                let _: () = msg_send![prev_app, release];
            }
        }
    }
}

#[cfg(target_os = "macos")]
unsafe impl Send for PreviousAppState {}
#[cfg(target_os = "macos")]
unsafe impl Sync for PreviousAppState {}

#[cfg(target_os = "macos")]
pub trait WebviewWindowExt {
    fn to_yeet_panel(&self) -> tauri::Result<ShareId<RawNSPanel>>;
    fn center_at_cursor_monitor(&self) -> Result<(), String>;
}

#[cfg(target_os = "macos")]
impl<R: Runtime> WebviewWindowExt for WebviewWindow<R> {
    fn to_yeet_panel(&self) -> tauri::Result<ShareId<RawNSPanel>> {
        use cocoa::appkit::NSWindowCollectionBehavior;

        let panel = self.to_panel()?;

        // NonActivatingPanel: panel doesn't steal activation from the
        // frontmost app, which is what lets it overlay fullscreen Spaces
        // without yanking the user out.
        #[allow(non_upper_case_globals)]
        const NSWindowStyleMaskNonActivatingPanel: i32 = 1 << 7;
        panel.set_style_mask(NSWindowStyleMaskNonActivatingPanel);

        // Level 24 (kCGPopUpMenuWindowLevel) — above normal windows and
        // fullscreen apps but below screensaver/security dialogs.
        panel.set_level(24);

        // CanJoinAllSpaces: visible on every Space / desktop.
        // FullScreenAuxiliary: allowed to show alongside a fullscreen app.
        panel.set_collection_behaviour(
            NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
        );

        panel.set_hides_on_deactivate(false);

        // Delegate — sticky panel, resignKey is a no-op.
        let delegate = panel_delegate!(YeetPanelDelegate {
            window_did_resign_key
        });
        delegate.set_listener(Box::new(move |delegate_name: String| {
            if delegate_name == "window_did_resign_key" {
                log::info!("panel resigned key window (ignored — panel is sticky)");
            }
        }));
        panel.set_delegate(delegate);

        Ok(panel)
    }

    fn center_at_cursor_monitor(&self) -> Result<(), String> {
        // Prefer the monitor under the cursor so the panel follows the
        // active display (and its active Space, including fullscreen apps)
        // instead of sticking to wherever the hidden window last sat.
        let monitor = self
            .cursor_position()
            .ok()
            .and_then(|p| self.monitor_from_point(p.x, p.y).ok().flatten())
            .or_else(|| self.current_monitor().ok().flatten())
            .or_else(|| self.primary_monitor().ok().flatten())
            .ok_or_else(|| "no monitor available".to_string())?;

        let scale = monitor.scale_factor();
        let monitor_size = monitor.size().to_logical::<f64>(scale);
        let monitor_pos = monitor.position().to_logical::<f64>(scale);

        let window_size = self
            .outer_size()
            .map_err(|e| e.to_string())?
            .to_logical::<f64>(scale);

        let x = monitor_pos.x + (monitor_size.width - window_size.width) / 2.0;
        let y = monitor_pos.y + (monitor_size.height - window_size.height) / 2.0 - 50.0;

        self.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)))
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}

/// Apply native macOS vibrancy effect
#[cfg(target_os = "macos")]
pub fn set_window_blur<R: Runtime>(window: &WebviewWindow<R>, _enabled: bool) -> Result<(), String> {
    use cocoa::appkit::{NSColor, NSWindow as NSWindowTrait};
    use cocoa::base::{nil, NO, YES};
    use cocoa::foundation::NSRect;
    use objc::{class, msg_send, sel, sel_impl};

    let ns_window = match window.ns_window() {
        Ok(w) => w as id,
        Err(e) => return Err(e.to_string()),
    };

    if ns_window.is_null() {
        return Err("ns_window is null".to_string());
    }

    unsafe {
        // Make window transparent
        let _: () = msg_send![ns_window, setOpaque: NO];
        ns_window.setBackgroundColor_(NSColor::clearColor(nil));
        let _: () = msg_send![ns_window, setTitlebarAppearsTransparent: YES];

        let content_view: id = ns_window.contentView();
        if content_view.is_null() {
            return Err("content_view is null".to_string());
        }

        // Enable layer backing
        let _: () = msg_send![content_view, setWantsLayer: YES];
        let content_layer: id = msg_send![content_view, layer];
        if !content_layer.is_null() {
            let _: () = msg_send![content_layer, setCornerRadius: 10.0_f64];
            let _: () = msg_send![content_layer, setMasksToBounds: YES];
        }

        let bounds: NSRect = msg_send![content_view, bounds];

        // Create NSVisualEffectView
        let visual_effect_class = class!(NSVisualEffectView);
        let visual_effect_view: id = msg_send![visual_effect_class, alloc];
        let visual_effect_view: id = msg_send![visual_effect_view, initWithFrame: bounds];

        if visual_effect_view.is_null() {
            return Err("Failed to create NSVisualEffectView".to_string());
        }

        // Use HUDWindow material (13) - modern replacement for deprecated UltraDark
        let _: () = msg_send![visual_effect_view, setMaterial: 13_i64];
        // State active (1)
        let _: () = msg_send![visual_effect_view, setState: 1_i64];
        // Blending mode behind window (0)
        let _: () = msg_send![visual_effect_view, setBlendingMode: 0_i64];

        // Auto-resize (width | height sizable)
        let autoresizing: u64 = 2 | 16;
        let _: () = msg_send![visual_effect_view, setAutoresizingMask: autoresizing];

        // Corner radius
        let _: () = msg_send![visual_effect_view, setWantsLayer: YES];
        let layer: id = msg_send![visual_effect_view, layer];
        if !layer.is_null() {
            let _: () = msg_send![layer, setCornerRadius: 10.0_f64];
            let _: () = msg_send![layer, setMasksToBounds: YES];
        }

        // Insert behind webview (position -1 = below)
        let _: () = msg_send![content_view, addSubview: visual_effect_view positioned: -1_i64 relativeTo: nil];

        // Make webview transparent
        let subviews: id = msg_send![content_view, subviews];
        if !subviews.is_null() {
            let count: usize = msg_send![subviews, count];
            for i in 0..count {
                let subview: id = msg_send![subviews, objectAtIndex: i];
                if subview.is_null() || subview == visual_effect_view {
                    continue;
                }
                let responds: bool = msg_send![subview, respondsToSelector: sel!(setDrawsBackground:)];
                if responds {
                    let _: () = msg_send![subview, setDrawsBackground: NO];
                }
                let responds2: bool = msg_send![subview, respondsToSelector: sel!(setValue:forKey:)];
                if responds2 {
                    let key: id = msg_send![class!(NSString), stringWithUTF8String: b"drawsBackground\0".as_ptr()];
                    let no_value: id = msg_send![class!(NSNumber), numberWithBool: NO];
                    let _: () = msg_send![subview, setValue: no_value forKey: key];
                }
            }
        }

        log::info!("Native macOS vibrancy applied");
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
pub fn set_window_blur<R: Runtime>(_window: &WebviewWindow<R>, _enabled: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn fade_in_panel(panel: &ShareId<RawNSPanel>) {
    use objc::{msg_send, sel, sel_impl};
    unsafe {
        let ns_panel: id = std::mem::transmute(panel.clone());
        let _: () = msg_send![ns_panel, setAlphaValue: 0.0_f64];
        panel.show();
        let animator: id = msg_send![ns_panel, animator];
        let _: () = msg_send![animator, setAlphaValue: 1.0_f64];
    }
}

// Tauri commands

#[tauri::command]
pub async fn show_window<R: Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use crate::window::WebviewWindowExt;

        // Capture the previous frontmost app before we take focus
        if let Some(prev_app_state) = app.try_state::<PreviousAppState>() {
            prev_app_state.capture();
        }

        if let Ok(panel) = app.get_webview_panel(MAIN_WINDOW_LABEL) {
            if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                let _ = window.center_at_cursor_monitor();
            }
            app.run_on_main_thread(move || {
                fade_in_panel(&panel);
            })
            .map_err(|e| e.to_string())?;
            let _ = app.emit("panel-shown", ());
            return Ok(());
        }
    }

    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    let _ = app.emit("panel-shown", ());

    Ok(())
}

#[tauri::command]
pub async fn hide_window<R: Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    // Always exit hotkey mode when hiding
    if let Some(hotkey_state) = app.try_state::<HotkeyModeState>() {
        hotkey_state.exit();
    }

    #[cfg(target_os = "macos")]
    {
        // Get the previous app state before hiding
        let prev_app_state = app.try_state::<PreviousAppState>();

        if let Ok(panel) = app.get_webview_panel(MAIN_WINDOW_LABEL) {
            // Set guard to prevent delegate from re-entering order_out
            let hide_guard = app.try_state::<PanelHideGuard>();
            if let Some(ref guard) = hide_guard {
                guard.set_hiding();
            }

            // AppKit operations must run on the main thread
            app.run_on_main_thread(move || {
                panel.order_out(None);
            }).map_err(|e| e.to_string())?;

            if let Some(ref guard) = hide_guard {
                guard.clear_hiding();
            }

            // Restore focus to the previous app
            if let Some(state) = prev_app_state {
                state.restore();
            }

            return Ok(());
        }
    }

    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        window.hide().map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn toggle_window<R: Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use crate::window::WebviewWindowExt;
        if let Ok(panel) = app.get_webview_panel(MAIN_WINDOW_LABEL) {
            // Check visibility before running on main thread
            let is_visible = panel.is_visible();

            if is_visible {
                // Exit hotkey mode when closing
                if let Some(hotkey_state) = app.try_state::<HotkeyModeState>() {
                    hotkey_state.exit();
                }

                // Closing - get previous app state for restoration
                let prev_app_state = app.try_state::<PreviousAppState>();

                // Set guard to prevent delegate from re-entering order_out
                let hide_guard = app.try_state::<PanelHideGuard>();
                if let Some(ref guard) = hide_guard {
                    guard.set_hiding();
                }

                app.run_on_main_thread(move || {
                    panel.order_out(None);
                }).map_err(|e| e.to_string())?;

                if let Some(ref guard) = hide_guard {
                    guard.clear_hiding();
                }

                // Restore focus to previous app
                if let Some(state) = prev_app_state {
                    state.restore();
                }
            } else {
                // Opening - capture the current frontmost app first
                if let Some(prev_app_state) = app.try_state::<PreviousAppState>() {
                    prev_app_state.capture();
                }

                if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                    let _ = window.center_at_cursor_monitor();
                }

                app.run_on_main_thread(move || {
                    fade_in_panel(&panel);
                })
                .map_err(|e| e.to_string())?;
                let _ = app.emit("panel-shown", ());
            }

            return Ok(());
        }
    }

    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let is_visible = window.is_visible().map_err(|e| e.to_string())?;
        if is_visible {
            window.hide().map_err(|e| e.to_string())?;
        } else {
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
            let _ = app.emit("panel-shown", ());
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn is_window_visible<R: Runtime>(app: tauri::AppHandle<R>) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(panel) = app.get_webview_panel(MAIN_WINDOW_LABEL) {
            return Ok(panel.is_visible());
        }
    }

    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        window.is_visible().map_err(|e| e.to_string())
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub fn enter_hotkey_mode(hotkey_state: tauri::State<'_, HotkeyModeState>) {
    hotkey_state.enter();
}

#[tauri::command]
pub fn exit_hotkey_mode(
    hotkey_state: tauri::State<'_, HotkeyModeState>,
    selected_state: tauri::State<'_, SelectedItemState>,
) {
    hotkey_state.exit();
    // Clear selected item to prevent modifier-release paste after cancel.
    selected_state.take();
}

#[tauri::command]
pub fn set_selected_item(state: tauri::State<'_, SelectedItemState>, id: String) {
    state.set(id);
}

#[tauri::command]
pub fn is_hotkey_mode_active(hotkey_state: tauri::State<'_, HotkeyModeState>) -> bool {
    hotkey_state.is_active()
}

#[tauri::command]
pub fn set_settings_open(state: tauri::State<'_, SettingsOpenState>, open: bool) {
    state.set(open);
}

/// Returns true if Cmd or Shift is currently held down.
#[cfg(target_os = "macos")]
#[tauri::command]
pub fn are_modifiers_held() -> bool {
    extern "C" {
        fn CGEventSourceFlagsState(stateID: u32) -> u64;
    }
    const MASK_COMMAND: u64 = 0x100000;
    const MASK_SHIFT: u64 = 0x20000;
    let flags = unsafe { CGEventSourceFlagsState(1) };
    (flags & MASK_COMMAND != 0) || (flags & MASK_SHIFT != 0)
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub fn are_modifiers_held() -> bool {
    false
}
