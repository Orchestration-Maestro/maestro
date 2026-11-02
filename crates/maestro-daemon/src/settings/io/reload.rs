//! Settings-owner load barriers, error attribution and retained-area application.

use std::path::Path;

use maestro_settings::{SettingsError, SettingsErrorKind, SettingsManager, SettingsScope};

use super::{
    super::model::{Config, ConfigReloadReport, ConfigReloadStatus, LoadedConfig},
    AcceptedAreas, project_global_settings,
};

/// Load startup preferences from the supplied, newly initialized manager.
///
/// Global read/parse failures use the base with an empty user scope and emit
/// warnings. Project errors are returned separately for their existing reporting path.
pub fn load_config(
    manager: &mut SettingsManager,
    agent_dir: &Path,
) -> (LoadedConfig, Vec<SettingsError>) {
    let earlier = manager.drain_errors();
    let mut settings = manager.effective_global_settings();
    let (global, mut other, mut diagnostics) = drain_load_errors(manager, earlier);
    if !global.is_empty() {
        diagnostics.extend(global.iter().map(|error| {
            if is_parse_error(error) {
                tracing::warn!(err = %error.error, "config parse error, using defaults");
            } else {
                tracing::warn!(err = %error.error, "config read error, using defaults");
            }
            document_diagnostic(error, "using defaults")
        }));
        settings = manager.global_base_settings();
    }
    let mut loaded = project_global_settings(&settings, agent_dir);
    AcceptedAreas::default().recover(&mut loaded, manager, agent_dir);
    if loaded
        .diagnostics
        .iter()
        .any(|message| message.starts_with("invalid "))
    {
        tracing::warn!(err = ?loaded.diagnostics, "config parse error, using defaults");
        for diagnostic in &mut loaded.diagnostics {
            if let Some((message, _)) = diagnostic.split_once("; keeping current ") {
                *diagnostic = format!("{message}; using defaults");
            }
        }
    }
    loaded.diagnostics.append(&mut diagnostics);
    collect_recovery_errors(manager, &mut loaded, &mut other);
    (loaded, other)
}

/// Reload the supplied manager and replace only successfully decoded areas.
///
/// Earlier queued errors and new project errors are returned separately; only a
/// new global read failure rejects this reload. No storage or manager is created.
pub async fn reload_config(
    manager: &mut SettingsManager,
    current: &mut LoadedConfig,
    agent_dir: &Path,
) -> (ConfigReloadReport, Vec<SettingsError>) {
    let (loaded, other) = load_live_config(manager, &current.accepted_areas, agent_dir).await;
    let report = match loaded {
        Ok(loaded) => {
            current.accepted_areas.clone_from(&loaded.accepted_areas);
            apply_loaded(&mut current.config, loaded)
        }
        Err(diagnostics) => ConfigReloadReport {
            status: ConfigReloadStatus::Failed,
            diagnostics,
        },
    };
    (report, other)
}

/// Flushes pending writes and loads independently decoded areas from one global document.
/// Global document errors reject the load; project errors retain their separate reporting path.
pub async fn load_live_config(
    manager: &mut SettingsManager,
    accepted: &AcceptedAreas,
    agent_dir: &Path,
) -> (Result<LoadedConfig, Vec<String>>, Vec<SettingsError>) {
    manager.flush().await;
    let mut other = manager.drain_errors();
    manager.reload().await;
    let earlier = manager.drain_errors();
    let settings = manager.effective_global_settings();
    let (global, project, mut diagnostics) = drain_load_errors(manager, earlier);
    other.extend(project);
    if !global.is_empty() {
        return (
            Err(global
                .iter()
                .map(|error| document_diagnostic(error, "keeping current config"))
                .collect()),
            other,
        );
    }
    let mut loaded = project_global_settings(&settings, agent_dir);
    accepted.recover(&mut loaded, manager, agent_dir);
    loaded.diagnostics.append(&mut diagnostics);
    collect_recovery_errors(manager, &mut loaded, &mut other);
    (Ok(loaded), other)
}

/// Returns recovery resolutions once and preserves unrelated errors for their caller.
fn collect_recovery_errors(
    manager: &mut SettingsManager,
    loaded: &mut LoadedConfig,
    other: &mut Vec<SettingsError>,
) {
    let (failures, project, diagnostics) = drain_load_errors(manager, Vec::new());
    other.extend(failures);
    other.extend(project);
    loaded.diagnostics.extend(diagnostics);
    loaded.diagnostics.sort();
    loaded.diagnostics.dedup();
}

/// Keep fresh global resolutions, failures and unrelated project errors.
/// Earlier global resolutions may describe the project layer or an obsolete base.
fn drain_load_errors(
    manager: &mut SettingsManager,
    earlier: Vec<SettingsError>,
) -> (Vec<SettingsError>, Vec<SettingsError>, Vec<String>) {
    let (global, other): (Vec<_>, Vec<_>) = manager
        .drain_errors()
        .into_iter()
        .chain(earlier.into_iter().filter(|error| {
            error.scope != SettingsScope::Global
                || error.kind != SettingsErrorKind::StoredValueIgnored
        }))
        .partition(|error| error.scope == SettingsScope::Global);
    let (ignored, failures): (Vec<_>, Vec<_>) = global
        .into_iter()
        .partition(|error| error.kind == SettingsErrorKind::StoredValueIgnored);
    let mut diagnostics: Vec<_> = ignored
        .iter()
        .map(|error| error.error.to_string())
        .collect();
    diagnostics.sort();
    diagnostics.dedup();
    (failures, other, diagnostics)
}

/// Apply original semantic areas without coupling their namespace placement.
pub(crate) fn apply_loaded(current: &mut Config, loaded: LoadedConfig) -> ConfigReloadReport {
    macro_rules! apply {
        ($($field:ident),* $(,)?) => { $(
            if !loaded.invalid_sections.iter().any(|area| area == stringify!($field)) {
                current.$field = loaded.config.$field;
            }
        )* };
    }
    apply!(
        onboarding,
        theme,
        terminal,
        session,
        server,
        update,
        ui,
        worktrees,
        advanced,
        experimental,
        remote
    );
    ConfigReloadReport {
        status: if loaded.diagnostics.is_empty() {
            ConfigReloadStatus::Applied
        } else {
            ConfigReloadStatus::Partial
        },
        diagnostics: loaded.diagnostics,
    }
}

/// Identify parser failures by their concrete cause, not error-message spelling.
fn is_parse_error(error: &SettingsError) -> bool {
    error.error.is::<serde_json::Error>()
}

/// Explain document failures with their startup or live-reload impact.
fn document_diagnostic(error: &SettingsError, impact: &str) -> String {
    let kind = if is_parse_error(error) {
        "parse"
    } else {
        "read"
    };
    format!("config {kind} error: {}; {impact}", error.error)
}

/// Compact actionable banner; details remain in the accompanying diagnostics.
#[must_use]
pub fn config_diagnostic_summary(diagnostics: &[String]) -> Option<String> {
    if diagnostics.is_empty() {
        return None;
    }
    let read = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("config read error:"));
    let impact = if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("using defaults"))
    {
        if read {
            " unreadable; using defaults"
        } else {
            " invalid; using defaults"
        }
    } else if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("keeping current config"))
    {
        if read {
            " unreadable; keeping current config"
        } else {
            " invalid; keeping current config"
        }
    } else if diagnostics
        .iter()
        .all(|diagnostic| diagnostic.starts_with("unknown config key "))
    {
        " has unknown keys"
    } else {
        ""
    };
    Some(format!("settings.json{impact}; maestro settings check"))
}
