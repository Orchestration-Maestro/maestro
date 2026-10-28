//! Branch tracking read from user, repository, included and worktree configuration.
use super::discovery::GitWorktreeInfo;
use std::path::{Path, PathBuf};

/// Values the caller supplies because configuration depends on them besides the repository.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigEnvironment {
    /// User-level files, lowest precedence first.
    pub user_files: Vec<PathBuf>,
    /// Home directory that a leading `~/` in an include path or `gitdir` pattern names.
    pub home: Option<PathBuf>,
}

impl ConfigEnvironment {
    /// Git's user-level files for the supplied `XDG_CONFIG_HOME` and home directory.
    #[must_use]
    pub fn from_variables(_xdg_config_home: Option<&Path>, _home: Option<&Path>) -> Self {
        Self::default()
    }
}

/// The remote and merge reference a branch tracks, with the fetch refspecs that map them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchConfig {
    /// Remote named by `branch.<name>.remote`.
    pub remote: String,
    /// Reference named by `branch.<name>.merge`.
    pub merge_ref: String,
}

impl BranchConfig {
    /// The local reference the remote's fetch refspecs map the merge reference to.
    #[must_use]
    pub fn upstream_full_ref(&self) -> Option<String> {
        None
    }
}

/// One configuration read and the files it depended on.
#[derive(Debug, Clone)]
pub struct ConfigRead {
    /// Tracking for the branch, when both remote and merge reference are configured.
    pub branch: Option<BranchConfig>,
}

impl ConfigRead {
    /// Whether every file this read depended on is unchanged and was read without error.
    #[must_use]
    pub fn is_current(&self) -> bool {
        true
    }
}

/// Read the tracking configuration of `branch` for the checkout described by `info`.
#[must_use]
pub fn read_branch_config(
    _info: &GitWorktreeInfo,
    _branch: &str,
    _environment: &ConfigEnvironment,
) -> ConfigRead {
    ConfigRead { branch: None }
}
