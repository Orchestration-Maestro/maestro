//! `gitdir` conditions compare a checkout's native spelling with the authored pattern.
use super::{PATHNAME, Rules, Subject};
use crate::workspace::git::discovery::GitWorktreeInfo;
use std::path::{Path, PathBuf};

/// A checkout whose paths native Windows spell with `\`.
fn windows_checkout() -> GitWorktreeInfo {
    let git_dir = PathBuf::from(r"D:\work\repo\.git");
    GitWorktreeInfo {
        repo_root: PathBuf::from(r"D:\work\repo"),
        git_dir: git_dir.clone(),
        git_common_dir: git_dir,
        is_bare: false,
        is_linked_worktree: false,
    }
}

/// `info` judged from the configuration file in its own metadata directory.
fn subject(info: &GitWorktreeInfo) -> Subject<'_> {
    Subject {
        info,
        branch: "main",
        home: None,
    }
}

#[test]
fn gitdir_condition_matches_a_windows_spelled_checkout() {
    let info = windows_checkout();
    let subject = subject(&info);
    let config = Path::new(r"D:\work\repo\.git\config");
    for pattern in [r"D:/work/repo/.git", r"D:\work\repo\.git"] {
        assert!(
            subject.git_dir_matches(pattern, config, PATHNAME, true),
            "{pattern}"
        );
    }
}

#[test]
fn gitdir_conditions_keep_their_case_rules_on_windows_spellings() {
    let info = windows_checkout();
    let subject = subject(&info);
    let config = Path::new(r"D:\work\repo\.git\config");
    let upper = r"D:/WORK/REPO/.GIT";
    let ignoring_case = Rules {
        ignore_case: true,
        ..PATHNAME
    };
    assert!(subject.git_dir_matches(upper, config, ignoring_case, true));
    assert!(!subject.git_dir_matches(upper, config, PATHNAME, true));
}

#[test]
fn gitdir_condition_reads_a_backslash_as_a_file_character_off_windows() {
    let git_dir = PathBuf::from(r"/work/repo\name/.git");
    let info = GitWorktreeInfo {
        repo_root: PathBuf::from(r"/work/repo\name"),
        git_dir: git_dir.clone(),
        git_common_dir: git_dir,
        is_bare: false,
        is_linked_worktree: false,
    };
    let subject = subject(&info);
    let config = Path::new(r"/work/repo\name/.git/config");
    assert!(subject.git_dir_matches(r"/work/repo\name/.git", config, PATHNAME, false));
    assert!(!subject.git_dir_matches("/work/repo/name/.git", config, PATHNAME, false));
}
