use crate::database::{ClipboardItem, Database};
use crate::exclusions;
use crate::keyboard;
use crate::settings::SettingsManager;
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_clipboard_manager::ClipboardExt;
use uuid::Uuid;

#[cfg(target_os = "macos")]
const YEET_BUNDLE_ID: &str = "com.yeet.app";

/// Cached state of the last seen clipboard so the poll loop can dedup.
pub struct ClipboardMonitor {
    last_hash: Mutex<Option<String>>,
}

impl ClipboardMonitor {
    pub fn new() -> Self {
        Self {
            last_hash: Mutex::new(None),
        }
    }

    pub fn init_last_hash(&self, db: &Database) {
        if let Ok(hash) = db.get_last_hash() {
            *self.last_hash.lock().unwrap() = hash;
        }
    }

    pub fn set_last_hash(&self, hash: Option<String>) {
        *self.last_hash.lock().unwrap() = hash;
    }
}

/// Pasteboard read result.
#[derive(Debug, Clone)]
pub enum PasteContent {
    Text(String),
    Url(String),
    Files(Vec<String>),
    Image { hash: String, data: Vec<u8>, source_name: Option<String> },
}

impl PasteContent {
    fn type_tag(&self) -> &'static str {
        match self {
            PasteContent::Text(s) => detect_text_type(s),
            PasteContent::Url(_) => "url",
            PasteContent::Files(_) => "file",
            PasteContent::Image { .. } => "image",
        }
    }

    fn raw_content(&self) -> String {
        match self {
            PasteContent::Text(s) => s.clone(),
            PasteContent::Url(s) => s.clone(),
            PasteContent::Files(paths) => paths.join("\n"),
            PasteContent::Image { hash, .. } => format!("[image:{}]", hash),
        }
    }
}

fn images_dir(app_data_dir: &PathBuf) -> PathBuf {
    let dir = app_data_dir.join("images");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn save_image_to_disk(app_data_dir: &PathBuf, hash: &str, data: &[u8]) -> Option<String> {
    let dir = images_dir(app_data_dir);
    let path = dir.join(format!("{}.png", hash));
    if !path.exists() {
        std::fs::write(&path, data).ok()?;
    }
    Some(path.to_string_lossy().to_string())
}

/// Check if the pasteboard contains any of the given UTI marker types.
#[cfg(target_os = "macos")]
pub fn pasteboard_has_type(type_str: &str) -> bool {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
        if pb.is_null() {
            return false;
        }
        let ns_type: id = NSString::alloc(nil).init_str(type_str);
        let types: id = msg_send![pb, types];
        if types.is_null() {
            return false;
        }
        let contains: bool = msg_send![types, containsObject: ns_type];
        contains
    }
}

/// Read the current clipboard via NSPasteboard. Prefers image → files → urls → text.
#[cfg(target_os = "macos")]
pub fn read_pasteboard() -> Option<PasteContent> {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::CStr;
    use std::os::raw::c_char;

    unsafe {
        let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
        if pb.is_null() {
            return None;
        }

        let read_string_for = |type_str: &str| -> Option<String> {
            let ns_type: id = NSString::alloc(nil).init_str(type_str);
            let s: id = msg_send![pb, stringForType: ns_type];
            if s.is_null() {
                return None;
            }
            let c: *const c_char = msg_send![s, UTF8String];
            if c.is_null() {
                return None;
            }
            CStr::from_ptr(c).to_str().ok().map(|s| s.to_string())
        };

        // Image types — check first so screenshot tools (CleanShot X, etc.)
        // that provide both image data and a file path are captured as images.
        for img_type in &["public.png", "public.tiff", "public.jpeg"] {
            let ns_type: id = NSString::alloc(nil).init_str(img_type);
            let data: id = msg_send![pb, dataForType: ns_type];
            if !data.is_null() {
                let len: usize = msg_send![data, length];
                let bytes: *const u8 = msg_send![data, bytes];
                let slice = std::slice::from_raw_parts(bytes, len);
                let mut hasher = Sha256::new();
                hasher.update(slice);
                let hash = format!("{:x}", hasher.finalize());

                // Try to grab the original filename if a file path is also present.
                let source_name = {
                    let ft: id = NSString::alloc(nil).init_str("NSFilenamesPboardType");
                    let arr: id = msg_send![pb, propertyListForType: ft];
                    if !arr.is_null() {
                        let count: usize = msg_send![arr, count];
                        if count > 0 {
                            let p: id = msg_send![arr, objectAtIndex: 0usize];
                            if !p.is_null() {
                                let c: *const c_char = msg_send![p, UTF8String];
                                if !c.is_null() {
                                    CStr::from_ptr(c).to_str().ok().map(|s| {
                                        s.rsplit('/').next().unwrap_or(s).to_string()
                                    })
                                } else { None }
                            } else { None }
                        } else { None }
                    } else { None }
                };

                return Some(PasteContent::Image { hash, data: slice.to_vec(), source_name });
            }
        }

        // Files (NSFilenamesPboardType -> property list of paths)
        {
            let ns_type: id = NSString::alloc(nil).init_str("NSFilenamesPboardType");
            let arr: id = msg_send![pb, propertyListForType: ns_type];
            if !arr.is_null() {
                let count: usize = msg_send![arr, count];
                if count > 0 {
                    let mut paths: Vec<String> = Vec::with_capacity(count);
                    for i in 0..count {
                        let p: id = msg_send![arr, objectAtIndex: i];
                        if p.is_null() {
                            continue;
                        }
                        let c: *const c_char = msg_send![p, UTF8String];
                        if c.is_null() {
                            continue;
                        }
                        if let Ok(s) = CStr::from_ptr(c).to_str() {
                            paths.push(s.to_string());
                        }
                    }
                    if !paths.is_empty() {
                        return Some(PasteContent::Files(paths));
                    }
                }
            }
        }

        // URL (public.url -> string)
        if let Some(s) = read_string_for("public.url") {
            if !s.trim().is_empty() {
                return Some(PasteContent::Url(s));
            }
        }

        // Plain text (try both modern and legacy UTIs)
        for text_type in &["public.utf8-plain-text", "NSStringPboardType"] {
            if let Some(s) = read_string_for(text_type) {
                if !s.is_empty() {
                    return Some(PasteContent::Text(s));
                }
            }
        }

        None
    }
}

#[cfg(not(target_os = "macos"))]
pub fn read_pasteboard() -> Option<PasteContent> {
    None
}

/// Detect a richer text type for plain text content (url/code/text).
fn detect_text_type(text: &str) -> &'static str {
    let trimmed = text.trim();
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("ftp://")
    {
        return "url";
    }
    if looks_like_code(trimmed) {
        return "code";
    }
    "text"
}

fn looks_like_code(text: &str) -> bool {
    let indicators = [
        "function ", "const ", "let ", "var ", "import ", "export ", "class ", "def ",
        "fn ", "pub ", "async ", "await ", "return ", "if (", "for (", "while (",
        "=>", "->", "();",
    ];
    let lower = text.to_lowercase();
    indicators.iter().filter(|i| lower.contains(*i)).count() >= 2
}

fn compute_hash(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

fn create_text_preview(text: &str) -> String {
    let preview: String = text
        .chars()
        .take(500)
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect();
    if text.len() > 500 {
        format!("{}...", preview)
    } else {
        preview
    }
}

/// Core capture: read the pasteboard, dedupe by content hash, gate on
/// excluded_apps / excluded_types / Yeet-frontmost, and insert a new item.
pub fn capture_clipboard<R: Runtime>(app: &AppHandle<R>) -> Result<Option<ClipboardItem>, String> {
    let monitor = app
        .try_state::<ClipboardMonitor>()
        .ok_or_else(|| "ClipboardMonitor not initialized".to_string())?;

    // Check NSPasteboard marker types (transient, autogenerated, concealed).
    #[cfg(target_os = "macos")]
    if let Some(mgr) = app.try_state::<SettingsManager>() {
        let s = mgr.get();
        if s.ignore_transient && pasteboard_has_type("org.nspasteboard.TransientType") {
            return Ok(None);
        }
        if s.ignore_autogenerated && pasteboard_has_type("org.nspasteboard.AutoGeneratedType") {
            return Ok(None);
        }
        if s.ignore_concealed && pasteboard_has_type("org.nspasteboard.ConcealedType") {
            return Ok(None);
        }
    }

    let Some(content) = read_pasteboard() else {
        return Ok(None);
    };

    let raw = content.raw_content();
    if raw.is_empty() {
        return Ok(None);
    }
    let hash = compute_hash(&raw);

    // Dedup: skip if content hash matches what we last captured.
    {
        let last = monitor.last_hash.lock().unwrap();
        if last.as_ref() == Some(&hash) {
            return Ok(None);
        }
    }

    let content_type = content.type_tag().to_string();

    // Frontmost-app gating: skip if Yeet is frontmost or app is excluded.
    // Don't update last_hash here so the item gets captured once the app is no longer frontmost.
    if let Some(frontmost) = exclusions::get_frontmost_app() {
        #[cfg(target_os = "macos")]
        if frontmost.eq_ignore_ascii_case(YEET_BUNDLE_ID) {
            return Ok(None);
        }
        if let Some(mgr) = app.try_state::<SettingsManager>() {
            let s = mgr.get();
            let excluded_app = s
                .excluded_apps
                .iter()
                .any(|e| !e.is_empty() && frontmost.to_lowercase().contains(&e.to_lowercase()));
            if excluded_app {
                return Ok(None);
            }
        }
    }

    // Type gating
    if let Some(mgr) = app.try_state::<SettingsManager>() {
        let s = mgr.get();
        if s.excluded_types.iter().any(|t| t == &content_type) {
            return Ok(None);
        }
    }

    // For images, save the data to disk and use the file path as content.
    let (final_content, preview) = match &content {
        PasteContent::Image { hash: img_hash, data, source_name } => {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())?;
            let path = save_image_to_disk(&app_data_dir, img_hash, data)
                .unwrap_or_else(|| raw.clone());
            let preview = source_name.clone().unwrap_or_else(|| "[image]".to_string());
            (path, preview)
        }
        PasteContent::Text(t) => (raw.clone(), create_text_preview(t)),
        PasteContent::Url(u) => (raw.clone(), create_text_preview(u)),
        PasteContent::Files(paths) => (raw.clone(), paths.join("\n")),
    };

    let item = ClipboardItem {
        id: Uuid::new_v4().to_string(),
        content_type,
        content: final_content,
        preview,
        hash: hash.clone(),
        created_at: Utc::now(),
    };

    let db = app
        .try_state::<Database>()
        .ok_or_else(|| "Database not initialized".to_string())?;
    db.insert_item(&item).map_err(|e| e.to_string())?;

    let history_limit = app
        .try_state::<SettingsManager>()
        .map(|m| m.get().history_limit)
        .unwrap_or(100);
    db.enforce_limit(history_limit).map_err(|e| e.to_string())?;

    monitor.set_last_hash(Some(hash));
    let _ = app.emit("clipboard-changed", &item);

    log::info!("[capture] stored {} item", item.content_type);
    Ok(Some(item))
}

#[tauri::command]
pub async fn check_clipboard<R: Runtime>(app: AppHandle<R>) -> Result<Option<ClipboardItem>, String> {
    capture_clipboard(&app)
}

// ---- Tauri commands the UI uses ----

#[tauri::command]
pub async fn get_clipboard_items(
    db: tauri::State<'_, Database>,
    limit: u32,
    offset: u32,
    search: Option<String>,
) -> Result<Vec<ClipboardItem>, String> {
    db.get_items(limit, offset, search.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_clipboard_item(
    db: tauri::State<'_, Database>,
    id: String,
) -> Result<(), String> {
    db.delete_item(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_history(db: tauri::State<'_, Database>) -> Result<(), String> {
    db.clear_history().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn paste_item<R: Runtime>(
    app: AppHandle<R>,
    db: tauri::State<'_, Database>,
    id: String,
) -> Result<(), String> {
    let item = db.get_item(&id).map_err(|e| e.to_string())?;
    if let Some(item) = item {
        if item.content_type == "image" {
            let data = resolve_image_bytes(&app, &item.content)?;
            write_image_to_clipboard(&data)?;
        } else {
            app.clipboard()
                .write_text(&item.content)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_image_base64<R: Runtime>(
    app: AppHandle<R>,
    db: tauri::State<'_, Database>,
    id: String,
) -> Result<Option<String>, String> {
    let item = db.get_item(&id).map_err(|e| e.to_string())?;
    let Some(item) = item else { return Ok(None) };
    if item.content_type != "image" {
        return Ok(None);
    }

    use base64::Engine;

    // New format: content is a file path
    let path = std::path::Path::new(&item.content);
    if path.exists() && path.is_file() {
        let data = std::fs::read(path).map_err(|e| e.to_string())?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
        return Ok(Some(format!("data:image/png;base64,{}", b64)));
    }

    // Old format: content is [image:{hash}] — try to find the file by hash
    if item.content.starts_with("[image:") && item.content.ends_with(']') {
        let hash = &item.content[7..item.content.len() - 1];
        let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        let candidate = app_data_dir.join("images").join(format!("{}.png", hash));
        if candidate.exists() {
            let data = std::fs::read(&candidate).map_err(|e| e.to_string())?;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
            return Ok(Some(format!("data:image/png;base64,{}", b64)));
        }
    }

    log::warn!("[get_image_base64] image not found for item {}: content={}", id, &item.content[..item.content.len().min(100)]);
    Ok(None)
}

/// Write `id`'s content to the clipboard, hide the panel, restore focus, and
/// simulate Cmd+V — used when the user picks an item from the panel.
pub async fn do_paste_and_simulate<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    // Consume selected item so the modifier-release watcher won't double-paste.
    if let Some(sel) = app.try_state::<crate::window::SelectedItemState>() {
        sel.take();
    }

    let item = {
        let db = app.state::<Database>();
        db.get_item(&id).map_err(|e| e.to_string())?
    };

    if let Some(item) = item {
        eprintln!("[paste] type={} content={}", item.content_type, &item.content[..item.content.len().min(120)]);

        let write_ok = if item.content_type == "image" {
            match write_image_to_clipboard_and_remember(&app, &item.content) {
                Ok(()) => { eprintln!("[paste] image clipboard write OK"); true }
                Err(e) => { eprintln!("[paste] image clipboard write FAILED: {}", e); false }
            }
        } else {
            match write_to_clipboard_and_remember(&app, &item.content) {
                Ok(()) => true,
                Err(e) => { eprintln!("[paste] text clipboard write FAILED: {}", e); false }
            }
        };

        // Always hide the window, even if the clipboard write failed,
        // so the panel doesn't get stuck visible with stale state.
        crate::window::hide_window(app.clone()).await?;

        if write_ok {
            eprintln!("[paste] window hidden, sleeping 150ms");
            tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

            #[cfg(target_os = "macos")]
            {
                eprintln!("[paste] firing simulate_cmd_v (ax_trusted={})", keyboard::ax_is_trusted());
                if let Err(e) = keyboard::simulate_cmd_v() {
                    eprintln!("[paste] simulate_cmd_v FAILED: {}", e);
                } else {
                    eprintln!("[paste] simulate_cmd_v OK");
                }
            }
        }
    } else {
        eprintln!("[paste] item not found for id={}", id);
    }

    // Restore the selection so the next panel open remembers what was pasted.
    if let Some(sel) = app.try_state::<crate::window::SelectedItemState>() {
        sel.set(id);
    }

    // Exit hotkey mode AFTER the paste completes so the modifier-release
    // watcher can't re-register the interceptor mid-flight.
    if let Some(state) = app.try_state::<crate::window::HotkeyModeState>() {
        state.exit();
    }

    Ok(())
}

#[tauri::command]
pub async fn paste_and_simulate<R: Runtime>(
    app: AppHandle<R>,
    _db: tauri::State<'_, Database>,
    id: String,
) -> Result<(), String> {
    do_paste_and_simulate(app, id).await
}


/// Write `text` to the clipboard and tell the monitor to ignore the resulting
/// changeCount bump so we don't re-record what we just wrote.
fn write_to_clipboard_and_remember<R: Runtime>(
    app: &AppHandle<R>,
    text: &str,
) -> Result<(), String> {
    app.clipboard()
        .write_text(text)
        .map_err(|e| e.to_string())?;

    let hash = compute_hash(text);
    if let Some(monitor) = app.try_state::<ClipboardMonitor>() {
        monitor.set_last_hash(Some(hash));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn write_image_to_clipboard(data: &[u8]) -> Result<(), String> {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
        let _: () = msg_send![pb, clearContents];
        let ns_data: id = msg_send![class!(NSData), dataWithBytes:data.as_ptr() length:data.len()];
        let png_type: id = NSString::alloc(nil).init_str("public.png");
        let ok: bool = msg_send![pb, setData:ns_data forType:png_type];
        if !ok {
            return Err("NSPasteboard setData:forType: returned false".into());
        }

    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn write_image_to_clipboard(_data: &[u8]) -> Result<(), String> {
    Err("Image clipboard write not supported on this platform".to_string())
}

/// Resolve the image content string to actual bytes on disk.
/// Handles both formats: direct file path and legacy `[image:{hash}]`.
fn resolve_image_bytes<R: Runtime>(app: &AppHandle<R>, content: &str) -> Result<Vec<u8>, String> {
    let path = std::path::Path::new(content);
    if path.exists() && path.is_file() {
        log::info!("[paste-image] reading image from path: {}", content);
        return std::fs::read(path).map_err(|e| format!("read image file: {}", e));
    }

    if content.starts_with("[image:") && content.ends_with(']') {
        let hash = &content[7..content.len() - 1];
        let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        let candidate = app_data_dir.join("images").join(format!("{}.png", hash));
        if candidate.exists() {
            log::info!("[paste-image] resolved old-format hash to: {}", candidate.display());
            return std::fs::read(&candidate).map_err(|e| format!("read image file: {}", e));
        }
    }

    Err(format!("image file not found for content: {}", &content[..content.len().min(80)]))
}

fn write_image_to_clipboard_and_remember<R: Runtime>(
    app: &AppHandle<R>,
    content: &str,
) -> Result<(), String> {
    let data = resolve_image_bytes(app, content)?;

    write_image_to_clipboard(&data)?;
    log::info!("[paste-image] wrote {} bytes to clipboard", data.len());

    let mut hasher = Sha256::new();
    hasher.update(&data);
    let img_hash = format!("{:x}", hasher.finalize());
    let raw = format!("[image:{}]", img_hash);
    let hash = compute_hash(&raw);

    if let Some(monitor) = app.try_state::<ClipboardMonitor>() {
        monitor.set_last_hash(Some(hash));
    }
    Ok(())
}
