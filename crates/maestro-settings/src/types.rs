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
            Self::Storage { scope } => write!(f, "settings storage failure in {scope:?}"),
        }
    }
}
impl std::error::Error for SettingsError {}
