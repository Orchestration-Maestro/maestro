//! Global-only workspace projection through the existing settings owner.

mod ignored;
mod reload;
mod retention;

#[cfg(target_os = "linux")]
pub(crate) use reload::apply_loaded;
pub use reload::{config_diagnostic_summary, load_config, load_live_config, reload_config};

use std::{cell::RefCell, path::Path};

use maestro_settings::Settings;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::model::{Config, LoadedConfig};
use ignored::{Decoder, Segment, ValueRef, format_path, path_key};
pub use retention::AcceptedAreas;

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
        accepted_areas: AcceptedAreas::default(),
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
    macro_rules! area {
        ($field:ident, $root:literal, $value:expr) => {
            if let Some(value) = decode_area($value, $root, stringify!($field), loaded) {
                loaded.config.$field = value;
                loaded.accepted_areas.remember(stringify!($field), settings);
            }
        };
    }
    let empty = Value::Object(serde_json::Map::new());
    project_onboarding(settings, loaded);
    area!(theme, "multiplexer.theme", nested(mux, "theme", &empty));
    area!(remote, "multiplexer.remote", nested(mux, "remote", &empty));
    area!(ui, "multiplexer", ui_values(mux));
    area!(
        terminal,
        "daemon",
        selected(
            settings,
            "daemon",
            &["paneShell", "paneShellMode", "newPaneCwd"]
        )
    );
    area!(
        session,
        "daemon",
        selected(settings, "daemon", &["resumeAgentsOnRestore"])
    );
    area!(
        server,
        "daemon",
        selected(settings, "daemon", &["headlessCols", "headlessRows"])
    );
    area!(
        advanced,
        "daemon",
        selected(settings, "daemon", &["scrollbackLimitBytes"])
    );
    area!(
        worktrees,
        "daemon",
        selected(settings, "daemon", &["worktreesDirectory"])
    );
    area!(
        update,
        "update",
        combined(
            selected(settings, "update", &["channel", "versionCheck"]),
            selected(settings, "daemon", &["detectionCatalogCheck"])
        )
    );
    area!(
        experimental,
        "multiplexer",
        combined(
            selected(settings, "multiplexer", EXPERIMENTAL_KEYS),
            selected(settings, "daemon", &["paneHistory"])
        )
    );
}

/// Optional onboarding still requires its enclosing namespace to be an object.
fn project_onboarding(settings: &Settings, loaded: &mut LoadedConfig) {
    let mux = settings.0.get("multiplexer");
    if mux.is_some_and(|value| !value.is_object()) {
        reject_area("onboarding", "multiplexer must be an object", loaded);
    } else if let Some(value) = decode_area(
        nested(mux, "onboarding", &Value::Null),
        "multiplexer.onboarding",
        "onboarding",
        loaded,
    ) {
        loaded.config.onboarding = value;
        loaded.accepted_areas.remember("onboarding", settings);
    }
}

/// Select a nested record, preserving a malformed container for typed rejection.
fn nested<'a>(container: Option<&'a Value>, key: &str, fallback: &'a Value) -> ValueRef<'a> {
    ValueRef::Value(match container {
        None => fallback,
        Some(Value::Object(values)) => values.get(key).unwrap_or(fallback),
        Some(value) => value,
    })
}

/// Select flattened keys for one original semantic area.
fn selected<'a>(
    settings: &'a Settings,
    root: &'static str,
    keys: &'static [&'static str],
) -> ValueRef<'a> {
    match settings.0.get(root) {
        None => ValueRef::Fields(Vec::new()),
        Some(Value::Object(values)) => ValueRef::Fields(
            values
                .iter()
                .filter(|(key, _)| keys.contains(&key.as_str()))
                .map(|(key, value)| (key.as_str(), value))
                .collect(),
        ),
        Some(value) => ValueRef::Value(value),
    }
}

/// Interface fields and unknown siblings, excluding independently decoded areas.
fn ui_values(container: Option<&Value>) -> ValueRef<'_> {
    match container {
        None => ValueRef::Fields(Vec::new()),
        Some(Value::Object(values)) => ValueRef::Fields(
            values
                .iter()
                .filter(|(key, _)| {
                    !matches!(key.as_str(), "theme" | "remote" | "onboarding")
                        && !EXPERIMENTAL_KEYS.contains(&key.as_str())
                })
                .map(|(key, value)| (key.as_str(), value))
                .collect(),
        ),
        Some(value) => ValueRef::Value(value),
    }
}

/// Join fields split between namespaces without hiding a malformed source.
fn combined<'a>(first: ValueRef<'a>, second: ValueRef<'a>) -> ValueRef<'a> {
    match (first, second) {
        (ValueRef::Fields(mut first), ValueRef::Fields(second)) => {
            first.extend(second);
            first.sort_unstable_by_key(|(key, _)| *key);
            ValueRef::Fields(first)
        }
        (ValueRef::Fields(_), invalid) | (invalid, _) => invalid,
    }
}

/// Decode once and publish ignored paths only after the complete area succeeds.
fn decode_area<T: DeserializeOwned>(
    value: ValueRef<'_>,
    root: &'static str,
    area: &'static str,
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
