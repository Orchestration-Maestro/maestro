//! Settings fixtures for the daemon's tests, on every platform.

use maestro_settings::BaseSettingsLayer;
use serde_json::Value;

/// A synthetic accepted release with caller-selected locks.
///
/// # Errors
/// Returns why `value` is not a settings document.
pub fn layer(value: Value, locked: &[&str]) -> Result<BaseSettingsLayer, String> {
    Ok(BaseSettingsLayer {
        settings: serde_json::from_value(value).map_err(|error| error.to_string())?,
        locked: locked.iter().map(|path| (*path).to_owned()).collect(),
        source: "git:example/config#v1".into(),
        recorded_at: "2026-10-27T12:00:00Z".into(),
    })
}
