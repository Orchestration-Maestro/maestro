//! Owned daemon selection, inventory and local command guidance.

use crate::paths::DaemonPaths;

/// Environment input selecting an inherited daemon name.
pub const SESSION_ENV_VAR: &str = "MAESTRO_DAEMON";
/// Reserved target identifying the daemon at the agent root.
pub const DEFAULT_SESSION_NAME: &str = "default";

/// Supplied process-environment observations; never mutated by selection.
#[derive(Clone, Copy, Debug, Default)]
pub struct SelectionEnvironment<'a> {
    /// Inherited daemon target.
    pub daemon_name: Option<&'a str>,
    /// API socket override.
    pub api_socket: Option<&'a str>,
    /// Client-only socket override.
    pub client_socket: Option<&'a str>,
}

/// Retained selection for a single daemon client or host.
#[derive(Clone, Debug)]
pub struct Selection {
    /// Shared root and selected daemon resource paths.
    pub paths: DaemonPaths,
    /// Whether command input explicitly chose this target.
    pub explicit: bool,
    /// Selected API endpoint, including an applicable override.
    pub api_socket_path: String,
    /// Selected client endpoint from the shared socket selector.
    pub client_socket_path: std::path::PathBuf,
    /// Retained implicit API override for restart guidance.
    api_override: Option<String>,
}

impl Selection {
    /// Observe attach arguments or a supplied daemon-command target without rewriting argv.
    ///
    /// # Errors
    /// Returns missing daemon-option values or invalid selected names.
    #[cfg(unix)]
    pub fn from_args(
        agent_dir: &str,
        args: &[String],
        target: Option<&str>,
        environment: SelectionEnvironment<'_>,
    ) -> Result<Self, String> {
        let request = target.map_or_else(
            || attach_target(args),
            |name| Ok(AttachRequest::Named(name.to_owned())),
        )?;
        let environment = if matches!(request, AttachRequest::Help) {
            SelectionEnvironment::default()
        } else {
            environment
        };
        let requested = match request {
            AttachRequest::Named(name) => Some(name),
            AttachRequest::Inherited | AttachRequest::Help => None,
        };
        let explicit = requested.is_some();
        let overridden = environment.api_socket.is_some() || environment.client_socket.is_some();
        let name = requested
            .as_deref()
            .or_else(|| (!overridden).then_some(environment.daemon_name).flatten());
        let paths = DaemonPaths::new(agent_dir, name)?;
        let api_override = (!explicit).then_some(environment.api_socket).flatten();
        let api_socket_path = api_override.map_or_else(|| paths.api_socket_path(), str::to_owned);
        let client_socket_path = if explicit {
            paths.client_socket_path(None, None, None)
        } else {
            paths.client_socket_path(None, api_override, environment.client_socket)
        };
        Ok(Self {
            paths,
            explicit,
            api_socket_path,
            client_socket_path,
            api_override: api_override.map(str::to_owned),
        })
    }

    /// Restart instructions retaining a custom API socket when one selected this daemon.
    #[must_use]
    pub fn restart_after_update_guidance(&self) -> String {
        if let Some(socket) = &self.api_override {
            return restart_after_update_guidance(
                &format!("MAESTRO_SOCKET_PATH={socket} maestro daemon stop"),
                None,
            );
        }
        restart_after_update_guidance(
            &self.paths.local_stop_command(),
            Some(&self.paths.local_attach_command()),
        )
    }
}

/// Attach help bypasses name validation without becoming an explicit target.
#[cfg(unix)]
enum AttachRequest {
    /// No target supplied by the command.
    Inherited,
    /// Help belongs to command dispatch, not daemon selection.
    Help,
    /// An explicitly supplied daemon target, before validation.
    Named(String),
}

/// Observe attach-only options before the child-argument separator.
#[cfg(unix)]
fn attach_target(args: &[String]) -> Result<AttachRequest, String> {
    if args.get(1).map(String::as_str) != Some("attach") {
        return Ok(AttachRequest::Inherited);
    }
    let mut requested = AttachRequest::Inherited;
    let mut iter = args.iter().skip(2).take_while(|arg| arg.as_str() != "--");
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "help" | "--help" | "-h" => return Ok(AttachRequest::Help),
            "--daemon" => {
                requested =
                    AttachRequest::Named(iter.next().ok_or("missing value for --daemon")?.clone());
            }
            "--session" => {
                iter.next();
            }
            _ => {
                if let Some(value) = arg.strip_prefix("--daemon=") {
                    requested = AttachRequest::Named(value.to_owned());
                }
            }
        }
    }
    Ok(requested)
}

/// Validate one ASCII daemon-directory component.
///
/// # Errors
/// Returns a diagnostic for empty, dot, oversized or non-component names.
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("daemon name cannot be empty".into());
    }
    if name.len() > 64 {
        return Err("daemon name cannot be longer than 64 bytes".into());
    }
    if matches!(name, "." | "..") {
        return Err("daemon name cannot be . or ..".into());
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err("daemon name may only contain ASCII letters, numbers, '.', '_' and '-'".into());
    }
    Ok(())
}

/// Normalize the reserved default name to the agent root.
///
/// # Errors
/// Returns the daemon-name diagnostic for an invalid target.
pub fn parse_target_name(name: &str) -> Result<Option<String>, String> {
    validate_name(name)?;
    Ok((name != DEFAULT_SESSION_NAME).then(|| name.to_owned()))
}

impl DaemonPaths {
    /// Attach command for this target.
    #[must_use]
    pub fn local_attach_command(&self) -> String {
        self.name().map_or_else(
            || "maestro attach".into(),
            |name| format!("maestro attach --daemon {name}"),
        )
    }

    /// Stop command for this target; stopping itself is owned by the daemon host.
    #[must_use]
    pub fn local_stop_command(&self) -> String {
        self.name().map_or_else(
            || "maestro daemon stop".into(),
            |name| format!("maestro daemon stop {name}"),
        )
    }
}

/// Three-line restart instructions with the pane-process warning.
#[must_use]
pub fn restart_after_update_guidance(stop_command: &str, attach_command: Option<&str>) -> String {
    let restart = attach_command.map_or_else(
        || format!("Run `{stop_command}`, then restart Maestro with the same socket override."),
        |command| format!("Run `{stop_command}`, then run `{command}` again."),
    );
    format!(
        "Stop the old daemon to use the new version.\nStopping exits pane processes.\n{restart}"
    )
}

/// Observed daemon identity and liveness, without selecting it globally.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SessionInfo {
    /// Display name, including the reserved default target.
    pub name: String,
    /// Whether the daemon lives at the supplied agent root.
    pub default: bool,
    /// Whether a local connection to its API socket succeeds.
    pub running: bool,
    /// Authored API socket path.
    pub socket_path: String,
    /// Authored daemon data directory.
    pub session_dir: String,
}

/// Observe one validated daemon's local API endpoint.
#[cfg(unix)]
#[must_use]
pub fn session_info(paths: &DaemonPaths) -> SessionInfo {
    let socket_path = paths.api_socket_path();
    SessionInfo {
        name: paths.name().unwrap_or(DEFAULT_SESSION_NAME).to_owned(),
        default: paths.name().is_none(),
        running: is_running_at(std::path::Path::new(&socket_path)),
        socket_path,
        session_dir: paths.data_dir(),
    }
}

/// List default once followed by valid named daemon directories in lexical order.
///
/// # Errors
/// Returns directory-reading or entry-metadata errors, except a missing directory.
#[cfg(unix)]
pub fn list_sessions(agent_dir: &str) -> std::io::Result<Vec<SessionInfo>> {
    let root = DaemonPaths::new(agent_dir, None).map_err(std::io::Error::other)?;
    let mut sessions = vec![session_info(&root)];
    let entries = match std::fs::read_dir(root.instances_dir()) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(sessions),
        Err(error) => return Err(error),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if let Some(name) = entry
            .file_name()
            .to_str()
            .filter(|name| *name != DEFAULT_SESSION_NAME && validate_name(name).is_ok())
        {
            names.push(name.to_owned());
        }
    }
    names.sort();
    for name in names {
        let paths = DaemonPaths::new(agent_dir, Some(&name)).map_err(std::io::Error::other)?;
        sessions.push(session_info(&paths));
    }
    Ok(sessions)
}

/// Remove an idle named daemon, tolerating an already missing directory.
///
/// # Errors
/// Refuses default and running daemons; returns validation or removal errors.
#[cfg(unix)]
pub fn delete_session(agent_dir: &str, name: &str) -> Result<SessionInfo, String> {
    let paths = DaemonPaths::new(agent_dir, Some(name))?;
    if paths.name().is_none() {
        return Err("deleting the default daemon is not supported".into());
    }
    let info = session_info(&paths);
    if info.running {
        return Err(format!("daemon {name} is running; stop it before deleting"));
    }
    match std::fs::remove_dir_all(paths.data_dir()) {
        Ok(()) => Ok(info),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(info),
        Err(error) => Err(error.to_string()),
    }
}

/// A graceful stop through the selected API socket.
#[cfg(unix)]
mod stop;

#[cfg(unix)]
pub use stop::stop_session;

/// Probe the existing local socket owner without a second transport or stop protocol.
#[cfg(unix)]
fn is_running_at(path: &std::path::Path) -> bool {
    path.exists() && maestro_ipc::local_socket::connect_local_stream(path).is_ok()
}

#[cfg(test)]
mod tests;
