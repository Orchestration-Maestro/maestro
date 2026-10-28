//! Worktree names, checkout paths and the Git commands that add, remove and list worktrees.
#![cfg(test)]
use crate::repository_support::{init_repo_with_commit, run_git};
use maestro_daemon::git::worktree::{
    self, ExistingWorktree, WorktreeError, branch_to_path_slug, checkout_has_dirty_files,
    default_checkout_path, generated_branch_slug, local_branch_exists, worktrees_root,
};
use maestro_test_directory::Scratch;
use std::path::{Path, PathBuf};

/// A repository named `repo` on branch `main` with one committed file.
fn repository(scratch: &Scratch) -> PathBuf {
    let repo = scratch.path().join("repo");
    init_repo_with_commit(&repo);
    run_git(&repo, &["branch", "-M", "main"]);
    std::fs::write(repo.join("README.md"), "test\n").unwrap();
    run_git(&repo, &["add", "README.md"]);
    run_git(&repo, &["commit", "--quiet", "-m", "readme"]);
    repo
}

/// The trimmed standard output of one successful Git command.
fn git_stdout(cwd: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// A canonical path, so entries compare equal across symbolic links.
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap()
}

#[test]
fn generated_branch_names_are_namespaced_and_stable() {
    assert_eq!(generated_branch_slug(0), "worktree/brave-river-0000");
    assert_eq!(generated_branch_slug(9), "worktree/calm-cloud-0009");
    assert_eq!(
        generated_branch_slug(u64::MAX),
        "worktree/silver-valley-ffff"
    );
}

#[test]
fn branch_slug_makes_a_branch_safe_folder_name() {
    assert_eq!(
        branch_to_path_slug("worktree/brave-river"),
        "worktree-brave-river"
    );
    assert_eq!(
        branch_to_path_slug("issue/137 Worktree Spaces"),
        "issue-137-worktree-spaces"
    );
    assert_eq!(branch_to_path_slug("--Mixed__Case.v1--"), "mixed-case-v1");
    assert_eq!(branch_to_path_slug("feature/\u{c9}clair"), "feature-clair");
    assert_eq!(branch_to_path_slug("///"), "worktree");
    assert_eq!(branch_to_path_slug(""), "worktree");
}

#[test]
fn default_checkout_path_appends_repository_and_branch_folder() {
    assert_eq!(
        default_checkout_path(
            Path::new("/home/me/.maestro/worktrees"),
            "maestro",
            "worktree/brave-river"
        ),
        PathBuf::from("/home/me/.maestro/worktrees/maestro/worktree-brave-river")
    );
}

#[test]
fn worktrees_root_expands_a_leading_tilde_with_the_supplied_home() {
    let home = || Some(PathBuf::from("/home/me"));
    assert_eq!(
        worktrees_root("~/.maestro/worktrees", home),
        PathBuf::from("/home/me/.maestro/worktrees")
    );
    assert_eq!(worktrees_root("~", home), PathBuf::from("/home/me"));
}

#[test]
fn worktrees_root_keeps_an_absolute_directory_without_asking_for_home() {
    let root = worktrees_root("/srv/trees", || panic!("home was read"));
    assert_eq!(root, PathBuf::from("/srv/trees"));
}

#[cfg(not(windows))]
#[test]
fn worktrees_root_keeps_other_tilde_spellings_literal_below_the_current_directory() {
    let cwd = std::env::current_dir().unwrap();
    let home = || Some(PathBuf::from("/home/me"));
    for authored in ["~other/trees", r"~\trees", "~trees"] {
        assert_eq!(worktrees_root(authored, home), cwd.join(authored));
    }
}

#[test]
fn worktrees_root_without_a_home_keeps_the_tilde_below_the_current_directory() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(
        worktrees_root("~/trees", || None),
        cwd.join("~").join("trees")
    );
}

#[test]
fn worktrees_root_makes_a_relative_directory_absolute() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(worktrees_root("trees", || None), cwd.join("trees"));
    assert_eq!(worktrees_root("", || None), cwd);
}

#[test]
fn dirty_checkout_error_matches_the_hint_git_gives() {
    assert!(
        WorktreeError::new(
            "fatal: '/w/repo' contains modified or untracked files, use --force to delete it"
        )
        .is_dirty_checkout()
    );
    assert!(
        !WorktreeError::new("fatal: '/w/repo' is a missing but already registered worktree")
            .is_dirty_checkout()
    );
    assert!(
        !WorktreeError::new(
            "fatal: '/w/repo' contains a locked worktree, use --force only if you know why"
        )
        .is_dirty_checkout()
    );
    assert!(WorktreeError::dirty_checkout(Path::new("/w/repo")).is_dirty_checkout());
    assert!(
        WorktreeError::dirty_checkout(Path::new("/w/repo"))
            .message()
            .contains("/w/repo")
    );
}

#[test]
fn porcelain_listing_parses_branches_and_flags() {
    let output = b"\
worktree /repo/main
HEAD abc
branch refs/heads/main

worktree /repo/issue
HEAD def
branch refs/heads/worktree/issue

worktree /repo/detached
HEAD fed
detached
prunable gitdir file points to non-existent location

worktree /repo/bare.git
bare
";
    let entry = |path: &str, branch: Option<&str>, bare, detached, prunable| ExistingWorktree {
        path: PathBuf::from(path),
        branch: branch.map(str::to_owned),
        is_bare: bare,
        is_detached: detached,
        is_prunable: prunable,
    };
    assert_eq!(
        ExistingWorktree::parse_porcelain(output),
        vec![
            entry("/repo/main", Some("main"), false, false, false),
            entry("/repo/issue", Some("worktree/issue"), false, false, false),
            entry("/repo/detached", None, false, true, true),
            entry("/repo/bare.git", None, true, false, false),
        ]
    );
}

#[test]
fn porcelain_listing_ignores_a_stanza_without_a_path_and_a_missing_final_blank_line() {
    let output = b"HEAD abc\nbranch refs/heads/orphan\n\nworktree /repo/last\ndetached";
    let parsed = ExistingWorktree::parse_porcelain(output);
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].path, PathBuf::from("/repo/last"));
    assert_eq!(parsed[0].branch, None);
    assert!(parsed[0].is_detached);
}

#[test]
fn add_creates_a_new_branch_at_the_base() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let base = git_stdout(&repo, &["rev-parse", "HEAD"]);
    run_git(
        &repo,
        &["commit", "--quiet", "--allow-empty", "-m", "later"],
    );
    let checkout = scratch.path().join("trees/topic");

    worktree::add(&repo, &checkout, "feature/topic", &base).unwrap();

    assert!(checkout.join("README.md").exists());
    assert_eq!(
        git_stdout(&checkout, &["branch", "--show-current"]),
        "feature/topic"
    );
    assert_eq!(git_stdout(&checkout, &["rev-parse", "HEAD"]), base);
}

#[test]
fn add_checks_out_an_existing_branch_without_moving_it() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let first = git_stdout(&repo, &["rev-parse", "HEAD"]);
    run_git(
        &repo,
        &["commit", "--quiet", "--allow-empty", "-m", "later"],
    );
    run_git(&repo, &["branch", "topic"]);
    let tip = git_stdout(&repo, &["rev-parse", "topic"]);
    let checkout = scratch.path().join("trees/topic");

    worktree::add(&repo, &checkout, "topic", &first).unwrap();

    assert_eq!(
        git_stdout(&checkout, &["branch", "--show-current"]),
        "topic"
    );
    assert_eq!(git_stdout(&checkout, &["rev-parse", "HEAD"]), tip);
}

#[test]
fn add_reports_git_message_for_a_branch_checked_out_elsewhere() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let checkout = scratch.path().join("trees/main");

    let error = worktree::add(&repo, &checkout, "main", "HEAD").unwrap_err();

    assert!(error.message().contains("main"), "{error}");
    assert!(!checkout.exists());
}

#[test]
fn add_reports_git_message_for_an_unknown_base() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let checkout = scratch.path().join("trees/topic");

    let error = worktree::add(&repo, &checkout, "topic", "no-such-base").unwrap_err();

    assert!(error.message().contains("no-such-base"), "{error}");
    assert!(!checkout.exists());
}

#[test]
fn add_reads_a_base_that_looks_like_an_option_as_a_revision() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let checkout = scratch.path().join("trees/topic");

    let error = worktree::add(&repo, &checkout, "topic", "--no-checkout").unwrap_err();

    assert!(error.message().contains("--no-checkout"), "{error}");
    assert!(!checkout.exists());
    assert!(!local_branch_exists(&repo, "topic").unwrap());
}

#[test]
fn local_branch_lookup_separates_missing_from_failure() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    assert!(local_branch_exists(&repo, "main").unwrap());
    assert!(!local_branch_exists(&repo, "missing").unwrap());
    let elsewhere = scratch.path().join("not-a-repository");
    std::fs::create_dir(&elsewhere).unwrap();
    assert!(
        !local_branch_exists(&elsewhere, "main")
            .unwrap_err()
            .message()
            .is_empty()
    );
}

#[test]
fn remove_deletes_the_checkout_and_keeps_the_branch() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let checkout = scratch.path().join("trees/topic");
    worktree::add(&repo, &checkout, "topic", "HEAD").unwrap();

    worktree::remove(&repo, &checkout, false).unwrap();

    assert!(!checkout.exists());
    assert!(local_branch_exists(&repo, "topic").unwrap());
}

#[test]
fn remove_refuses_a_dirty_checkout_until_forced() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let checkout = scratch.path().join("trees/topic");
    worktree::add(&repo, &checkout, "topic", "HEAD").unwrap();
    std::fs::write(checkout.join("README.md"), "dirty\n").unwrap();

    let error = worktree::remove(&repo, &checkout, false).unwrap_err();
    assert!(error.is_dirty_checkout(), "{error}");
    assert!(checkout.join("README.md").exists());

    worktree::remove(&repo, &checkout, true).unwrap();
    assert!(!checkout.exists());
}

#[test]
fn remove_without_force_reports_a_checkout_git_does_not_list() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let missing = scratch.path().join("trees/missing");

    let error = worktree::remove(&repo, &missing, false).unwrap_err();

    assert!(error.message().contains("not a working tree"), "{error}");
}

/// Removes a checkout, then recreates its directory holding a link to a vanished admin folder.
fn leftover_checkout(scratch: &Scratch, repo: &Path) -> PathBuf {
    let checkout = scratch.path().join("trees/leftover");
    worktree::add(repo, &checkout, "leftover", "HEAD").unwrap();
    worktree::remove(repo, &checkout, true).unwrap();
    std::fs::create_dir_all(&checkout).unwrap();
    let admin = repo.join(".git/worktrees/stale");
    std::fs::write(
        checkout.join(".git"),
        format!("gitdir: {}\n", admin.display()),
    )
    .unwrap();
    std::fs::write(checkout.join("leftover"), "leftover\n").unwrap();
    checkout
}

#[test]
fn forced_remove_clears_a_leftover_checkout_of_the_repository() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let checkout = leftover_checkout(&scratch, &repo);

    let error = worktree::remove(&repo, &checkout, false).unwrap_err();
    assert!(error.message().contains("not a working tree"), "{error}");
    assert!(checkout.exists());

    worktree::remove(&repo, &checkout, true).unwrap();
    assert!(!checkout.exists());
}

#[cfg(unix)]
#[test]
fn forced_remove_clears_a_leftover_checkout_when_the_repository_is_reached_through_a_link() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let checkout = leftover_checkout(&scratch, &repo);
    let linked = scratch.path().join("linked-repo");
    std::os::unix::fs::symlink(&repo, &linked).unwrap();

    worktree::remove(&linked, &checkout, true).unwrap();

    assert!(!checkout.exists());
}

#[test]
fn forced_remove_keeps_an_unrelated_replacement_directory() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let checkout = scratch.path().join("trees/replaced");
    std::fs::create_dir_all(&checkout).unwrap();
    std::fs::write(checkout.join("unrelated"), "do not delete\n").unwrap();

    let error = worktree::remove(&repo, &checkout, true).unwrap_err();

    assert!(error.message().contains("not a working tree"), "{error}");
    assert!(checkout.join("unrelated").exists());
}

#[test]
fn forced_remove_keeps_a_directory_linked_to_another_repository() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let checkout = leftover_checkout(&scratch, &repo);
    let admin = scratch.path().join("other/.git/worktrees/stale");
    std::fs::write(
        checkout.join(".git"),
        format!("gitdir: {}\n", admin.display()),
    )
    .unwrap();

    assert!(worktree::remove(&repo, &checkout, true).is_err());
    assert!(checkout.join("leftover").exists());
}

#[test]
fn forced_remove_of_a_checkout_that_is_already_gone_succeeds() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let missing = scratch.path().join("trees/missing");

    worktree::remove(&repo, &missing, true).unwrap();

    assert!(!missing.exists());
}

#[test]
fn dirty_detection_reports_clean_modified_and_untracked_checkouts() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let checkout = scratch.path().join("trees/topic");
    worktree::add(&repo, &checkout, "topic", "HEAD").unwrap();
    assert!(!checkout_has_dirty_files(&checkout).unwrap());

    std::fs::create_dir(checkout.join("nested")).unwrap();
    std::fs::write(checkout.join("nested/new.txt"), "new\n").unwrap();
    assert!(checkout_has_dirty_files(&checkout).unwrap());
    std::fs::remove_dir_all(checkout.join("nested")).unwrap();

    std::fs::write(checkout.join("README.md"), "dirty\n").unwrap();
    assert!(checkout_has_dirty_files(&checkout).unwrap());
}

#[test]
fn dirty_detection_reports_git_message_outside_a_repository() {
    let scratch = Scratch::new().unwrap();
    let error = checkout_has_dirty_files(scratch.path()).unwrap_err();
    assert!(!error.message().is_empty());
}

#[test]
fn list_reports_the_main_and_linked_worktrees_with_their_branches() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let checkout = scratch.path().join("trees/topic");
    worktree::add(&repo, &checkout, "feature/topic", "HEAD").unwrap();

    let entries = worktree::list(&repo).unwrap();

    let found: Vec<_> = entries
        .iter()
        .map(|entry| (canonical(&entry.path), entry.branch.as_deref()))
        .collect();
    assert_eq!(
        found,
        vec![
            (canonical(&repo), Some("main")),
            (canonical(&checkout), Some("feature/topic")),
        ]
    );
    assert!(
        entries
            .iter()
            .all(|entry| { !entry.is_bare && !entry.is_detached && !entry.is_prunable })
    );
}

#[test]
fn list_marks_detached_and_prunable_worktrees() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let detached = scratch.path().join("trees/detached");
    let gone = scratch.path().join("trees/gone");
    run_git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            detached.to_str().unwrap(),
        ],
    );
    worktree::add(&repo, &gone, "gone", "HEAD").unwrap();
    let gone_key = canonical(&gone);
    std::fs::remove_dir_all(&gone).unwrap();

    let entries = worktree::list(&repo).unwrap();

    let detached_entry = entries
        .iter()
        .find(|entry| canonical(&entry.path) == canonical(&detached))
        .unwrap();
    assert!(detached_entry.is_detached && detached_entry.branch.is_none());
    let gone_entry = entries.iter().find(|entry| entry.path == gone_key).unwrap();
    assert!(gone_entry.is_prunable);
}

#[test]
fn list_marks_a_bare_repository() {
    let scratch = Scratch::new().unwrap();
    let bare = scratch.path().join("bare.git");
    run_git(
        scratch.path(),
        &["init", "--quiet", "--bare", bare.to_str().unwrap()],
    );

    let entries = worktree::list(&bare).unwrap();

    assert_eq!(entries.len(), 1);
    assert!(entries[0].is_bare);
    assert_eq!(canonical(&entries[0].path), canonical(&bare));
}

#[cfg(unix)]
#[test]
fn list_keeps_the_bytes_of_a_checkout_path_that_text_cannot_hold() {
    use std::os::unix::ffi::OsStrExt;
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let checkout = scratch
        .path()
        .join(<std::ffi::OsStr as OsStrExt>::from_bytes(b"trees-\xff"));
    worktree::add(&repo, &checkout, "bytes", "HEAD").unwrap();

    let entries = worktree::list(&repo).unwrap();

    assert!(
        entries
            .iter()
            .any(|entry| canonical(&entry.path) == canonical(&checkout))
    );
}

#[test]
fn list_reports_git_message_outside_a_repository() {
    let scratch = Scratch::new().unwrap();
    let error = worktree::list(scratch.path()).unwrap_err();
    assert!(!error.message().is_empty());
}
