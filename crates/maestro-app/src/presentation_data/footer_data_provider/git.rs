//! Git metadata discovery and branch selection.

use std::io;

use maestro_path::{Cwd, dirname, join, resolve, try_resolve};

use super::operations::{FooterFileKind, FooterOperations};

/// Where a repository keeps the HEAD that names its branch.
pub(super) struct GitPaths {
    /// The directory whose `.git` entry named the repository.
    pub(super) repo_dir: String,
    /// The HEAD file that names the checked-out branch.
    pub(super) head_path: String,
    /// Shared metadata directory for this repository or linked worktree.
    pub(super) common_git_dir: String,
}

/// Trim as `String.prototype.trim` of the JavaScript specification does:
/// Unicode white space, except U+0085, plus U+FEFF.
fn trim(text: &str) -> &str {
    text.trim_matches(|c: char| (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}')
}

/// Resolve `paths` right to left; the working directory is read only when the
/// operands and the recorded drive directories leave a part open.
fn resolve_path(operations: &dyn FooterOperations, paths: &[&str]) -> io::Result<String> {
    let drives: Vec<(char, String)> = paths
        .iter()
        .filter_map(|path| {
            let [letter @ (b'a'..=b'z' | b'A'..=b'Z'), b':', ..] = path.as_bytes() else {
                return None;
            };
            let drive = letter.to_ascii_uppercase() as char;
            Some((drive, operations.drive_directory(drive)?))
        })
        .collect();
    let drives: Vec<(char, &str)> = drives.iter().map(|(d, path)| (*d, path.as_str())).collect();
    if let Ok(resolved) = try_resolve(paths, &drives) {
        return Ok(resolved);
    }
    let current = operations.current_dir()?;
    let cwd = Cwd {
        current: &current,
        drive_directories: &drives,
    };
    Ok(resolve(paths, &cwd))
}

/// Walk from `cwd` towards its root for the nearest directory with a `.git`
/// entry that names a repository.
///
/// A `.git` entry that cannot be examined, a worktree link that cannot be
/// followed and a repository without HEAD each end the walk with no result
/// instead of falling back to an enclosing repository.
pub(super) fn find_git_paths(operations: &dyn FooterOperations, cwd: &str) -> Option<GitPaths> {
    discover(operations, cwd).ok().flatten()
}

/// The walk of [`find_git_paths`], with its failures.
fn discover(operations: &dyn FooterOperations, cwd: &str) -> io::Result<Option<GitPaths>> {
    let mut dir = cwd.to_owned();
    loop {
        let git_path = join(&[&dir, ".git"]);
        'entry: {
            if !operations.exists(&git_path) {
                break 'entry;
            }
            let (git_dir, worktree) = match operations.stat_kind(&git_path)? {
                FooterFileKind::Directory => (git_path, false),
                FooterFileKind::File => {
                    let content = operations.read_text(&git_path)?;
                    let Some(target) = trim(&content).strip_prefix("gitdir: ") else {
                        break 'entry;
                    };
                    (resolve_path(operations, &[&dir, trim(target)])?, true)
                }
                FooterFileKind::Other => break 'entry,
            };
            let head_path = join(&[&git_dir, "HEAD"]);
            if !operations.exists(&head_path) {
                return Ok(None);
            }
            let common_dir = join(&[&git_dir, "commondir"]);
            let common_git_dir = if worktree && operations.exists(&common_dir) {
                let common = operations.read_text(&common_dir)?;
                resolve_path(operations, &[&git_dir, trim(&common)])?
            } else {
                git_dir
            };
            return Ok(Some(GitPaths {
                repo_dir: dir,
                head_path,
                common_git_dir,
            }));
        }
        let parent = dirname(&dir);
        if parent == dir {
            return Ok(None);
        }
        dir = parent;
    }
}

/// The branch HEAD names: `detached` for any other readable HEAD, and the branch Git
/// reports when HEAD holds the placeholder a reftable repository writes.
///
/// A HEAD that cannot be read gives `None`.
pub(super) fn resolve_branch(
    operations: &dyn FooterOperations,
    paths: &GitPaths,
) -> Option<String> {
    match read_branch(operations, paths) {
        BranchRead::Ready(branch) => branch,
        BranchRead::Query(repo) => Some(git_answer(
            operations.symbolic_ref_sync(repo).ok().flatten(),
        )),
    }
}
/// HEAD selection or the directory requiring a symbolic-reference query.
pub(super) enum BranchRead<'a> {
    /// A completed HEAD read, including unreadable or absent metadata.
    Ready(Option<String>),
    /// A placeholder requiring Git in this repository directory.
    Query(&'a str),
}
/// Select a readable HEAD before deciding whether Git is needed.
pub(super) fn read_branch<'a>(
    operations: &dyn FooterOperations,
    paths: &'a GitPaths,
) -> BranchRead<'a> {
    let Ok(head) = operations.read_text(&paths.head_path) else {
        return BranchRead::Ready(None);
    };
    let Some(branch) = trim(&head).strip_prefix("ref: refs/heads/") else {
        return BranchRead::Ready(Some("detached".to_owned()));
    };
    if branch == ".invalid" {
        BranchRead::Query(&paths.repo_dir)
    } else {
        BranchRead::Ready(Some(branch.to_owned()))
    }
}
/// Trim successful stdout, falling back to detached on failure or empty text.
pub(super) fn git_answer(output: Option<String>) -> String {
    output
        .map(|stdout| trim(&stdout).to_owned())
        .filter(|branch| !branch.is_empty())
        .unwrap_or_else(|| "detached".to_owned())
}
