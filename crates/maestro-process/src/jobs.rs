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
