//! Downloaded detection catalog versions and publication.

use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, fmt};
/// A validated dotted numeric version, retaining its trimmed spelling.
#[derive(Debug, Clone)]
pub struct ManifestVersion(String);

impl ManifestVersion {
    /// Parses a dotted sequence of ASCII unsigned 64-bit segments.
    ///
    /// # Errors
    /// Rejects empty, nonnumeric, empty-segment or oversized input.
    pub fn parse(value: &str) -> Result<Self, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("version must not be empty".to_string());
        }
        for segment in trimmed.split('.') {
            if segment.is_empty() {
                return Err(format!("version {trimmed:?} contains an empty segment"));
            }
            if !segment.chars().all(|ch| ch.is_ascii_digit()) {
                return Err(format!("version {trimmed:?} must be dotted numeric"));
            }
            segment
                .parse::<u64>()
                .map_err(|_| format!("version {trimmed:?} contains an oversized segment"))?;
        }
        Ok(Self(trimmed.to_string()))
    }
}

impl fmt::Display for ManifestVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ManifestVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ManifestVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl Ord for ManifestVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut left = self.0.split('.');
        let mut right = other.0.split('.');
        loop {
            let pair = (left.next(), right.next());
            if pair == (None, None) {
                return Ordering::Equal;
            }
            let value = |part: Option<&str>| {
                part.and_then(|part| part.parse::<u64>().ok())
                    .unwrap_or_default()
            };
            let order = value(pair.0).cmp(&value(pair.1));
            if order != Ordering::Equal {
                return order;
            }
        }
    }
}

impl PartialOrd for ManifestVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ManifestVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for ManifestVersion {}

use crate::agents::{Agent, agent_label};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
/// Highest supported declared detection engine version.
pub const MANIFEST_ENGINE_VERSION: u32 = 3;
/// Last catalog check and per-agent publication outcomes.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ManifestUpdateStatus {
    /// Time of the most recent catalog check in Unix seconds.
    pub last_check_unix: Option<u64>,
    /// Most recent operation outcome, including failure details when applicable.
    pub last_result: Option<String>,
    /// Per-agent update outcomes indexed by canonical catalogue label.
    #[serde(default)]
    pub agents: BTreeMap<String, AgentRemoteStatus>,
}

impl ManifestUpdateStatus {
    /// Returns the retained update outcome for a catalogue entry.
    #[must_use]
    pub fn agent_status(&self, agent: Agent) -> Option<AgentRemoteStatus> {
        self.agents.get(agent_label(agent)).cloned()
    }
}
/// Download metadata and the most recent outcome for one agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRemoteStatus {
    /// Version of the currently cached valid download.
    pub cached_version: Option<String>,
    /// Version published by the most recent successful update.
    pub attempted_version: Option<String>,
    /// Time of the most recent agent check in Unix seconds.
    pub last_checked_unix: Option<u64>,
    /// Most recent operation outcome, including failure details when applicable.
    pub last_result: String,
    /// Failure detail from the most recent agent check.
    pub last_error: Option<String>,
}
/// Returns the status filename under the supplied catalog directory.
#[must_use]
pub fn status_path(root: &Path) -> PathBuf {
    root.join("status.toml")
}
/// Returns the canonical agent filename under the catalog remote directory.
#[must_use]
pub fn remote_manifest_path(root: &Path, agent: Agent) -> PathBuf {
    root.join("remote")
        .join(format!("{}.toml", agent_label(agent)))
}
/// Reads status from the supplied directory, defaulting on missing or invalid data.
#[must_use]
pub fn load_status(root: &Path) -> ManifestUpdateStatus {
    let path = status_path(root);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return ManifestUpdateStatus::default();
    };
    toml::from_str(&content).unwrap_or_else(|error| {
        tracing::warn!(path = %path.display(), "failed to parse agent detection manifest status: {error}");
        ManifestUpdateStatus::default()
    })
}
/// One successfully published agent document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestUpdateCommit {
    /// Catalogue identity of the published document.
    pub agent: Agent,
    /// Published version.
    pub version: ManifestVersion,
}
/// Publishes only a newer valid document, preserving equal-version byte identity.
///
/// # Errors
/// Returns validation, downgrade, changed-content or publication diagnostics.
pub fn process_agent_manifest(
    root: &Path,
    agent: Agent,
    content: &str,
) -> Result<Option<ManifestUpdateCommit>, String> {
    let parsed = crate::manifest::parse_remote_manifest_for_agent(agent, content)?;
    if let Some(current) = cached_remote_version(root, agent) {
        match parsed.version.cmp(&current) {
            Ordering::Less => {
                return Err(format!(
                    "remote version {} is older than cached {current}",
                    parsed.version
                ));
            }
            Ordering::Equal => {
                let committed =
                    std::fs::read_to_string(remote_manifest_path(root, agent)).unwrap_or_default();
                if committed != content {
                    return Err(format!(
                        "remote version {} changed content without a version bump",
                        parsed.version
                    ));
                }
                return Ok(None);
            }
            Ordering::Greater => {}
        }
    }

    atomic_write(&remote_manifest_path(root, agent), content.as_bytes())?;
    Ok(Some(ManifestUpdateCommit {
        agent,
        version: parsed.version,
    }))
}

/// Returns the version of a valid download at the supplied catalog root.
#[must_use]
pub fn cached_remote_version(root: &Path, agent: Agent) -> Option<ManifestVersion> {
    let content = std::fs::read_to_string(remote_manifest_path(root, agent)).ok()?;
    crate::manifest::parse_remote_manifest_for_agent(agent, &content)
        .ok()
        .map(|parsed| parsed.version)
}
/// Publishes bytes using the shared durable replacement owner.
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let committed = maestro_lock::replace(path, bytes, maestro_lock::Replace::default())
        .map_err(|error| error.to_string())?;
    if let Some(error) = committed.dir_sync {
        tracing::warn!(path = %path.display(), error = %error, "agent detection manifest committed but parent directory sync failed");
    }
    Ok(())
}
/// Transport inputs for a single bounded catalog or document fetch.
#[derive(Debug, Clone, Copy)]
pub struct FetchRequest<'a> {
    /// Complete URL selected from the supplied catalog location.
    pub url: &'a str,
    /// Largest response body admitted by the detection engine.
    pub max_bytes: usize,
    /// Transport retry policy.
    pub retries: u32,
    /// Transport connection timeout.
    pub connect_timeout: std::time::Duration,
    /// Total transport timeout.
    pub total_timeout: std::time::Duration,
}
/// Successfully completed catalog check and its per-agent outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestUpdateOutput {
    /// Documents committed during this check, in catalog order.
    pub updated: Vec<ManifestUpdateCommit>,
    /// Status published after the check.
    pub status: ManifestUpdateStatus,
}
/// Checks the supplied catalog, reloads committed rules, then calls the observer.
///
/// # Errors
/// Returns a bundled-document error if reloading the supplied cache fails.
pub fn auto_update(
    cache: &mut crate::manifest::ManifestCache,
    root: &Path,
    url: &str,
    fetch: &mut impl FnMut(FetchRequest<'_>) -> Result<Vec<u8>, String>,
    callback: impl FnOnce(&crate::manifest::ManifestCache, ManifestUpdateOutput),
) -> Result<(), String> {
    let output = match check_and_update(root, url, fetch) {
        Ok(output) => output,
        Err(error) => {
            tracing::warn!("agent detection manifest update failed: {error}");
            let mut status = load_status(root);
            status.last_check_unix = Some(now_unix());
            status.last_result = Some(format!("failed: {error}"));
            let _ = save_status(root, &status);
            ManifestUpdateOutput {
                updated: Vec::new(),
                status,
            }
        }
    };
    if !output.updated.is_empty() {
        cache.reload_manifests()?;
    }
    callback(cache, output);
    Ok(())
}
/// Processes every catalog entry independently and publishes the resulting status.
///
/// # Errors
/// Returns a catalog transport, UTF-8, schema or URL validation diagnostic.
pub fn check_and_update(
    root: &Path,
    url: &str,
    fetch: &mut impl FnMut(FetchRequest<'_>) -> Result<Vec<u8>, String>,
) -> Result<ManifestUpdateOutput, String> {
    let catalog = parse_catalog(&fetch_text(url, fetch)?)?;
    let base = base_url(url)?;
    let mut status = load_status(root);
    let check_time = now_unix();
    status.last_check_unix = Some(check_time);
    status.last_result = Some("checked".to_owned());
    let mut updated = Vec::new();
    for entry in catalog {
        let manifest_url = join_url(&base, &entry.path)?;
        let result = fetch_text(&manifest_url, fetch)
            .map_err(|error| format!("fetch failed: {error}"))
            .and_then(|content| process_agent_manifest(root, entry.agent, &content));
        let (commit, outcome) = agent_outcome(root, entry.agent, check_time, result);
        status
            .agents
            .insert(agent_label(entry.agent).to_owned(), outcome);
        updated.extend(commit);
    }
    if let Err(error) = save_status(root, &status) {
        tracing::warn!("failed to save agent detection manifest update status: {error}");
        status.last_result = Some(format!("failed_to_save_status: {error}"));
    }
    Ok(ManifestUpdateOutput { updated, status })
}
/// Converts a per-agent result without allowing its failure to cancel siblings.
fn agent_outcome(
    root: &Path,
    agent: Agent,
    check_time: u64,
    result: Result<Option<ManifestUpdateCommit>, String>,
) -> (Option<ManifestUpdateCommit>, AgentRemoteStatus) {
    let (commit, cached_version, attempted_version, last_result, last_error) = match result {
        Ok(Some(commit)) => {
            let version = commit.version.to_string();
            (
                Some(commit),
                Some(version.clone()),
                Some(version),
                "updated",
                None,
            )
        }
        Ok(None) => (
            None,
            cached_remote_version(root, agent).map(|v| v.to_string()),
            None,
            "current",
            None,
        ),
        Err(error) => {
            tracing::warn!(agent = agent_label(agent), error = %error, "agent detection manifest update failed for agent");
            (
                None,
                cached_remote_version(root, agent).map(|v| v.to_string()),
                None,
                "failed",
                Some(error),
            )
        }
    };
    (
        commit,
        AgentRemoteStatus {
            cached_version,
            attempted_version,
            last_checked_unix: Some(check_time),
            last_result: last_result.to_owned(),
            last_error,
        },
    )
}
/// Saves status through the same recoverable byte publication owner as documents.
fn save_status(root: &Path, status: &ManifestUpdateStatus) -> Result<(), String> {
    let content = toml::to_string_pretty(status).map_err(|error| error.to_string())?;
    atomic_write(&status_path(root), content.as_bytes())
}
/// Maximum admitted response bytes, checked before strict UTF-8 decoding.
const MAX_FETCH_BYTES: usize = 256 * 1024;
/// Calls the supplied transport with bounded retry and timeout policy.
fn fetch_text(
    url: &str,
    fetch: &mut impl FnMut(FetchRequest<'_>) -> Result<Vec<u8>, String>,
) -> Result<String, String> {
    let bytes = fetch(FetchRequest {
        url,
        max_bytes: MAX_FETCH_BYTES,
        retries: 2,
        connect_timeout: std::time::Duration::from_secs(5),
        total_timeout: std::time::Duration::from_secs(15),
    })?;
    if bytes.len() > MAX_FETCH_BYTES {
        return Err(format!(
            "response from {url} exceeded {MAX_FETCH_BYTES} bytes"
        ));
    }
    String::from_utf8(bytes).map_err(|error| format!("response was not UTF-8: {error}"))
}
/// Returns the catalog's containing URL rather than a filesystem directory.
fn base_url(url: &str) -> Result<String, String> {
    url.rsplit_once('/')
        .map(|(base, _)| base.to_owned())
        .ok_or_else(|| format!("catalog URL {url} has no base path"))
}
/// Resolves a validated relative document path against the catalog container.
fn join_url(base: &str, path: &str) -> Result<String, String> {
    if path.contains("://") || path.starts_with('/') || path.split('/').any(|part| part == "..") {
        return Err(format!("unsafe manifest path {path}"));
    }
    Ok(format!("{}/{}", base.trim_end_matches('/'), path))
}
/// Observes wall-clock Unix seconds for persisted status timestamps.
fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
/// Wire catalog schema with ordered authored entries.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestCatalog {
    /// Required catalog format version.
    schema_version: u32,
    #[serde(default)]
    /// Ordered document references.
    agents: Vec<ManifestCatalogAgent>,
}

/// One authored identity and relative document location.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestCatalogAgent {
    /// Authored catalogue label or alias.
    id: String,
    /// Unmodified safe relative URL path.
    path: String,
}

/// Resolved identity and safe location of one catalog entry.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CatalogAgent {
    /// Resolved canonical identity.
    agent: Agent,
    /// Unmodified safe relative URL path.
    path: String,
}

/// Validates schema, canonical identity uniqueness and relative path safety.
fn parse_catalog(content: &str) -> Result<Vec<CatalogAgent>, String> {
    let catalog: ManifestCatalog =
        toml::from_str(content).map_err(|err| format!("failed to parse catalog TOML: {err}"))?;
    if catalog.schema_version != 1 {
        return Err(format!(
            "unsupported catalog schema_version {}",
            catalog.schema_version
        ));
    }

    let mut seen = std::collections::BTreeSet::new();
    let mut agents = Vec::new();
    for entry in catalog.agents {
        let Some(agent) = crate::agents::parse_agent_label(&entry.id) else {
            tracing::warn!(agent = entry.id, "skipping unknown remote manifest agent");
            continue;
        };
        if entry.path.trim().is_empty() {
            return Err(format!("catalog entry {} has an empty path", entry.id));
        }
        if entry.path.contains("://")
            || entry.path.starts_with('/')
            || entry.path.split('/').any(|part| part == "..")
        {
            return Err(format!(
                "catalog entry {} has an unsafe path {}",
                entry.id, entry.path
            ));
        }
        if !seen.insert(agent_label(agent).to_string()) {
            return Err(format!("catalog contains duplicate agent {}", entry.id));
        }
        agents.push(CatalogAgent {
            agent,
            path: entry.path,
        });
    }
    Ok(agents)
}

#[cfg(test)]
mod tests;
