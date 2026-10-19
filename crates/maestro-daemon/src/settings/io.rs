//! Global-only workspace projection through the existing settings owner.

mod ignored;
mod reload;

pub use reload::{config_diagnostic_summary, load_config, reload_config};

use std::{cell::RefCell, path::Path};

use maestro_settings::Settings;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use super::model::{Config, LoadedConfig};
use ignored::{Decoder, Segment, format_path, path_key};

/// Client experimental keys stored alongside interface preferences.
const EXPERIMENTAL_KEYS: &[&str] = &[
    "allowNested",
    "kittyGraphics",
    "revealHiddenCursorForCjkIme",
    "cjkImeAgents",
    "cjkImeCursorShape",
    "switchAsciiInputSourceInPrefix",
];
/// Daemon keys whose original semantic areas remain independent.
const DAEMON_KEYS: &[&str] = &[
    "paneShell",
    "paneShellMode",
    "newPaneCwd",
    "resumeAgentsOnRestore",
    "headlessCols",
    "headlessRows",
    "scrollbackLimitBytes",
    "paneHistory",
    "worktreesDirectory",
    "detectionCatalogCheck",
];

/// Decode only the accepted global document without changing its raw members.
#[must_use]
pub fn project_global_settings(settings: &Settings, agent_dir: &Path) -> LoadedConfig {
    let mut loaded = LoadedConfig {
        config: Config::default(),
        diagnostics: Vec::new(),
        invalid_sections: Vec::new(),
    };
    project_areas(settings, &mut loaded);
    unknown_namespace_keys(
        settings.0.get("daemon"),
        "daemon",
        DAEMON_KEYS,
        &mut loaded.diagnostics,
    );
    if !loaded.invalid_sections.iter().any(|area| area == "update") {
        unknown_namespace_keys(
            settings.0.get("update"),
            "update",
            &["channel", "versionCheck"],
            &mut loaded.diagnostics,
        );
    }
    loaded
        .diagnostics
        .extend(loaded.config.collect_diagnostics(agent_dir));
    loaded
}

/// Decode each original area independently after selecting its stored keys.
fn project_areas(settings: &Settings, loaded: &mut LoadedConfig) {
    let mux = settings.0.get("multiplexer");
    let daemon = settings.0.get("daemon");
    let update = settings.0.get("update");
    macro_rules! area {
        ($field:ident, $root:literal, $value:expr) => {
            if let Some(value) = decode_area(&$value, $root, stringify!($field), loaded) {
                loaded.config.$field = value;
            }
        };
    }
    project_onboarding(mux, loaded);
    area!(theme, "multiplexer.theme", nested(mux, "theme", empty()));
    area!(remote, "multiplexer.remote", nested(mux, "remote", empty()));
    area!(ui, "multiplexer", ui_values(mux));
    area!(
        terminal,
        "daemon",
        selected(daemon, &["paneShell", "paneShellMode", "newPaneCwd"])
    );
    area!(
        session,
        "daemon",
        selected(daemon, &["resumeAgentsOnRestore"])
    );
    area!(
        server,
        "daemon",
        selected(daemon, &["headlessCols", "headlessRows"])
    );
    area!(
        advanced,
        "daemon",
        selected(daemon, &["scrollbackLimitBytes"])
    );
    area!(
        worktrees,
        "daemon",
        selected(daemon, &["worktreesDirectory"])
    );
    area!(
        update,
        "update",
        combined(
            selected(update, &["channel", "versionCheck"]),
            selected(daemon, &["detectionCatalogCheck"])
        )
    );
    area!(
        experimental,
        "multiplexer",
        combined(
            selected(mux, EXPERIMENTAL_KEYS),
            selected(daemon, &["paneHistory"])
        )
    );
}

/// Optional onboarding still requires its enclosing namespace to be an object.
fn project_onboarding(mux: Option<&Value>, loaded: &mut LoadedConfig) {
    if mux.is_some_and(|value| !value.is_object()) {
        reject_area("onboarding", "multiplexer must be an object", loaded);
    } else if let Some(value) = decode_area(
        &nested(mux, "onboarding", Value::Null),
        "multiplexer.onboarding",
        "onboarding",
        loaded,
    ) {
        loaded.config.onboarding = value;
    }
}

/// An absent record is decoded using its own field defaults.
fn empty() -> Value {
    Value::Object(Map::new())
}

/// Select a nested record, preserving a malformed container for typed rejection.
fn nested(container: Option<&Value>, key: &str, fallback: Value) -> Value {
    match container {
        None => fallback,
        Some(Value::Object(values)) => values.get(key).cloned().unwrap_or(fallback),
        Some(value) => value.clone(),
    }
}

/// Select flattened keys for one original semantic area.
fn selected(container: Option<&Value>, keys: &[&str]) -> Value {
    match container {
        None => empty(),
        Some(Value::Object(values)) => Value::Object(
            keys.iter()
                .filter_map(|key| {
                    values
                        .get(*key)
                        .map(|value| ((*key).to_owned(), value.clone()))
                })
                .collect(),
        ),
        Some(value) => value.clone(),
    }
}

/// Interface fields and unknown siblings, excluding independently decoded areas.
fn ui_values(container: Option<&Value>) -> Value {
    let Some(Value::Object(values)) = container else {
        return container.cloned().unwrap_or_else(empty);
    };
    Value::Object(
        values
            .iter()
            .filter(|(key, _)| {
                !matches!(key.as_str(), "theme" | "remote" | "onboarding")
                    && !EXPERIMENTAL_KEYS.contains(&key.as_str())
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    )
}

/// Join fields split between namespaces without hiding a malformed source.
fn combined(first: Value, second: Value) -> Value {
    match (first, second) {
        (Value::Object(mut first), Value::Object(second)) => {
            first.extend(second);
            Value::Object(first)
        }
        (Value::Object(_), invalid) | (invalid, _) => invalid,
    }
}

/// Decode once and publish ignored paths only after the complete area succeeds.
fn decode_area<T: DeserializeOwned>(
    value: &Value,
    root: &str,
    area: &str,
    loaded: &mut LoadedConfig,
) -> Option<T> {
    let ignored = RefCell::new(Vec::new());
    match T::deserialize(Decoder {
        value,
        path: root
            .split('.')
            .map(|key| Segment::Key(key.to_owned()))
            .collect(),
        ignored: &ignored,
    }) {
        Ok(config) => {
            let mut paths = ignored.into_inner();
            paths.sort();
            paths.dedup();
            loaded
                .diagnostics
                .extend(paths.iter().map(|path| unknown_key(&format_path(path))));
            Some(config)
        }
        Err(error) => {
            reject_area(area, &error.to_string(), loaded);
            None
        }
    }
}

/// Record one typed failure without publishing its ignored-key paths.
fn reject_area(area: &str, error: &str, loaded: &mut LoadedConfig) {
    loaded.invalid_sections.push(area.to_owned());
    loaded.diagnostics.push(format!(
        "invalid {area} config: {error}; keeping current {area} settings"
    ));
}

/// Warn about unused members of owned namespaces, not unrelated global settings.
fn unknown_namespace_keys(
    value: Option<&Value>,
    root: &str,
    known: &[&str],
    diagnostics: &mut Vec<String>,
) {
    let Some(Value::Object(values)) = value else {
        return;
    };
    let mut paths: Vec<_> = values
        .keys()
        .filter(|key| !known.contains(&key.as_str()))
        .collect();
    paths.sort();
    paths.dedup();
    diagnostics.extend(
        paths
            .iter()
            .map(|key| unknown_key(&format!("{root}.{}", path_key(key)))),
    );
}

/// Human-readable ignored-key diagnostic shared by namespace and area decoding.
fn unknown_key(path: &str) -> String {
    format!("unknown config key {path}; ignoring key")
}
