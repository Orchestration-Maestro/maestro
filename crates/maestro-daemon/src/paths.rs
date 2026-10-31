//! Authored daemon paths rooted in a supplied agent directory.

use maestro_detection::agents::{Agent, agent_label};
use std::path::PathBuf;

/// Agent-wide resources and a validated optional daemon identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DaemonPaths {
    /// The caller's original agent-directory spelling.
    agent_dir: String,
    /// A named daemon, or the default daemon at the agent root.
    name: Option<String>,
}

impl DaemonPaths {
    /// Retain a root and validate a target without creating any files.
    ///
    /// # Errors
    /// Returns the daemon-name diagnostic for an invalid target.
    pub fn new(agent_dir: &str, name: Option<&str>) -> Result<Self, String> {
        Ok(Self {
            agent_dir: agent_dir.to_owned(),
            name: name
                .map(crate::instances::parse_target_name)
                .transpose()?
                .flatten(),
        })
    }

    /// The supplied agent root, unchanged.
    #[must_use]
    pub fn agent_dir(&self) -> &str {
        &self.agent_dir
    }

    /// The validated name, absent for the default daemon.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The default root or selected named daemon folder.
    #[must_use]
    pub fn data_dir(&self) -> String {
        self.name.as_ref().map_or_else(
            || self.agent_dir.clone(),
            |name| child(&self.instances_dir(), name),
        )
    }

    /// Shared parent of named-daemon directories.
    pub(crate) fn instances_dir(&self) -> String {
        child(&self.agent_dir, "daemons")
    }

    /// The API socket's authored path.
    #[must_use]
    pub fn api_socket_path(&self) -> String {
        child(&self.data_dir(), "daemon.sock")
    }

    /// Select the client socket through the native shared owner.
    #[cfg(unix)]
    #[must_use]
    pub fn client_socket_path(
        &self,
        explicit: Option<&str>,
        api_override: Option<&str>,
        client_override: Option<&str>,
    ) -> std::path::PathBuf {
        maestro_ipc::socket_paths::client_socket_path(
            explicit.map(std::path::Path::new),
            api_override,
            client_override,
            std::path::Path::new(&self.data_dir()),
        )
    }

    /// The retained topology snapshot.
    #[must_use]
    pub fn snapshot_path(&self) -> String {
        child(&self.data_dir(), "daemon-snapshot.json")
    }

    /// The separately retained pane history.
    #[must_use]
    pub fn history_path(&self) -> String {
        child(&self.data_dir(), "daemon-snapshot-history.json")
    }

    /// The log for an in-process, client or daemon role.
    #[must_use]
    pub fn log_path(&self, role: &str) -> String {
        child(&self.data_dir(), &format!("maestro-{role}.log"))
    }

    /// An agent's shared detection override.
    #[must_use]
    pub fn detection_override(&self, agent: Agent) -> String {
        child(
            &self.agent_dir,
            &format!("detection/{}.toml", agent_label(agent)),
        )
    }

    /// The shared downloaded detection catalog folder.
    #[must_use]
    pub fn detection_catalog(&self) -> String {
        child(&self.agent_dir, "detection/catalog")
    }

    /// The catalog update status file.
    #[must_use]
    pub fn detection_status(&self) -> String {
        child(&self.detection_catalog(), "status.toml")
    }

    /// A downloaded remote detection manifest.
    #[must_use]
    pub fn detection_manifest(&self, agent: Agent) -> String {
        child(
            &self.detection_catalog(),
            &format!("remote/{}.toml", agent_label(agent)),
        )
    }

    /// An action's shared configuration folder.
    #[must_use]
    pub fn action_config(&self, id: &str) -> String {
        child(
            &self.agent_dir,
            &format!("workspace-actions/{}/config", action_path_component(id)),
        )
    }

    /// An action's shared state folder.
    #[must_use]
    pub fn action_state(&self, id: &str) -> String {
        child(
            &self.agent_dir,
            &format!("workspace-actions/{}/state", action_path_component(id)),
        )
    }

    /// The product announcement state file.
    #[must_use]
    pub fn announcements(&self) -> String {
        child(&self.agent_dir, "announcements.json")
    }

    /// The shared remote resource folder.
    #[must_use]
    pub fn remote(&self) -> String {
        child(&self.agent_dir, "remote")
    }
}

/// Append a known relative component without normalizing the supplied root.
fn child(root: &str, relative: &str) -> String {
    if root.is_empty() || root.ends_with(std::path::MAIN_SEPARATOR) || root.ends_with('/') {
        format!("{root}{relative}")
    } else {
        format!("{root}/{relative}")
    }
}

/// Expands only an exact `~` or a leading `~/` (`~\` on Windows) against the home directory
/// that `home` supplies, joining the rest as a native path. Every other spelling is kept,
/// and `home` is asked for only when the spelling starts with `~`.
pub(crate) fn expand_home_path(path: &str, home: impl FnOnce() -> Option<PathBuf>) -> PathBuf {
    let Some(rest) = path.strip_prefix('~') else {
        return path.into();
    };
    if !rest.is_empty() && !rest.starts_with(std::path::is_separator) {
        return path.into();
    }
    let Some(home) = home() else {
        return path.into();
    };
    if rest.is_empty() {
        return home;
    }
    rest.strip_prefix(std::path::is_separator)
        .map_or_else(|| path.into(), |rest| join_tilde_rest(home, rest))
}

/// Joins `rest` below `home` natively: on Windows every separator splits the rest and empty
/// components are dropped; elsewhere an absolute rest replaces the home.
fn join_tilde_rest(home: PathBuf, rest: &str) -> PathBuf {
    if cfg!(windows) {
        rest.split(['/', '\\'])
            .filter(|component| !component.is_empty())
            .fold(home, |path, component| path.join(component))
    } else {
        home.join(rest)
    }
}

/// Maximum ASCII bytes in an encoded action component.
const ACTION_PATH_COMPONENT_MAX_BYTES: usize = 120;

/// Encode an arbitrary action identity into one bounded filesystem component.
#[must_use]
pub fn action_path_component(value: &str) -> String {
    let mut component = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        {
            component.push(byte as char);
        } else {
            use std::fmt::Write as _;
            let _ = write!(component, "%{byte:02X}");
        }
    }
    if component.ends_with('.') {
        component.pop();
        component.push_str("%2E");
    }
    if component.is_empty() {
        return "%plugin".to_string();
    }
    if has_windows_reserved_stem_for_path_component(&component) {
        component = format!("%{component}");
    }
    if component.len() > ACTION_PATH_COMPONENT_MAX_BYTES {
        let hash = short_action_id_hash_for_path_component(value);
        let prefix_len = ACTION_PATH_COMPONENT_MAX_BYTES - hash.len() - 1;
        let prefix = component[..prefix_len].to_owned();
        return format!("{prefix}-{hash}");
    }
    component
}

/// First six SHA-256 bytes of the raw identity, unlike the package cache's
/// four-byte digest of a prefixed identity.
fn short_action_id_hash_for_path_component(value: &str) -> String {
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(value.as_bytes());
    let mut hash = String::with_capacity(12);
    for byte in &digest[..6] {
        use std::fmt::Write as _;
        let _ = write!(hash, "{byte:02x}");
    }
    hash
}

/// Whether the first dot-delimited stem is reserved by Windows.
fn has_windows_reserved_stem_for_path_component(value: &str) -> bool {
    let stem = value.split('.').next().unwrap_or(value);
    maestro_path::win32::is_reserved(stem)
}
