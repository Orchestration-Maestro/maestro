//! Shared native repository discovery with process-owning command fallbacks.
use maestro_process::{OutputMode, SpawnSyncOptions, spawn_sync};
use std::path::Path;

pub use maestro_workspace::workspace::git::discovery::{
    GitSpaceMetadata, GitWorktreeInfo, automatic_workspace_label, canonicalize_best_effort_path,
    derive_label_from_cwd, fallback_label_from_cwd, git_repo_root, git_space_metadata,
    git_worktree_info,
};
pub use maestro_workspace::workspace::git::native::{
    GitPaths, discover_git_paths, git_ref_storage_is_reftable, read_text, trim,
};

/// Read the branch name, querying Git only for reftable storage.
#[must_use]
pub fn git_branch(cwd: &Path) -> Option<String> {
    let paths = discover_git_paths(cwd)?;
    if git_ref_storage_is_reftable(&paths.git_common_dir) {
        return git_trimmed_stdout(
            &paths.repo_root,
            &["symbolic-ref", "--quiet", "--short", "HEAD"],
        );
    }
    let head = read_text(&paths.git_dir.join("HEAD")).ok()?;
    let branch = trim(&head).strip_prefix("ref: refs/heads/")?;
    (!branch.is_empty()).then(|| branch.to_owned())
}

/// Resolve an existing revision through Git, including reftable references.
#[must_use]
pub fn git_rev_parse_verify(repo_root: &Path, revision: &str) -> Option<String> {
    git_trimmed_stdout(repo_root, &["rev-parse", "--verify", revision])
}

/// Run a noninteractive Git query through the existing process owner.
fn git_trimmed_stdout(repo_root: &Path, args: &[&str]) -> Option<String> {
    let mut command = maestro_process::command("git");
    command.arg("-C").arg(repo_root).args(args);
    let output = spawn_sync(
        &mut command,
        SpawnSyncOptions::Capture {
            max_buffer_bytes: None,
            stdout: OutputMode::Capture,
            input: None,
            timeout: None,
            stderr: OutputMode::Discard,
        },
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let stdout = trim(&stdout);
    (!stdout.is_empty()).then(|| stdout.to_owned())
}
