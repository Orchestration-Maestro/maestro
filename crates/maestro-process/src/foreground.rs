//! Foreground process observations shared across native platforms.

/// One live member of a foreground process group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForegroundProcess {
    /// Native process identity.
    pub pid: u32,
    /// Kernel process name.
    pub name: String,
    /// Separately reported executable name when available.
    pub argv0: Option<String>,
    /// Argument values when available.
    pub argv: Option<Vec<String>>,
    /// Combined command-line text when available.
    pub cmdline: Option<String>,
}

/// Live members of a foreground process group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForegroundJob {
    /// Native process-group identity.
    pub process_group_id: u32,
    /// Observed group members.
    pub processes: Vec<ForegroundProcess>,
}

/// Whether this process leads its own native session, as a detached daemon does.
#[cfg(unix)]
#[must_use]
pub fn current_process_is_detached_server_daemon() -> bool {
    rustix::process::getsid(None).is_ok_and(|session| session == rustix::process::getpid())
}
