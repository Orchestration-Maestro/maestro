//! Mode selection, diagnostics and directory precedence before a frontend starts.

use crate::input::args::{Args, Diagnostic, DiagnosticType, Mode};
use maestro_app::configuration::config::paths::expand_tilde_path;
use maestro_app::{
    AgentSessionRuntimeDiagnostic, AgentSessionRuntimeDiagnosticType, SettingsManager,
};
use maestro_theme::Theme;
use std::io::{self, Write};

/// The status startup ends with when a reported problem is fatal.
const FAILURE: i32 = 1;

/// How startup presents its results.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMode {
    /// The interactive terminal frontend.
    Interactive,
    /// The prompts run in order, then the final answer is written as text.
    Print,
    /// The prompts run in order and every session event is streamed as a JSON line after
    /// the session header; no final answer is written.
    Json,
    /// JSONL requests and responses.
    Rpc,
}

impl AppMode {
    /// Whether this mode is the interactive frontend.
    #[must_use]
    pub fn is_interactive(self) -> bool {
        self == Self::Interactive
    }
}

/// Select the presentation mode from the parsed mode, the print flag and the stdin kind.
///
/// An explicit remote or record mode wins outright, a print request or a redirected
/// standard input selects print output, and every other run is interactive.
#[must_use]
pub fn resolve_app_mode(parsed: &Args, stdin_is_tty: bool) -> AppMode {
    match parsed.mode {
        Some(Mode::Rpc) => AppMode::Rpc,
        Some(Mode::Json) => AppMode::Json,
        Some(Mode::Text) | None if parsed.print.unwrap_or(false) || !stdin_is_tty => AppMode::Print,
        Some(Mode::Text) | None => AppMode::Interactive,
    }
}

/// Whether a run in `app_mode` needs standard output reserved for its records.
///
/// The answer only follows from the mode; the caller does the reserving.
#[must_use]
pub fn takes_over_stdout(app_mode: AppMode) -> bool {
    !app_mode.is_interactive()
}

/// The form a print run writes: JSON events for the JSON mode, the final answer text
/// otherwise.
#[must_use]
pub fn to_print_output_mode(app_mode: AppMode) -> Mode {
    if app_mode == AppMode::Json {
        Mode::Json
    } else {
        Mode::Text
    }
}

/// Collect the settings failures of one reporting context as warnings.
#[must_use]
pub fn collect_settings_diagnostics(
    settings: &mut SettingsManager,
    context: &str,
) -> Vec<AgentSessionRuntimeDiagnostic> {
    settings
        .drain_errors()
        .into_iter()
        .map(|failure| AgentSessionRuntimeDiagnostic {
            r#type: AgentSessionRuntimeDiagnosticType::Warning,
            message: format!(
                "({context}, {} settings) {}",
                failure.scope.label(),
                failure.error
            ),
        })
        .collect()
}

/// The rendered line of one message with its severity prefix and style.
fn render_message(
    severity: AgentSessionRuntimeDiagnosticType,
    message: &str,
    styles_enabled: bool,
) -> String {
    match severity {
        AgentSessionRuntimeDiagnosticType::Error => {
            Theme::red(&format!("Error: {message}"), styles_enabled)
        }
        AgentSessionRuntimeDiagnosticType::Warning => {
            Theme::yellow(&format!("Warning: {message}"), styles_enabled)
        }
        AgentSessionRuntimeDiagnosticType::Info => Theme::dim(message, styles_enabled),
    }
}

/// Write every diagnostic to `stderr` with its severity prefix and style.
///
/// # Errors
/// Returns the write failure of `stderr`.
pub fn report_diagnostics(
    diagnostics: &[AgentSessionRuntimeDiagnostic],
    stderr: &mut dyn Write,
    styles_enabled: bool,
) -> io::Result<()> {
    for diagnostic in diagnostics {
        let line = render_message(diagnostic.r#type, &diagnostic.message, styles_enabled);
        writeln!(stderr, "{line}")?;
    }
    Ok(())
}

/// Write the parser diagnostics, returning the status that ends startup when one is an error.
///
/// # Errors
/// Returns the write failure of `stderr`.
pub fn report_parser_diagnostics(
    diagnostics: &[Diagnostic],
    stderr: &mut dyn Write,
    styles_enabled: bool,
) -> io::Result<Option<i32>> {
    let failed = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.r#type == DiagnosticType::Error);
    for diagnostic in diagnostics {
        let severity = match diagnostic.r#type {
            DiagnosticType::Warning => AgentSessionRuntimeDiagnosticType::Warning,
            DiagnosticType::Error => AgentSessionRuntimeDiagnosticType::Error,
        };
        let line = render_message(severity, &diagnostic.message, styles_enabled);
        writeln!(stderr, "{line}")?;
    }
    Ok(failed.then_some(FAILURE))
}

/// The fork flags that cannot be combined, in their reported order.
fn fork_conflicts(parsed: &Args) -> Vec<&'static str> {
    [
        (parsed
            .session
            .as_deref()
            .is_some_and(|value| !value.is_empty()))
        .then_some("--session"),
        (parsed.r#continue == Some(true)).then_some("--continue"),
        (parsed.resume == Some(true)).then_some("--resume"),
        (parsed.no_session == Some(true)).then_some("--no-session"),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Write the flags that conflict with an explicit fork request.
///
/// Returns the status that ends startup when the flags conflict, and `None` when the
/// combination is accepted or no fork was requested.
///
/// # Errors
/// Returns the write failure of `stderr`.
pub fn validate_fork_flags(
    parsed: &Args,
    stderr: &mut dyn Write,
    styles_enabled: bool,
) -> io::Result<Option<i32>> {
    let forking = parsed
        .fork
        .as_deref()
        .is_some_and(|value| !value.is_empty());
    if !forking {
        return Ok(None);
    }
    let conflicts = fork_conflicts(parsed);
    if conflicts.is_empty() {
        return Ok(None);
    }
    let line = format!(
        "Error: --fork cannot be combined with {}",
        conflicts.join(", ")
    );
    writeln!(stderr, "{}", Theme::red(&line, styles_enabled))?;
    Ok(Some(FAILURE))
}

/// Resolve the session directory: the explicit argument, the environment, then the setting.
///
/// An authored argument wins even when empty, an empty environment value falls through,
/// and the environment is expanded through `home`.
///
/// # Errors
/// Returns the error of `home` when the environment value needs it.
pub fn resolve_session_dir(
    parsed: &Args,
    environment: Option<&str>,
    settings: &SettingsManager,
    home: impl FnOnce() -> io::Result<String>,
) -> io::Result<Option<String>> {
    if let Some(directory) = &parsed.session_dir {
        return Ok(Some(directory.clone()));
    }
    match environment.filter(|value| !value.is_empty()) {
        Some(directory) => expand_tilde_path(directory, home).map(Some),
        None => Ok(settings.get_session_dir()),
    }
}
