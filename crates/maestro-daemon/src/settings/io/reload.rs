//! Settings-owner load barriers, error attribution and retained-area application.

use std::path::Path;

use maestro_settings::{SettingsError, SettingsManager, SettingsScope};

use super::{
    super::model::{Config, ConfigReloadReport, ConfigReloadStatus, LoadedConfig},
    project_global_settings,
};

/// Load startup preferences from the supplied, newly initialized manager.
///
/// Global read/parse failures use defaults and emit startup warnings. Project
/// errors are returned separately for their existing reporting path.
pub fn load_config(
    manager: &mut SettingsManager,
    agent_dir: &Path,
) -> (LoadedConfig, Vec<SettingsError>) {
    let (global, other): (Vec<_>, Vec<_>) = manager
        .drain_errors()
        .into_iter()
        .partition(|error| error.scope == SettingsScope::Global);
    if !global.is_empty() {
        let diagnostics = global
            .iter()
            .map(|error| {
                if is_parse_error(error) {
                    tracing::warn!(err = %error.error, "config parse error, using defaults");
                } else {
                    tracing::warn!(err = %error.error, "config read error, using defaults");
                }
                document_diagnostic(error, "using defaults")
            })
            .collect();
        return (
            LoadedConfig {
                config: Config::default(),
                diagnostics,
                invalid_sections: Vec::new(),
            },
            other,
        );
    }
    let mut loaded = project_global_settings(&manager.get_global_settings(), agent_dir);
    if !loaded.invalid_sections.is_empty() {
        tracing::warn!(err = ?loaded.diagnostics, "config parse error, using defaults");
        for diagnostic in &mut loaded.diagnostics {
            if let Some((message, _)) = diagnostic.split_once("; keeping current ") {
                *diagnostic = format!("{message}; using defaults");
            }
        }
    }
    (loaded, other)
}

/// Reload the supplied manager and replace only successfully decoded areas.
///
/// Earlier queued errors and new project errors are returned separately; only a
/// new global read failure rejects this reload. No storage or manager is created.
pub async fn reload_config(
    manager: &mut SettingsManager,
    current: &mut Config,
    agent_dir: &Path,
) -> (ConfigReloadReport, Vec<SettingsError>) {
    manager.flush().await;
    let mut other = manager.drain_errors();
    manager.reload().await;
    let (global, project): (Vec<_>, Vec<_>) = manager
        .drain_errors()
        .into_iter()
        .partition(|error| error.scope == SettingsScope::Global);
    other.extend(project);
    if !global.is_empty() {
        return (
            ConfigReloadReport {
                status: ConfigReloadStatus::Failed,
                diagnostics: global
                    .iter()
                    .map(|error| document_diagnostic(error, "keeping current config"))
                    .collect(),
            },
            other,
        );
    }
    let loaded = project_global_settings(&manager.get_global_settings(), agent_dir);
    let report = apply_loaded(current, loaded);
    (report, other)
}

/// Apply original semantic areas without coupling their namespace placement.
fn apply_loaded(current: &mut Config, loaded: LoadedConfig) -> ConfigReloadReport {
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
