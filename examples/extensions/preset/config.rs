//! Layered preset files read through the host's authored-path helpers.
use indexmap::IndexMap;
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult, ThinkingLevel, tools::path_utils};
use serde::Deserialize;

/// Optional settings supplied by one named preset.
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Preset {
    /// Provider of the requested model.
    pub provider: Option<String>,
    /// Exact model identifier in the configured registry.
    pub model: Option<String>,
    /// Requested reasoning effort.
    pub thinking_level: Option<ThinkingLevel>,
    /// Tool names replacing the active selection when any are valid.
    pub tools: Option<Vec<String>>,
    /// Instructions appended to the supplied system prompt.
    pub instructions: Option<String>,
}

/// Presets retain file order; a project entry replaces the whole global entry.
pub(super) type Presets = IndexMap<String, Preset>;

/// Reads both layers, reporting an unreadable or invalid file without losing the other layer.
pub(super) fn load(api: &ExtensionAPI, cwd: &str) -> ExtensionResult<Presets> {
    let agent = std::env::var("MAESTRO_CODING_AGENT_DIR")
        .ok()
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| "~/.maestro/agent".into());
    let global =
        path_utils::resolve_to_cwd(api, "presets.json", &path_utils::expand_path(api, &agent)?)?;
    let project = path_utils::resolve_to_cwd(api, ".maestro/presets.json", cwd)?;
    let mut presets = Presets::new();
    for (layer, path) in [("global", global), ("project", project)] {
        match std::fs::read(&path) {
            Ok(text) => match serde_json::from_str::<Presets>(&String::from_utf8_lossy(&text)) {
                Ok(values) => presets.extend(values),
                Err(error) => eprintln!("Failed to load {layer} presets from {path}: {error}"),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => eprintln!("Failed to load {layer} presets from {path}: {error}"),
        }
    }
    Ok(presets)
}
