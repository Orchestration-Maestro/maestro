//! Terminal input translated through the host's key decoder.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult, matches_key, parse_key};

/// The engine's input byte constants.
pub struct DoomKeys;
impl DoomKeys {
    /// Turn right.
    pub const KEY_RIGHTARROW: u8 = 0xae;
    /// Turn left.
    pub const KEY_LEFTARROW: u8 = 0xac;
    /// Move forward.
    pub const KEY_UPARROW: u8 = 0xad;
    /// Move backward.
    pub const KEY_DOWNARROW: u8 = 0xaf;
    /// Strafe left.
    pub const KEY_STRAFE_L: u8 = 0xa0;
    /// Strafe right.
    pub const KEY_STRAFE_R: u8 = 0xa1;
    /// Open or use.
    pub const KEY_USE: u8 = 0xa2;
    /// Fire.
    pub const KEY_FIRE: u8 = 0xa3;
    /// Escape or cancel a menu.
    pub const KEY_ESCAPE: u8 = 27;
    /// Confirm a menu choice.
    pub const KEY_ENTER: u8 = 13;
    /// Toggle the map.
    pub const KEY_TAB: u8 = 9;
    /// Function key F1.
    pub const KEY_F1: u8 = 0x80 + 0x3b;
    /// Function key F2.
    pub const KEY_F2: u8 = 0x80 + 0x3c;
    /// Function key F3.
    pub const KEY_F3: u8 = 0x80 + 0x3d;
    /// Function key F4.
    pub const KEY_F4: u8 = 0x80 + 0x3e;
    /// Function key F5.
    pub const KEY_F5: u8 = 0x80 + 0x3f;
    /// Function key F6.
    pub const KEY_F6: u8 = 0x80 + 0x40;
    /// Function key F7.
    pub const KEY_F7: u8 = 0x80 + 0x41;
    /// Function key F8.
    pub const KEY_F8: u8 = 0x80 + 0x42;
    /// Function key F9.
    pub const KEY_F9: u8 = 0x80 + 0x43;
    /// Function key F10.
    pub const KEY_F10: u8 = 0x80 + 0x44;
    /// Function key F11.
    pub const KEY_F11: u8 = 0x80 + 0x57;
    /// Function key F12.
    pub const KEY_F12: u8 = 0x80 + 0x58;
    /// Delete the preceding character.
    pub const KEY_BACKSPACE: u8 = 127;
    /// Toggle pause.
    pub const KEY_PAUSE: u8 = 0xff;
    /// Increase the screen size.
    pub const KEY_EQUALS: u8 = 0x3d;
    /// Decrease the screen size.
    pub const KEY_MINUS: u8 = 0x2d;
    /// Run.
    pub const KEY_RSHIFT: u8 = 0xb6;
    /// Right Control.
    pub const KEY_RCTRL: u8 = 0x80 + 0x1d;
    /// Right Alt.
    pub const KEY_RALT: u8 = 0x80 + 0x38;
}

/// Maps plain and enhanced terminal keys to engine bytes.
/// # Errors
/// Returns a host key-decoder error.
pub fn map_key_to_doom(api: &ExtensionAPI, data: &str) -> ExtensionResult<Vec<u8>> {
    for (raw, key, code) in [
        ("\x1b[A", "up", 0xad),
        ("\x1b[B", "down", 0xaf),
        ("\x1b[C", "right", 0xae),
        ("\x1b[D", "left", 0xac),
    ] {
        if data == raw || matches_key(api, data, key)? {
            return Ok(vec![code]);
        }
    }
    for (key, code) in [("w", 0xad), ("s", 0xaf), ("a", 0xa0), ("d", 0xa1)] {
        if data == key || matches_key(api, data, key)? {
            return Ok(vec![code]);
        }
        if data == key.to_uppercase() || matches_key(api, data, &format!("shift+{key}"))? {
            return Ok(vec![code, DoomKeys::KEY_RSHIFT]);
        }
    }
    if matches!(data, "f" | "F")
        || matches_key(api, data, "f")?
        || matches_key(api, data, "shift+f")?
    {
        return Ok(vec![DoomKeys::KEY_FIRE]);
    }
    for (raw, key, code) in [
        (" ", "space", 0xa2),
        ("\r", "enter", 13),
        ("\n", "enter", 13),
        ("\x1b", "escape", 27),
        ("\t", "tab", 9),
        ("\x7f", "backspace", 127),
        ("y", "y", b'y'),
        ("Y", "shift+y", b'y'),
        ("n", "n", b'n'),
        ("N", "shift+n", b'n'),
    ] {
        if data == raw || matches_key(api, data, key)? {
            return Ok(vec![code]);
        }
    }
    if parse_key(api, data)?.is_some_and(|key| key.starts_with("ctrl+") && key != "ctrl+c")
        || (data.len() == 1 && data.as_bytes()[0] < 32 && data != "\x03")
    {
        return Ok(vec![DoomKeys::KEY_FIRE]);
    }
    if matches!(data, "+" | "=") {
        return Ok(vec![b'=']);
    }
    let mut chars = data.chars();
    if let Some(character) = chars.next().filter(|_| chars.next().is_none()) {
        let lower = character.to_ascii_lowercase();
        if !lower.is_control()
            && let Ok(byte) = u8::try_from(u32::from(lower))
        {
            return Ok(vec![byte]);
        }
    }
    Ok(Vec::new())
}
