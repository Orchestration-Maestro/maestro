//! Key requests call the existing native key owner.
use super::{decode, encode};
/// Runs a key operation with ordinary operands.
pub(super) fn call(operation: &str, data: &str) -> Result<String, String> {
    if operation == "keys.set_kitty_protocol_active" {
        let (active,): (bool,) = decode(data)?;
        maestro_tui::set_kitty_protocol_active(active);
        return encode(&());
    }
    if operation == "keys.is_kitty_protocol_active" {
        return encode(&maestro_tui::is_kitty_protocol_active());
    }
    if operation == "keys.matches_key" {
        let (data, key): (String, String) = decode(data)?;
        return encode(&maestro_tui::matches_key(&data, &key));
    }
    let (key,): (String,) = decode(data)?;
    match operation {
        "keys.parse_key" => return encode(&maestro_tui::keys::parse_key(&key)),
        "keys.decode_kitty_printable" => {
            return encode(&maestro_tui::keys::decode_kitty_printable(&key));
        }
        "keys.decode_printable_key" => {
            return encode(&maestro_tui::keys::decode_printable_key(&key));
        }
        "keys.is_key_release" => return encode(&maestro_tui::keys::is_key_release(&key)),
        "keys.is_key_repeat" => return encode(&maestro_tui::keys::is_key_repeat(&key)),
        _ => {}
    }
    let value = match operation {
        "keys.ctrl" => maestro_tui::Key::ctrl(&key),
        "keys.shift" => maestro_tui::Key::shift(&key),
        "keys.alt" => maestro_tui::Key::alt(&key),
        "keys.super_key" => maestro_tui::Key::super_key(&key),
        "keys.ctrl_shift" => maestro_tui::Key::ctrl_shift(&key),
        "keys.shift_ctrl" => maestro_tui::Key::shift_ctrl(&key),
        "keys.ctrl_alt" => maestro_tui::Key::ctrl_alt(&key),
        "keys.alt_ctrl" => maestro_tui::Key::alt_ctrl(&key),
        "keys.shift_alt" => maestro_tui::Key::shift_alt(&key),
        "keys.alt_shift" => maestro_tui::Key::alt_shift(&key),
        "keys.ctrl_super" => maestro_tui::Key::ctrl_super(&key),
        "keys.super_ctrl" => maestro_tui::Key::super_ctrl(&key),
        "keys.shift_super" => maestro_tui::Key::shift_super(&key),
        "keys.super_shift" => maestro_tui::Key::super_shift(&key),
        "keys.alt_super" => maestro_tui::Key::alt_super(&key),
        "keys.super_alt" => maestro_tui::Key::super_alt(&key),
        "keys.ctrl_shift_alt" => maestro_tui::Key::ctrl_shift_alt(&key),
        "keys.ctrl_shift_super" => maestro_tui::Key::ctrl_shift_super(&key),
        _ => return Err(format!("Unknown native helper: {operation}")),
    };
    encode(&value)
}
