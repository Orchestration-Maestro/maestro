//! Native startup conversions, ordered notices and host-owned warning presentation.

#[cfg(test)]
mod tests;

use maestro_json::{canonical_entries, pretty_object};
use maestro_path::join;
use maestro_session::session_manager::get_default_session_dir;
use serde_json::{Map, Value};
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::{symlink_dir, symlink_file};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
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

/// Admitted session entry and its referent identity before relocation.
struct PreparedSession {
    /// Authored source entry.
    path: PathBuf,
    /// Header-selected workspace.
    cwd: String,
    /// Resolved final referent for symbolic links.
    referent: Option<PathBuf>,
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
    let prepared: Vec<_> = entries
        .into_iter()
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".jsonl"))
        .filter_map(|entry| prepare_session(&entry.path()).ok().flatten())
        .collect();
    let moves = prepared
        .into_iter()
        .filter_map(|session| {
            let directory = get_default_session_dir(&session.cwd, &agent_dir).ok()?;
            let target = Path::new(&directory).join(session.path.file_name()?);
            (!target.exists()).then_some(PreparedMove {
                path: session.path,
                target,
                referent: session.referent,
            })
        })
        .collect();
    relocate(moves);
    Ok(())
}

/// Capture header admission and link identity before any source can move.
fn prepare_session(path: &Path) -> io::Result<Option<PreparedSession>> {
    let path = canonical_entry_path(path)?;
    let content = fs::read(&path)?;
    let text = String::from_utf8_lossy(&content);
    let line = text.split('\n').next().unwrap_or_default();
    let header: Value = serde_json::from_str(line).map_err(io::Error::other)?;
    if header.get("type").and_then(Value::as_str) != Some("session") {
        return Ok(None);
    }
    let Some(cwd) = header
        .get("cwd")
        .and_then(Value::as_str)
        .filter(|cwd| !cwd.is_empty())
    else {
        return Ok(None);
    };
    let referent = fs::read_link(&path)
        .ok()
        .map(|_| fs::canonicalize(&path))
        .transpose()?;
    Ok(Some(PreparedSession {
        path,
        cwd: cwd.into(),
        referent,
    }))
}

/// Normalize a directory identity without following the final entry's link.
fn canonical_entry_path(path: &Path) -> io::Result<PathBuf> {
    Ok(fs::canonicalize(path.parent().unwrap_or(Path::new(".")))?
        .join(path.file_name().unwrap_or_default()))
}

/// Remove an entry without following directory links on Windows.
fn remove_link_entry(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTypeExt;
        if fs::symlink_metadata(path)?.file_type().is_symlink_dir() {
            return fs::remove_dir(path);
        }
    }
    fs::remove_file(path)
}

/// Whether renaming the stored target preserves the captured referent.
#[cfg(any(windows, test))]
fn link_rename_preserves_referent(link: &Path, target: &Path, referent: &Path) -> bool {
    link.is_absolute()
        || target
            .parent()
            .is_some_and(|parent| parent.join(link) == referent)
}

/// Re-anchor a link, retaining its source if privilege-free rename changes identity.
fn publish_link(path: &Path, target: &Path, referent: &Path) -> io::Result<()> {
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(".migrating");
    let temporary = PathBuf::from(temporary);
    #[cfg(unix)]
    let created = symlink(referent, &temporary);
    #[cfg(windows)]
    let created = if referent.is_dir() {
        symlink_dir(referent, &temporary)
    } else {
        symlink_file(referent, &temporary)
    };
    // Windows reports missing symbolic-link privilege as ERROR_PRIVILEGE_NOT_HELD.
    #[cfg(windows)]
    if created
        .as_ref()
        .is_err_and(|error| error.raw_os_error() == Some(1314))
    {
        if link_rename_preserves_referent(&fs::read_link(path)?, target, referent) {
            return fs::rename(path, target);
        }
        return created;
    }
    created?;
    let result = fs::rename(&temporary, target);
    if result.is_err() {
        let _ = remove_link_entry(&temporary);
    }
    result?;
    if path != target {
        remove_link_entry(path)?;
    }
    Ok(())
}

/// Decoration selected for one logical console line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationStyle {
    /// Successful conversion.
    Success,
    /// Warning or conversion guidance.
    Warning,
    /// Subdued input prompt.
    Dim,
    /// Undecorated output.
    Plain,
}

/// Caller-owned line sink; the host appends one LF to each supplied line.
pub type MigrationOutput<'a> = dyn FnMut(MigrationStyle, &str) -> io::Result<()> + 'a;

/// Converted provider names and notices collected during startup.
#[derive(Debug)]
pub struct MigrationResult {
    /// Provider names in conversion order.
    pub migrated_auth_providers: Vec<String>,
    /// Ordered notices for later presentation.
    pub deprecation_warnings: Vec<String>,
}

/// Perform native startup conversions without presenting the collected notices.
///
/// # Errors
/// Propagates configuration-root, credential publication, binary-directory creation
/// and caller output errors. Individual file conversion failures are tolerated.
pub fn run_migrations(cwd: &str, output: &mut MigrationOutput<'_>) -> io::Result<MigrationResult> {
    let migrated_auth_providers = migrate_auth_to_auth_json()?;
    migrate_sessions_from_agent_root()?;
    migrate_tools_to_bin(output)?;
    migrate_keybindings_config_file()?;
    let agent = super::config::get_agent_dir()?;
    let project = join(&[cwd, super::config::CONFIG_DIR_NAME]);
    migrate_commands_to_prompts(&agent, "Global", output)?;
    migrate_commands_to_prompts(&project, "Project", output)?;
    let deprecation_warnings = [(agent, "Global"), (project, "Project")]
        .into_iter()
        .flat_map(|(root, label)| deprecated_extension_dirs(&root, label))
        .collect();
    Ok(MigrationResult {
        migrated_auth_providers,
        deprecation_warnings,
    })
}

/// Rewrite admitted legacy action names through the authored file, preserving links.
fn migrate_keybindings_config_file() -> io::Result<()> {
    let path = join(&[&super::config::get_agent_dir()?, "keybindings.json"]);
    if let Some(Value::Object(raw)) = read_document(&path) {
        let (config, migrated) =
            crate::presentation_data::keybindings::migrate_keybindings_config(&raw);
        if migrated && let Ok(text) = pretty_object(&config) {
            let _ = fs::write(path, format!("{text}\n"));
        }
    }
    Ok(())
}

/// An admitted relocation with its final link identity captured before any move.
struct PreparedMove {
    /// Canonical parent plus original entry name, without following the entry.
    path: PathBuf,
    /// Caller-selected destination.
    target: PathBuf,
    /// Canonical final target for a live symbolic link.
    referent: Option<PathBuf>,
}

/// Resolve a captured target through successfully moved entries or their ancestors.
fn relocated_referent(referent: &Path, moved: &BTreeMap<PathBuf, PathBuf>) -> PathBuf {
    for ancestor in referent.ancestors() {
        if let Some(target) = moved.get(ancestor)
            && let Ok(suffix) = referent.strip_prefix(ancestor)
        {
            return if suffix.as_os_str().is_empty() {
                target.clone()
            } else {
                target.join(suffix)
            };
        }
    }
    referent.to_path_buf()
}

/// Execute admitted moves once, publishing links against captured final identities.
fn relocate(prepared: Vec<PreparedMove>) -> usize {
    let mut moved = BTreeMap::new();
    let mut links = Vec::new();
    let mut count = 0;
    for entry in prepared {
        if entry.referent.is_some() {
            links.push(entry);
        } else if let Ok(identity) = fs::canonicalize(&entry.path)
            && fs::rename(&entry.path, &entry.target).is_ok()
        {
            count += 1;
            if let Ok(target) = fs::canonicalize(&entry.target) {
                moved.insert(identity, target);
            }
        }
    }
    links.retain_mut(|entry| {
        if fs::read_link(&entry.path).is_ok_and(|link| link.is_absolute()) {
            if fs::rename(&entry.path, &entry.target).is_err() {
                return false;
            }
            count += 1;
            entry.path.clone_from(&entry.target);
        }
        true
    });
    for entry in links
        .iter()
        .filter(|entry| entry.path != entry.target)
        .chain(links.iter().filter(|entry| entry.path == entry.target))
    {
        let Some(referent) = &entry.referent else {
            continue;
        };
        let referent = relocated_referent(referent, &moved);
        if entry.path == entry.target
            && fs::canonicalize(&entry.target).is_ok_and(|actual| actual == referent)
        {
            continue;
        }
        if publish_link(&entry.path, &entry.target, &referent).is_ok() && entry.path != entry.target
        {
            count += 1;
        }
    }
    count
}

/// Retire duplicate old binaries locally, admitting collision-free relocations.
fn migrate_tools_to_bin(output: &mut MigrationOutput<'_>) -> io::Result<()> {
    let agent = super::config::get_agent_dir()?;
    let tools = PathBuf::from(super::config::paths::get_tools_dir(&agent));
    let bin = PathBuf::from(super::config::paths::get_bin_dir(&agent));
    let prepared: Vec<_> = ["fd", "rg", "fd.exe", "rg.exe"]
        .into_iter()
        .filter_map(|name| {
            let path = canonical_entry_path(&tools.join(name)).ok()?;
            let identity = fs::canonicalize(&path).ok()?;
            let referent = fs::read_link(&path).ok().map(|_| identity);
            Some(PreparedMove {
                path,
                target: bin.join(name),
                referent,
            })
        })
        .collect();
    let mut moves = Vec::new();
    for entry in prepared {
        if !bin.exists() {
            fs::create_dir_all(&bin)?;
        }
        if entry.target.exists() {
            if canonical_entry_path(&entry.target).is_ok_and(|target| target != entry.path) {
                let _ = remove_link_entry(&entry.path);
            }
        } else {
            moves.push(entry);
        }
    }
    if relocate(moves) > 0 {
        output(
            MigrationStyle::Success,
            "Migrated managed binaries tools/ → bin/",
        )?;
    }
    Ok(())
}

/// Collect literal deprecated-directory notices in hooks/tools order.
fn deprecated_extension_dirs(root: &str, label: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    if Path::new(&join(&[root, "hooks"])).exists() {
        warnings.push(format!(
            "{label} hooks/ directory found. Hooks have been renamed to extensions."
        ));
    }
    if let Ok(entries) = fs::read_dir(join(&[root, "tools"]))
        && entries.filter_map(Result::ok).any(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            !name.starts_with('.')
                && !["fd", "rg", "fd.exe", "rg.exe"]
                    .iter()
                    .any(|binary| name.eq_ignore_ascii_case(binary))
        })
    {
        warnings.push(format!("{label} tools/ directory contains custom tools. Custom tools have been merged into extensions."));
    }
    warnings
}

/// Rename the authored command entry without replacing a readable destination.
fn migrate_commands_to_prompts(
    root: &str,
    label: &str,
    output: &mut MigrationOutput<'_>,
) -> io::Result<()> {
    let source = join(&[root, "commands"]);
    let target = join(&[root, "prompts"]);
    if Path::new(&source).exists() && !Path::new(&target).exists() {
        match fs::rename(source, target) {
            Ok(()) => output(
                MigrationStyle::Success,
                &format!("Migrated {label} commands/ → prompts/"),
            )?,
            Err(error) => output(
                MigrationStyle::Warning,
                &format!("Warning: Could not migrate {label} commands/ to prompts/: {error}"),
            )?,
        }
    }
    Ok(())
}

/// Present ordered warnings, then await one host-owned input completion.
/// Empty warnings neither write output nor poll the supplied future.
///
/// # Errors
/// Returns the original output or host error; only a successful wait writes the final blank line.
pub async fn show_deprecation_warnings<F>(
    warnings: &[String],
    output: &mut MigrationOutput<'_>,
    wait_for_input: F,
) -> io::Result<()>
where
    F: std::future::Future<Output = io::Result<()>>,
{
    if warnings.is_empty() {
        return Ok(());
    }
    for warning in warnings {
        output(MigrationStyle::Warning, &format!("Warning: {warning}"))?;
    }
    for line in [
        "\nMove your extensions to the extensions/ directory.",
        "Migration guide: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/application/startup.md#native-legacy-conversions",
        "Documentation: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/guest.md",
    ] {
        output(MigrationStyle::Warning, line)?;
    }
    output(MigrationStyle::Dim, "\nPress any key to continue...")?;
    wait_for_input.await?;
    output(MigrationStyle::Plain, "")
}
