//! The system-wide summon key: one key that brings the app forward from
//! anywhere, and hides it again when it is already in front. macOS only,
//! through Carbon's `RegisterEventHotKey`, which needs no accessibility
//! permission.

/// Used when `global_hotkey` is unset in desktop.toml.
pub const DEFAULT: &str = "ctrl-alt-f";

/// Carbon modifier masks.
const CMD: u32 = 1 << 8;
const SHIFT: u32 = 1 << 9;
const OPTION: u32 = 1 << 11;
const CONTROL: u32 = 1 << 12;

/// `"ctrl-alt-f"` → (macOS virtual key code, Carbon modifiers). Needs at
/// least one modifier, so a plain letter is never taken from every app.
pub fn parse(spec: &str) -> Result<(u32, u32), String> {
    let parts: Vec<&str> = spec.split('-').map(str::trim).collect();
    let Some((key, modifiers)) = parts.split_last() else {
        return Err("empty key".into());
    };
    let mut mask = 0;
    for modifier in modifiers {
        mask |= match modifier.to_lowercase().as_str() {
            "cmd" | "command" | "super" => CMD,
            "shift" => SHIFT,
            "alt" | "option" | "opt" => OPTION,
            "ctrl" | "control" => CONTROL,
            other => return Err(format!("unknown modifier \"{other}\" in \"{spec}\"")),
        };
    }
    if mask == 0 {
        return Err(format!(
            "\"{spec}\" needs a modifier (cmd, ctrl, alt or shift)"
        ));
    }
    let code = key_code(&key.to_lowercase())
        .ok_or_else(|| format!("unknown key \"{key}\" in \"{spec}\""))?;
    Ok((code, mask))
}

/// ANSI-layout virtual key codes (`kVK_*` in Carbon's Events.h).
fn key_code(key: &str) -> Option<u32> {
    const LETTERS: [u32; 26] = [
        0x00, 0x0B, 0x08, 0x02, 0x0E, 0x03, 0x05, 0x04, 0x22, 0x26, 0x28, 0x25, 0x2E, 0x2D, 0x1F,
        0x23, 0x0C, 0x0F, 0x01, 0x11, 0x20, 0x09, 0x0D, 0x07, 0x10, 0x06,
    ];
    const DIGITS: [u32; 10] = [0x1D, 0x12, 0x13, 0x14, 0x15, 0x17, 0x16, 0x1A, 0x1C, 0x19];
    const F_KEYS: [u32; 12] = [
        0x7A, 0x78, 0x63, 0x76, 0x60, 0x61, 0x62, 0x64, 0x65, 0x6D, 0x67, 0x6F,
    ];
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_lowercase() {
            return Some(LETTERS[(c as u8 - b'a') as usize]);
        }
        if let Some(d) = c.to_digit(10) {
            return Some(DIGITS[d as usize]);
        }
    }
    if let Some(n) = key.strip_prefix('f').and_then(|n| n.parse::<usize>().ok()) {
        return F_KEYS.get(n.checked_sub(1)?).copied();
    }
    Some(match key {
        "space" => 0x31,
        "enter" | "return" => 0x24,
        "tab" => 0x30,
        "escape" | "esc" => 0x35,
        "`" | "backtick" => 0x32,
        "," => 0x2B,
        "." => 0x2F,
        "/" => 0x2C,
        ";" => 0x29,
        "'" => 0x27,
        "[" => 0x21,
        "]" => 0x1E,
        "-" | "minus" => 0x1B,
        "=" => 0x18,
        "\\" => 0x2A,
        _ => return None,
    })
}

/// Registers `spec`; the receiver gets a message on each press. Call it
/// once, on the main thread, after the application has started.
#[cfg(target_os = "macos")]
pub fn register(spec: &str) -> Result<async_channel::Receiver<()>, String> {
    use std::ffi::c_void;
    use std::sync::OnceLock;

    #[repr(C)]
    struct EventHotKeyId {
        signature: u32,
        id: u32,
    }
    #[repr(C)]
    struct EventTypeSpec {
        class: u32,
        kind: u32,
    }
    type Handler = extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32;
    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        fn GetApplicationEventTarget() -> *mut c_void;
        fn RegisterEventHotKey(
            code: u32,
            modifiers: u32,
            id: EventHotKeyId,
            target: *mut c_void,
            options: u32,
            out: *mut *mut c_void,
        ) -> i32;
        fn InstallEventHandler(
            target: *mut c_void,
            handler: Handler,
            count: usize,
            types: *const EventTypeSpec,
            data: *mut c_void,
            out: *mut *mut c_void,
        ) -> i32;
    }

    static PRESSES: OnceLock<async_channel::Sender<()>> = OnceLock::new();
    extern "C" fn on_press(_: *mut c_void, _: *mut c_void, _: *mut c_void) -> i32 {
        if let Some(sender) = PRESSES.get() {
            let _ = sender.try_send(());
        }
        0
    }

    let (code, modifiers) = parse(spec)?;
    let (sender, receiver) = async_channel::bounded(4);
    if PRESSES.set(sender).is_err() {
        return Err("the summon key is already registered".into());
    }
    let pressed = EventTypeSpec {
        class: u32::from_be_bytes(*b"keyb"),
        kind: 5,
    };
    let id = EventHotKeyId {
        signature: u32::from_be_bytes(*b"fbbk"),
        id: 1,
    };
    let (mut handler, mut hotkey) = (std::ptr::null_mut(), std::ptr::null_mut());
    // SAFETY: plain Carbon calls on the main thread with valid pointers; the
    // handler is a 'static fn and the hotkey stays registered for the life
    // of the process.
    unsafe {
        let target = GetApplicationEventTarget();
        let status = InstallEventHandler(
            target,
            on_press,
            1,
            &pressed,
            std::ptr::null_mut(),
            &mut handler,
        );
        if status != 0 {
            return Err(format!(
                "could not listen for the summon key (error {status})"
            ));
        }
        let status = RegisterEventHotKey(code, modifiers, id, target, 0, &mut hotkey);
        if status != 0 {
            return Err(format!(
                "\"{spec}\" is taken by another app (error {status})"
            ));
        }
    }
    Ok(receiver)
}

#[cfg(not(target_os = "macos"))]
pub fn register(_spec: &str) -> Result<async_channel::Receiver<()>, String> {
    Err("a system-wide summon key is only supported on macOS".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modifiers_and_keys() {
        // Given
        let specs = ["ctrl-alt-f", "cmd-shift-space", "Option-F5", "ctrl-1"];

        // When
        let parsed: Vec<_> = specs.iter().map(|s| parse(s)).collect();

        // Then
        assert_eq!(
            parsed,
            [
                Ok((0x03, CONTROL | OPTION)),
                Ok((0x31, CMD | SHIFT)),
                Ok((0x60, OPTION)),
                Ok((0x12, CONTROL))
            ]
        );
    }

    #[test]
    fn rejects_bare_keys_and_unknown_names() {
        // Given
        let specs = ["f", "hyper-f", "ctrl-nope", "ctrl-f13"];

        // When
        let errors: Vec<bool> = specs.iter().map(|s| parse(s).is_err()).collect();

        // Then
        assert_eq!(errors, [true, true, true, true]);
    }
}
