#[cfg(target_os = "macos")]
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
#[cfg(target_os = "macos")]
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

#[cfg(target_os = "macos")]
const KEY_V: CGKeyCode = 9;

#[cfg(target_os = "macos")]
pub fn simulate_cmd_v() -> Result<(), String> {
    // Try CGEvent first (works in dev builds and when Accessibility is properly granted)
    if cgevent_cmd_v().is_ok() {
        return Ok(());
    }
    log::warn!("[keyboard] CGEvent paste failed, falling back to osascript");
    osascript_cmd_v()
}

#[cfg(target_os = "macos")]
fn cgevent_cmd_v() -> Result<(), String> {
    let source = CGEventSource::new(CGEventSourceStateID::Private)
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
