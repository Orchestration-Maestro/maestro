//! Compiled author key observations.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult, Key};
use serde_json::{Value, json};
/// Reads a complete key observation.
pub(super) fn observe(api: &ExtensionAPI, request: &Value) -> ExtensionResult<Value> {
    if let Some(active) = request["context"]["kittyProtocolActive"].as_bool() {
        maestro_extensions_wasm::keys::set_kitty_protocol_active(api, active)?;
    }
    match request["operation"].as_str() {
        Some("k.Key") => vocabulary(api),
        Some("k.isKittyProtocolActive") => Ok(json!(
            maestro_extensions_wasm::keys::is_kitty_protocol_active(&api.clone())?
        )),
        Some("k.parseKey") => Ok(json!(maestro_extensions_wasm::keys::parse_key(
            api,
            request["args"][0].as_str().ok_or("missing key data")?
        )?)),
        Some("k.decodeKittyPrintable") => Ok(json!(
            maestro_extensions_wasm::keys::decode_kitty_printable(
                api,
                request["args"][0].as_str().ok_or("missing key data")?
            )?
        )),
        Some("k.decodePrintableKey") => {
            Ok(json!(maestro_extensions_wasm::keys::decode_printable_key(
                api,
                request["args"][0].as_str().ok_or("missing key data")?
            )?))
        }
        Some("k.isKeyRelease") => Ok(json!(maestro_extensions_wasm::keys::is_key_release(
            api,
            request["args"][0].as_str().ok_or("missing key data")?
        )?)),
        Some("k.isKeyRepeat") => Ok(json!(maestro_extensions_wasm::keys::is_key_repeat(
            api,
            request["args"][0].as_str().ok_or("missing key data")?
        )?)),
        Some("k.matchesKey") => Ok(json!(maestro_extensions_wasm::keys::matches_key(
            api,
            request["args"][0].as_str().ok_or("missing key data")?,
            request["args"][1]
                .as_str()
                .ok_or("missing key identifier")?
        )?)),
        _ => Err("unknown helper observation".into()),
    }
}
/// The declared key names, without a second decoder.
const NAMES: &[(&str, &str)] = &[
    ("escape", Key::ESCAPE),
    ("esc", Key::ESC),
    ("enter", Key::ENTER),
    ("return", Key::RETURN),
    ("tab", Key::TAB),
    ("space", Key::SPACE),
    ("backspace", Key::BACKSPACE),
    ("delete", Key::DELETE),
    ("insert", Key::INSERT),
    ("clear", Key::CLEAR),
    ("home", Key::HOME),
    ("end", Key::END),
    ("pageUp", Key::PAGE_UP),
    ("pageDown", Key::PAGE_DOWN),
    ("up", Key::UP),
    ("down", Key::DOWN),
    ("left", Key::LEFT),
    ("right", Key::RIGHT),
    ("f1", Key::F1),
    ("f2", Key::F2),
    ("f3", Key::F3),
    ("f4", Key::F4),
    ("f5", Key::F5),
    ("f6", Key::F6),
    ("f7", Key::F7),
    ("f8", Key::F8),
    ("f9", Key::F9),
    ("f10", Key::F10),
    ("f11", Key::F11),
    ("f12", Key::F12),
    ("backtick", Key::BACKTICK),
    ("hyphen", Key::HYPHEN),
    ("equals", Key::EQUALS),
    ("leftbracket", Key::LEFTBRACKET),
    ("rightbracket", Key::RIGHTBRACKET),
    ("backslash", Key::BACKSLASH),
    ("semicolon", Key::SEMICOLON),
    ("quote", Key::QUOTE),
    ("comma", Key::COMMA),
    ("period", Key::PERIOD),
    ("slash", Key::SLASH),
    ("exclamation", Key::EXCLAMATION),
    ("at", Key::AT),
    ("hash", Key::HASH),
    ("dollar", Key::DOLLAR),
    ("percent", Key::PERCENT),
    ("caret", Key::CARET),
    ("ampersand", Key::AMPERSAND),
    ("asterisk", Key::ASTERISK),
    ("leftparen", Key::LEFTPAREN),
    ("rightparen", Key::RIGHTPAREN),
    ("underscore", Key::UNDERSCORE),
    ("plus", Key::PLUS),
    ("pipe", Key::PIPE),
    ("tilde", Key::TILDE),
    ("leftbrace", Key::LEFTBRACE),
    ("rightbrace", Key::RIGHTBRACE),
    ("colon", Key::COLON),
    ("lessthan", Key::LESSTHAN),
    ("greaterthan", Key::GREATERTHAN),
    ("question", Key::QUESTION),
];
/// One native modifier constructor.
type Builder = fn(&ExtensionAPI, &str) -> ExtensionResult<String>;
/// The native modifier methods the fixture calls independently.
const BUILDERS: &[(&str, Builder)] = &[
    ("ctrl", Key::ctrl),
    ("shift", Key::shift),
    ("alt", Key::alt),
    ("super", Key::super_key),
    ("ctrlShift", Key::ctrl_shift),
    ("shiftCtrl", Key::shift_ctrl),
    ("ctrlAlt", Key::ctrl_alt),
    ("altCtrl", Key::alt_ctrl),
    ("shiftAlt", Key::shift_alt),
    ("altShift", Key::alt_shift),
    ("ctrlSuper", Key::ctrl_super),
    ("superCtrl", Key::super_ctrl),
    ("shiftSuper", Key::shift_super),
    ("superShift", Key::super_shift),
    ("altSuper", Key::alt_super),
    ("superAlt", Key::super_alt),
    ("ctrlShiftAlt", Key::ctrl_shift_alt),
    ("ctrlShiftSuper", Key::ctrl_shift_super),
];
/// Reads every literal and every native modifier constructor.
fn vocabulary(api: &ExtensionAPI) -> ExtensionResult<Value> {
    let mut result = serde_json::Map::new();
    for (name, key) in NAMES {
        result.insert((*name).into(), json!(key));
    }
    for (name, builder) in BUILDERS {
        result.insert((*name).into(), json!(builder(api, "x")?));
    }
    Ok(Value::Object(result))
}
