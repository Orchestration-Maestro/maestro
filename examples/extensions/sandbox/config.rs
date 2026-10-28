//! Layered sandbox configuration read through the extension's supplied directories.
use maestro_extensions_wasm::{
    ExtensionAPI,
    config::get_agent_dir,
    tools::path_utils::{guest_path, resolve},
};
use serde_json::Value;

pub(crate) use super::config_data::{FilesystemConfig, SandboxConfig};
use super::config_data::{defaults, merge};

/// Applies global then project configuration, replacing arrays rather than appending.
pub(crate) fn load(api: &ExtensionAPI, cwd: &str) -> Result<SandboxConfig, String> {
    let mut value = defaults();
    let global = resolve(api, &[&get_agent_dir(api)?, "extensions", "sandbox.json"])?;
    let project = resolve(api, &[cwd, ".maestro", "sandbox.json"])?;
    for path in [global, project] {
        match read(api, &path) {
            Ok(Some(layer)) => merge(&mut value, layer),
            Ok(None) => {}
            Err(error) => eprintln!("Cannot read sandbox configuration {path}: {error}"),
        }
    }
    serde_json::from_value(value).map_err(|error| error.to_string())
}

/// Distinguishes an absent file from an unreadable or malformed configuration.
fn read(api: &ExtensionAPI, path: &str) -> Result<Option<Value>, String> {
    let text = match std::fs::read_to_string(guest_path(api, path)?) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let value = maestro_json::object::from_str(&text).map_err(|error| error.to_string())?;
    Ok(Some(value))
}

pub(crate) use super::config_data::display;
