//! Host-only authored path observations.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionResult,
    tools::path_utils::{expand_path, resolve_read_path, resolve_to_cwd},
};
use serde_json::{Value, json};
/// Reads a native expansion or resolution without guest filesystem access.
pub(super) fn observe(api: &ExtensionAPI, request: &Value) -> ExtensionResult<Value> {
    let path = request["args"][0].as_str().ok_or("missing path")?;
    match request["operation"].as_str() {
        Some("p.expandPath") => Ok(json!(expand_path(api, path)?)),
        Some("p.resolveToCwd") => Ok(json!(resolve_to_cwd(
            api,
            path,
            request["args"][1].as_str().ok_or("missing cwd")?
        )?)),
        Some("p.resolveReadPath") => Ok(json!(resolve_read_path(
            api,
            path,
            request["args"][1].as_str().ok_or("missing cwd")?
        )?)),
        _ => Err("unknown helper observation".into()),
    }
}
