//! Child-process records a guest shares with its host: the selection of each standard
//! descriptor, the selections of one launch and the way a child ended.
//!
//! The native extension owner and guest authoring re-export these records at their own paths.

/// Selection of one standard descriptor of a child.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Stdio {
    /// An independently retained raw byte pipe.
    #[default]
    Pipe,
    /// The native null device.
    Ignore,
    /// The host's corresponding descriptor.
    Inherit,
}

/// Literal selections for one child launch.
#[derive(Default)]
pub struct SpawnOptions {
    /// Omitted inherits the host directory.
    pub cwd: Option<String>,
    /// The entire environment; a present value replaces the ambient one, and a present-empty
    /// value leaves the child an empty environment.
    pub env: Option<Vec<(String, String)>>,
    /// Creates a new native session where the host supports it.
    pub detached: bool,
    /// Independent stdin, stdout and stderr selections.
    pub stdio: [Stdio; 3],
}

/// A numeric exit or a terminating signal, kept distinct.
#[derive(Debug, Eq, PartialEq)]
pub struct ExitStatus {
    /// Absent for signal termination.
    pub code: Option<i32>,
    /// Absent for numeric exit.
    pub signal: Option<String>,
}
