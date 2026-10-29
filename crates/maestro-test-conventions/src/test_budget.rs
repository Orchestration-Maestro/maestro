//! Offline validation of nextest's terminating test budgets.

use std::{collections::BTreeMap, path::Path};

use serde::Deserialize;
use toml::{Spanned, Value};

/// Runner profiles, ignoring unrelated nextest settings.
#[derive(Deserialize)]
struct Config {
    /// Named runner profiles.
    profile: BTreeMap<String, Profile>,
}

/// Time limits at profile scope and in selected overrides.
#[derive(Deserialize)]
struct Profile {
    /// Profile-wide limit, inherited when absent outside the default profile.
    #[serde(rename = "slow-timeout")]
    slow_timeout: Option<Spanned<Value>>,
    /// Selected per-test limits.
    #[serde(default)]
    overrides: Vec<Override>,
}

/// One test selector's optional time limit.
#[derive(Deserialize)]
struct Override {
    /// Explicit exception or tighter test budget.
    #[serde(rename = "slow-timeout")]
    slow_timeout: Option<Spanned<Value>>,
}

/// Check configuration when present; synthetic workspaces need no runner config.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    let path = root.join(".config/nextest.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    validate(&text).map_err(|error| format!("{}: {error}", path.display()))
}

/// Validate profile defaults before any per-test exceptions.
fn validate(text: &str) -> Result<(), String> {
    let config: Config = toml::from_str(text).map_err(|error| error.to_string())?;
    if config
        .profile
        .get("default")
        .and_then(|p| p.slow_timeout.as_ref())
        .is_none()
    {
        return Err("default profile must set a terminating 30-second budget".into());
    }
    for (name, profile) in config.profile {
        if let Some(timeout) = profile.slow_timeout {
            let limit = seconds(timeout.get_ref())?;
            if limit > 30 {
                return Err(format!(
                    "profile {name} exceeds the 30-second budget; use a per-test exception"
                ));
            }
        }
        for timeout in profile
            .overrides
            .into_iter()
            .filter_map(|entry| entry.slow_timeout)
        {
            let limit = seconds(timeout.get_ref())?;
            if limit > 30 && !documented(text, timeout.span().start) {
                let line = text[..timeout.span().start].lines().count();
                return Err(format!(
                    "line {line}: slow-timeout exception needs a measured cause and issue (#NNNN) in a comment immediately above slow-timeout"
                ));
            }
        }
    }
    Ok(())
}

/// Read a finite terminating limit in whole seconds, including its interval count.
fn seconds(timeout: &Value) -> Result<u64, String> {
    let invalid = || {
        "30-second budget requires slow-timeout = { period = 'Ns', terminate-after = N } with positive whole seconds and a positive termination count".to_owned()
    };
    let period = timeout
        .get("period")
        .and_then(Value::as_str)
        .and_then(|period| period.strip_suffix('s'))
        .and_then(|period| period.parse::<u64>().ok())
        .filter(|period| *period > 0)
        .ok_or_else(invalid)?;
    let count = timeout
        .get("terminate-after")
        .and_then(Value::as_integer)
        .and_then(|count| u64::try_from(count).ok())
        .filter(|count| *count > 0)
        .ok_or_else(invalid)?;
    period.checked_mul(count).ok_or_else(invalid)
}

/// Require measured seconds, a nonempty cause and a positive issue in adjacent comments.
fn documented(text: &str, start: usize) -> bool {
    let before_line = text[..start].rfind('\n').map_or("", |end| &text[..end]);
    let comment = before_line
        .lines()
        .rev()
        .map(str::trim)
        .take_while(|line| line.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ");
    let measured = comment
        .split("Measured: ")
        .nth(1)
        .and_then(|value| value.split(';').next())
        .and_then(|value| value.strip_suffix('s'))
        .and_then(|value| value.parse::<f64>().ok())
        .is_some_and(|value| value.is_finite() && value > 0.0);
    let cause = comment
        .split("cause: ")
        .nth(1)
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| !value.trim().is_empty());
    let issue = comment.split_whitespace().any(|word| {
        word.strip_prefix('#')
            .and_then(|number| number.parse::<u64>().ok())
            .is_some_and(|number| number > 0)
    });
    measured && cause && issue
}
