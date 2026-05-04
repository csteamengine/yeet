#[cfg(target_os = "macos")]
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
#[cfg(target_os = "macos")]
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

#[cfg(target_os = "macos")]
const KEY_V: CGKeyCode = 9;

#[cfg(target_os = "macos")]
pub fn ax_is_trusted() -> bool {
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    unsafe { AXIsProcessTrusted() }
}

#[cfg(target_os = "macos")]
fn prompt_accessibility_once() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static PROMPTED: AtomicBool = AtomicBool::new(false);
    if PROMPTED.swap(true, Ordering::Relaxed) {
        return;
    }

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
    log::info!("[keyboard] prompted for Accessibility, trusted: {}", trusted);
}

#[cfg(target_os = "macos")]
pub fn simulate_cmd_v() -> Result<(), String> {
    if !ax_is_trusted() {
        log::warn!("[keyboard] Accessibility not granted, prompting user and trying osascript");
        prompt_accessibility_once();
        return osascript_cmd_v();
    }
    if cgevent_cmd_v().is_ok() {
        return Ok(());
    }
    log::warn!("[keyboard] CGEvent paste failed, falling back to osascript");
    osascript_cmd_v()
}

#[cfg(target_os = "macos")]
fn cgevent_cmd_v() -> Result<(), String> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| "Failed to create CGEventSource")?;

    let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
        .map_err(|_| "Failed to create key down event")?;
    let key_up = CGEvent::new_keyboard_event(source, KEY_V, false)
        .map_err(|_| "Failed to create key up event")?;

    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);

    key_down.post(CGEventTapLocation::HID);
    key_up.post(CGEventTapLocation::HID);

    Ok(())
}

#[cfg(target_os = "macos")]
fn osascript_cmd_v() -> Result<(), String> {
    std::process::Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to keystroke \"v\" using command down")
        .spawn()
        .map_err(|e| format!("osascript failed: {}", e))?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn simulate_cmd_v() -> Result<(), String> {
    Err("Keyboard simulation not implemented for this platform".to_string())
}
