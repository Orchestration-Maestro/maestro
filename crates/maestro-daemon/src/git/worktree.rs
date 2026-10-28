//! Worktree names, checkout paths and the Git commands that add, remove and list worktrees.
use super::discovery::{
    canonicalize_best_effort_path, discover_git_paths, git_output, read_text, trim,
};
use crate::paths::expand_home_path;
use maestro_process::SpawnSyncError;
use std::{
    ffi::OsStr,
    fmt,
    path::{Path, PathBuf},
    process::Output,
};

/// Namespace of generated branch names and the folder name of a branch with no usable letters.
const WORKTREE_PREFIX: &str = "worktree";
/// First words of a generated branch name; the table length is the modulus that picks one.
const ADJECTIVES: [&str; 8] = [
    "brave", "calm", "clear", "green", "lucky", "quiet", "rapid", "silver",
];
/// Second words of a generated branch name.
const NOUNS: [&str; 8] = [
    "river", "cloud", "field", "forest", "harbor", "meadow", "stone", "valley",
];

/// A failed worktree operation, carrying the message Git printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeError {
    /// The text a caller shows or matches.
    message: String,
}

impl WorktreeError {
    /// Wraps a message that came from Git or from checking a checkout.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// The message Git gives when a checkout holds changes and removal is not forced.
    #[must_use]
    pub fn dirty_checkout(path: &Path) -> Self {
        Self::new(format!(
            "fatal: '{}' contains modified or untracked files, use --force to delete it",
            path.display()
        ))
    }

    /// The text of the failure.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Whether Git refused removal because the checkout holds changes that `--force` discards.
    #[must_use]
    pub fn is_dirty_checkout(&self) -> bool {
        let message = self.message.to_ascii_lowercase();
        message.contains("contains modified or untracked files")
            && message.contains("use --force to delete it")
    }

    /// Whether Git answered that the path is not one of its worktrees.
    fn is_not_a_working_tree(&self) -> bool {
        let message = self.message.to_ascii_lowercase();
        message.contains("is not a working tree") || message.contains("is not a worktree")
    }
}

impl fmt::Display for WorktreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for WorktreeError {}

/// One entry of `git worktree list --porcelain`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingWorktree {
    /// The checkout directory as Git printed it.
    pub path: PathBuf,
    /// The checked-out branch without `refs/heads/`, absent when detached or bare.
    pub branch: Option<String>,
    /// Whether the entry is the bare repository itself.
    pub is_bare: bool,
    /// Whether HEAD is detached.
    pub is_detached: bool,
    /// Whether Git would prune the entry because its checkout is gone.
    pub is_prunable: bool,
}

impl ExistingWorktree {
    /// Parses the output of `git worktree list --porcelain`.
    ///
    /// Entries are separated by blank lines; lines before the first `worktree` line of an
    /// entry belong to no entry.
    #[must_use]
    pub fn parse_porcelain(output: &[u8]) -> Vec<Self> {
        let mut entries = Vec::new();
        let mut current: Option<Self> = None;
        for line in output.split(|byte| *byte == b'\n') {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.iter().all(u8::is_ascii_whitespace) {
                entries.extend(current.take());
            } else if let Some(path) = line.strip_prefix(b"worktree ") {
                entries.extend(current.replace(Self::at(path_from_bytes(path))));
            } else if let Some(entry) = current.as_mut() {
                entry.read_attribute(line);
            }
        }
        entries.extend(current);
        entries
    }

    /// An entry with no branch and no flags.
    fn at(path: PathBuf) -> Self {
        Self {
            path,
            branch: None,
            is_bare: false,
            is_detached: false,
            is_prunable: false,
        }
    }

    /// Applies one attribute line of the entry.
    fn read_attribute(&mut self, line: &[u8]) {
        if let Some(branch) = line.strip_prefix(b"branch ") {
            let branch = String::from_utf8_lossy(branch);
            self.branch = Some(
                branch
                    .strip_prefix("refs/heads/")
                    .unwrap_or(&branch)
                    .to_owned(),
            );
        } else if line == b"detached" {
            self.is_detached = true;
        } else if line == b"bare" {
            self.is_bare = true;
        } else if line.starts_with(b"prunable") {
            self.is_prunable = true;
        }
    }
}

/// Builds a path from the bytes Git printed, keeping bytes that text cannot hold.
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec()))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// A branch name for a worktree created without one.
#[must_use]
pub fn generated_branch_slug(seed: u64) -> String {
    let pick = |words: [&'static str; 8], index: u64| {
        let index = usize::try_from(index % 8).unwrap_or_default();
        words.get(index).copied().unwrap_or(WORKTREE_PREFIX)
    };
    format!(
        "{WORKTREE_PREFIX}/{}-{}-{:04x}",
        pick(ADJECTIVES, seed),
        pick(NOUNS, seed / 8),
        seed & 0xffff
    )
}

/// A folder name for a branch: lowercase ASCII letters and digits joined by single dashes.
#[must_use]
pub fn branch_to_path_slug(branch: &str) -> String {
    let mut slug = String::new();
    for ch in branch.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        WORKTREE_PREFIX.to_owned()
    } else {
        slug.to_owned()
    }
}

/// The absolute folder that holds every repository's checkouts.
///
/// `directory` is the authored `daemon.worktreesDirectory`. A leading `~` expands against the
/// directory `home` supplies, and a relative result lies below the current directory.
#[must_use]
pub fn worktrees_root(directory: &str, home: impl FnOnce() -> Option<PathBuf>) -> PathBuf {
    let path = expand_home_path(directory, home);
    if path.is_absolute() {
        return path;
    }
    match std::env::current_dir() {
        Ok(cwd) => cwd.join(path),
        Err(_) => path,
    }
}

/// The checkout folder for `branch` of the repository named `repo_name`.
#[must_use]
pub fn default_checkout_path(root: &Path, repo_name: &str, branch: &str) -> PathBuf {
    root.join(repo_name).join(branch_to_path_slug(branch))
}

/// Views any operand as the native string a command receives.
fn os(operand: &(impl AsRef<OsStr> + ?Sized)) -> &OsStr {
    operand.as_ref()
}

/// Runs `git -C <dir> <args>` to completion with its output captured.
fn capture(dir: &Path, args: &[&OsStr]) -> Result<Output, WorktreeError> {
    git_output(dir, args).map_err(|error| {
        WorktreeError::new(match error {
            SpawnSyncError::Infrastructure(error)
            | SpawnSyncError::Spawn(error)
            | SpawnSyncError::Wait(error) => format!("cannot run git: {error}"),
            SpawnSyncError::Timeout => "git timed out".to_owned(),
        })
    })
}

/// Git's message for a failed run: its standard error, else its standard output, else the exit
/// status of the `label` command.
fn failure(label: &str, output: &Output) -> WorktreeError {
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).trim().to_owned();
    let (stderr, stdout) = (text(&output.stderr), text(&output.stdout));
    WorktreeError::new(if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("git {label} failed with status {}", output.status)
    })
}

/// Runs the `label` command and returns its standard output when Git succeeds.
fn run(dir: &Path, label: &str, args: &[&OsStr]) -> Result<Vec<u8>, WorktreeError> {
    let output = capture(dir, args)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(failure(label, &output))
    }
}

/// Whether `refs/heads/<branch>` exists in the repository.
///
/// # Errors
/// Returns Git's message when the lookup fails for a reason other than a missing branch.
pub fn local_branch_exists(repo_root: &Path, branch: &str) -> Result<bool, WorktreeError> {
    let reference = format!("refs/heads/{branch}");
    let output = capture(
        repo_root,
        &[
            os("show-ref"),
            os("--verify"),
            os("--quiet"),
            os(&reference),
        ],
    )?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(failure("show-ref", &output)),
    }
}

/// Adds a checkout of `branch` at `path`, creating the branch from `base` when it is new.
///
/// The branch, path and base are operands, never options: a base that starts with a dash is
/// read as a revision.
///
/// # Errors
/// Returns Git's message when the branch lookup or `git worktree add` fails.
pub fn add(repo_root: &Path, path: &Path, branch: &str, base: &str) -> Result<(), WorktreeError> {
    let mut args = vec![os("worktree"), os("add")];
    if local_branch_exists(repo_root, branch)? {
        args.extend([os("--"), os(path), os(branch)]);
    } else {
        args.extend([os("-b"), os(branch), os("--"), os(path), os(base)]);
    }
    run(repo_root, "worktree add", &args).map(drop)
}

/// Removes the checkout at `path`; a forced removal also clears a leftover directory Git no
/// longer lists.
///
/// # Errors
/// Returns Git's message, or the cleanup failure of a leftover directory.
pub fn remove(repo_root: &Path, path: &Path, force: bool) -> Result<(), WorktreeError> {
    let mut args = vec![os("worktree"), os("remove")];
    if force {
        args.push(os("--force"));
    }
    args.extend([os("--"), os(path)]);
    match run(repo_root, "worktree remove", &args) {
        Err(error) if force && error.is_not_a_working_tree() => {
            recover_leftover(repo_root, path, error)
        }
        result => result.map(drop),
    }
}

/// Settles a forced removal that Git answered with "not a working tree".
///
/// A path Git still lists keeps Git's answer. A directory that is not there is already
/// removed. A directory that is there goes only when its `.git` link names an admin folder of
/// this repository, so an unrelated replacement is never deleted.
fn recover_leftover(
    repo_root: &Path,
    path: &Path,
    error: WorktreeError,
) -> Result<(), WorktreeError> {
    let expected = canonicalize_best_effort_path(path);
    let listed = list(repo_root)?
        .iter()
        .any(|entry| canonicalize_best_effort_path(&entry.path) == expected);
    if listed {
        return Err(error);
    }
    if path.exists() {
        if !links_to_repository(repo_root, path) {
            return Err(error);
        }
        std::fs::remove_dir_all(path).map_err(|failure| {
            WorktreeError::new(format!(
                "{error}; failed to remove leftover checkout {}: {failure}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

/// Whether the `.git` link file of the checkout names an admin folder under the repository's
/// `worktrees` directory.
fn links_to_repository(repo_root: &Path, path: &Path) -> bool {
    let Ok(link) = read_text(&path.join(".git")) else {
        return false;
    };
    let Some(admin) = trim(&link).strip_prefix("gitdir:") else {
        return false;
    };
    let Some(paths) = discover_git_paths(repo_root) else {
        return false;
    };
    let worktrees = canonicalize_best_effort_path(&paths.git_common_dir).join("worktrees");
    canonicalize_best_effort_path(&path.join(trim(admin))).starts_with(worktrees)
}

/// Whether the checkout at `path` has modified or untracked files.
///
/// # Errors
/// Returns Git's message when `git status` fails.
pub fn checkout_has_dirty_files(path: &Path) -> Result<bool, WorktreeError> {
    let args = [os("status"), os("--porcelain"), os("--untracked-files=all")];
    run(path, "status", &args).map(|stdout| !stdout.is_empty())
}

/// Lists the worktrees of the repository.
///
/// # Errors
/// Returns Git's message when `git worktree list` fails.
pub fn list(repo_root: &Path) -> Result<Vec<ExistingWorktree>, WorktreeError> {
    let args = [os("worktree"), os("list"), os("--porcelain")];
    run(repo_root, "worktree list", &args).map(|stdout| ExistingWorktree::parse_porcelain(&stdout))
}

#[cfg(test)]
mod tests;
