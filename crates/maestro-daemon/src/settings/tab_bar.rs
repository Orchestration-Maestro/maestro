//! Right-side tab-bar entry descriptions and validation.

use serde::{Deserialize, Serialize};

/// An entry rendered at the right edge of the tab bar.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum TabBarRightEntryConfig {
    /// Current zoom state.
    Zoom {},
    /// Host name.
    Hostname {},
    /// Formatted date and time.
    Datetime {
        /// Authored strftime format.
        #[serde(default = "default_datetime_format")]
        format: String,
    },
    /// Literal text.
    Text {
        /// Text to display.
        text: String,
    },
    /// Output of a periodically executed command.
    Command {
        /// Command text.
        command: String,
        /// Refresh interval in seconds.
        #[serde(
            default = "default_command_interval_seconds",
            rename = "intervalSeconds"
        )]
        interval_seconds: u64,
        /// Execution deadline in seconds.
        #[serde(default = "default_command_timeout_seconds", rename = "timeoutSeconds")]
        timeout_seconds: u64,
    },
}

/// Default clock format.
fn default_datetime_format() -> String {
    "%H:%M".into()
}

/// Default command refresh interval.
const fn default_command_interval_seconds() -> u64 {
    5
}

/// Default command execution deadline.
const fn default_command_timeout_seconds() -> u64 {
    2
}

/// Parse and qualify a datetime format against the renderer's date/time value.
///
/// # Errors
/// Returns an explanation for empty, invalid or unsupported formats.
pub fn parse_tab_bar_datetime_format(
    value: &str,
) -> Result<time::format_description::OwnedFormatItem, String> {
    if value.is_empty() {
        return Err("datetime format is empty".into());
    }
    let format = time::format_description::parse_strftime_owned(value)
        .map_err(|error| format!("invalid datetime format: {error}"))?;
    time::PrimitiveDateTime::MIN
        .format(&format)
        .map_err(|error| format!("unsupported datetime format: {error}"))?;
    Ok(format)
}

/// Diagnose the first sixteen entries, retaining their authored order.
#[must_use]
pub fn tab_bar_right_diagnostics(entries: &[TabBarRightEntryConfig]) -> Vec<String> {
    let mut diagnostics = Vec::new();
    if entries.len() > 16 {
        diagnostics
            .push("multiplexer.tabBarRight may contain at most 16 entries; ignoring extras".into());
    }
    for (index, entry) in entries.iter().take(16).enumerate() {
        let field = format!("multiplexer.tabBarRight[{index}]");
        match entry {
            TabBarRightEntryConfig::Datetime { format } => {
                if format.is_empty() {
                    diagnostics.push(format!("{field} datetime format is empty; hiding entry"));
                } else if let Err(error) = parse_tab_bar_datetime_format(format) {
                    diagnostics.push(format!("{field} has {error}; hiding entry"));
                }
            }
            TabBarRightEntryConfig::Command {
                command,
                interval_seconds,
                timeout_seconds,
            } => {
                if command.trim().is_empty() {
                    diagnostics.push(format!("{field} command is empty; hiding entry"));
                }
                for (name, value, maximum) in [
                    ("intervalSeconds", *interval_seconds, 31_536_000),
                    ("timeoutSeconds", *timeout_seconds, 3600),
                ] {
                    diagnostics.extend(schedule_diagnostic(&field, name, value, maximum));
                }
            }
            TabBarRightEntryConfig::Zoom {}
            | TabBarRightEntryConfig::Hostname {}
            | TabBarRightEntryConfig::Text { .. } => {}
        }
    }
    diagnostics
}

/// Explain a zero or excessive command schedule value.
fn schedule_diagnostic(field: &str, name: &str, value: u64, maximum: u64) -> Option<String> {
    if value == 0 {
        Some(format!("{field} {name} must be at least 1; hiding entry"))
    } else if value > maximum {
        Some(format!(
            "{field} {name} may be at most {maximum}; hiding entry"
        ))
    } else {
        None
    }
}
