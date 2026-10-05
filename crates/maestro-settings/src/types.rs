use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// A persistent layer editable by callers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsScope {
    /// User configuration.
    User,
    /// Project configuration.
    Project,
}

/// Source of a winning leaf or an attempted edit, not a permission grant.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingsOrigin {
    /// Owned defaults and supplied engine values.
    Engine,
    /// Manifest values or lock authority.
    Manifest,
    /// User storage.
    User,
    /// Project storage.
    Project,
    /// Ephemeral caller with a nonsecret source identifier.
    Override(String),
}

/// Destination for a governed edit.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingsTarget {
    /// Replace a value in a stored layer.
    Stored(SettingsScope),
    /// Replace effective values without persisting them.
    Override(String),
}

/// The manifest's settings section; ordinary settings cannot change these locks.
#[derive(Clone, Debug, PartialEq)]
pub struct ManifestSettings {
    /// Values merged above engine settings.
    pub values: Map<String, Value>,
    /// Nonempty literal property paths freezing baseline JSON values or subtrees.
    pub locks: Vec<Vec<String>>,
}

/// Detached effective settings and their metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsSnapshot {
    /// Recursively merged values; arrays, scalars and present null replace.
    pub values: Map<String, Value>,
    /// One origin per scalar, null, atomic array or empty object.
    pub origins: BTreeMap<Vec<String>, SettingsOrigin>,
    /// Manifest-owned literal lock paths, including harmless duplicates.
    pub locks: Vec<Vec<String>>,
}

/// Safe file-operation diagnostics without file contents or parser excerpts.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingsFileError {
    /// Native I/O error category.
    Io(std::io::ErrorKind),
    /// Invalid JSON, using the parser's reported coordinates.
    Malformed {
        /// Parser-reported line.
        line: usize,
        /// Parser-reported column.
        column: usize,
    },
    /// Valid JSON whose root is not an object.
    NotObject,
    /// All acquisition attempts encountered contention.
    Contended,
    /// Caller cancelled before transaction admission.
    Cancelled,
}

/// Value-free diagnostic metadata; no attempted or frozen settings are retained.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingsError {
    /// Manifest lock has an empty path or traverses a nonobject.
    InvalidLock {
        /// Literal property path declared by the manifest.
        path: Vec<String>,
    },
    /// Manifest lock target does not exist in engine plus manifest values.
    MissingLockTarget {
        /// Literal property path declared by the manifest.
        path: Vec<String>,
    },
    /// An edit traverses a nonobject or supplies a nonobject root.
    InvalidPath {
        /// Attempted literal property path.
        path: Vec<String>,
        /// Caller attempting the edit.
        origin: SettingsOrigin,
    },
    /// An edit would change or omit a frozen value or subtree.
    LockConflict {
        /// Frozen literal property path.
        path: Vec<String>,
        /// Manifest authority, unchanged by equal restatements.
        locked_by: SettingsOrigin,
        /// Layer or caller attempting the change.
        attempted_by: SettingsOrigin,
    },
    /// File failure without publishing a new effective snapshot.
    File {
        /// Affected stored scope.
        scope: SettingsScope,
        /// Selected file path, not its contents.
        path: std::path::PathBuf,
        /// Value-free failure category.
        kind: SettingsFileError,
    },
    /// Invalid injected root or selected session-directory type.
    Location {
        /// Fixed input name, never its value.
        input: &'static str,
        /// Source of the invalid input. For a nonempty object `sessionDir`,
        /// the highest-precedence descendant origin is reported; overrides rank
        /// above project, user, manifest and engine origins.
        origin: SettingsOrigin,
    },
    /// Storage failed without publishing a new effective snapshot.
    Storage {
        /// Scope involved in the failure.
        scope: SettingsScope,
    },
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLock { path } => write!(f, "invalid Manifest lock at {path:?}"),
            Self::MissingLockTarget { path } => {
                write!(f, "missing Manifest lock target at {path:?}")
            }
            Self::InvalidPath { path, origin } => {
                write!(f, "invalid path {path:?} from {origin:?}")
            }
            Self::LockConflict {
                path,
                locked_by,
                attempted_by,
            } => write!(
                f,
                "lock conflict at {path:?}: locked by {locked_by:?}, attempted by {attempted_by:?}"
            ),
            Self::File { scope, path, kind } => write!(
                f,
                "settings file failure in {scope:?} at {path:?}: {kind:?}"
            ),
            Self::Location { input, origin } => {
                write!(f, "invalid settings location {input} from {origin:?}")
            }
            Self::Storage { scope } => write!(f, "settings storage failure in {scope:?}"),
        }
    }
}
impl std::error::Error for SettingsError {}
