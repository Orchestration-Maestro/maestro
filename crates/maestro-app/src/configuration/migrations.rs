//! Native legacy credential conversion and root session relocation.

use maestro_json::{canonical_entries, pretty_object};
use maestro_path::join;
use maestro_session::session_manager::get_default_session_dir;
use serde_json::{Map, Value};
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

/// Selected credentials and deferred source retirement.
struct PreparedAuth {
    /// Plain provider keys and open persisted records.
    migrated: Map<String, Value>,
    /// Provider names in source conversion order.
    providers: Vec<String>,
    /// Whether the OAuth input was admitted for retirement.
    oauth_read: bool,
    /// Remaining settings when a key container was admitted.
    settings: Option<Map<String, Value>>,
}

/// Read a legacy JSON value with native decoding boundaries.
fn read_document(path: &str) -> Option<Value> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_str(&String::from_utf8_lossy(&bytes)).ok()
}

/// Enumerate canonical object keys or indexed array/string members.
fn legacy_entries(value: Value) -> Vec<(String, Value)> {
    match value {
        Value::Object(fields) => canonical_entries(fields),
        Value::Array(values) => values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
        Value::String(text) => text
            .chars()
            .enumerate()
            .map(|(i, c)| (i.to_string(), c.to_string().into()))
            .collect(),
        _ => Vec::new(),
    }
}

/// Select both independent legacy sources without retiring either.
fn prepare_auth(oauth_path: &str, settings_path: &str) -> PreparedAuth {
    let oauth = read_document(oauth_path).filter(|value| !value.is_null());
    let oauth_read = oauth.is_some();
    let mut migrated = Map::new();
    let mut providers = Vec::new();
    for (provider, value) in oauth.into_iter().flat_map(legacy_entries) {
        let mut record = Map::from_iter([("type".into(), "oauth".into())]);
        record.extend(legacy_entries(value));
        migrated.insert(provider.clone(), record.into());
        providers.push(provider);
    }
    let mut settings = read_document(settings_path).and_then(|value| {
        if let Value::Object(fields) = value {
            Some(fields)
        } else {
            None
        }
    });
    let keys = settings.as_mut().and_then(|s| s.shift_remove("apiKeys"));
    if let Some(keys @ (Value::Object(_) | Value::Array(_))) = keys {
        for (provider, value) in legacy_entries(keys) {
            if !migrated.contains_key(&provider) && value.is_string() {
                migrated.insert(
                    provider.clone(),
                    serde_json::json!({"type":"api_key", "key": value}),
                );
                providers.push(provider);
            }
        }
    } else {
        settings = None;
    }
    PreparedAuth {
        migrated,
        providers,
        oauth_read,
        settings,
    }
}

/// Write through the authored path and clean a failed created target.
fn publish_auth(path: &str, bytes: &[u8]) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    let target = fs::canonicalize(path)?;
    let result = file.write_all(bytes);
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(target);
    }
    result
}

/// Convert legacy credentials beneath the call-time configuration root.
///
/// # Errors
/// Propagates configuration-root and credential publication errors.
pub fn migrate_auth_to_auth_json() -> io::Result<Vec<String>> {
    let agent_dir = super::config::get_agent_dir()?;
    let auth_path = join(&[&agent_dir, "auth.json"]);
    if Path::new(&auth_path).exists() {
        return Ok(Vec::new());
    }
    let oauth_path = join(&[&agent_dir, "oauth.json"]);
    let settings_path = join(&[&agent_dir, "settings.json"]);
    let prepared = prepare_auth(&oauth_path, &settings_path);
    if !prepared.migrated.is_empty() {
        fs::create_dir_all(maestro_path::dirname(&auth_path))?;
        let text = pretty_object(&prepared.migrated).map_err(io::Error::other)?;
        publish_auth(&auth_path, text.as_bytes())?;
    }
    if prepared.oauth_read {
        let _ = fs::rename(&oauth_path, format!("{oauth_path}.migrated"));
    }
    if let Some(settings) = prepared.settings
        && let Ok(text) = pretty_object(&settings)
    {
        let _ = fs::write(&settings_path, text);
    }
    Ok(prepared.providers)
}

/// Relocate admitted direct session files beneath the configuration root.
///
/// # Errors
/// Propagates configuration-root errors; scan and individual file failures are tolerated.
pub fn migrate_sessions_from_agent_root() -> io::Result<()> {
    let agent_dir = super::config::get_agent_dir()?;
    let Ok(entries) = fs::read_dir(&agent_dir).and_then(Iterator::collect::<io::Result<Vec<_>>>)
    else {
        return Ok(());
    };
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".jsonl") {
            continue;
        }
        let _ = move_session(&agent_dir, &name);
    }
    Ok(())
}

/// Admit the first header and rename the original entry without rewriting its body.
fn move_session(agent_dir: &str, name: &str) -> io::Result<()> {
    let path = join(&[agent_dir, name]);
    let content = fs::read(&path)?;
    let text = String::from_utf8_lossy(&content);
    let line = text.split('\n').next().unwrap_or_default();
    let header: Value = serde_json::from_str(line).map_err(io::Error::other)?;
    if header.get("type").and_then(Value::as_str) != Some("session") {
        return Ok(());
    }
    let Some(cwd) = header
        .get("cwd")
        .and_then(Value::as_str)
        .filter(|cwd| !cwd.is_empty())
    else {
        return Ok(());
    };
    let directory = get_default_session_dir(cwd, agent_dir)?;
    let target = join(&[&directory, name]);
    if !Path::new(&target).exists() {
        fs::rename(path, target)?;
    }
    Ok(())
}
