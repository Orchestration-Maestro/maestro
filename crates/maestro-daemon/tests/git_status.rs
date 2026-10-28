//! Status snapshots, their cache keys and the fingerprints that keep a cached one.
#![cfg(test)]
use crate::repository_support::{init_repo_with_commit, run_git};
use maestro_daemon::git::{
    discovery::{git_rev_parse_verify, git_space_metadata},
    status::{
        GitHeadIdentity, GitStatusCacheEntry, git_status_cache_key, git_status_cache_key_for_space,
        git_status_snapshot_for_cwd_with_demand,
    },
};
use maestro_workspace::workspace::{
    GitStatusRefreshDemand, WorkspaceGitStatusSnapshot, git::config::ConfigEnvironment,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

/// Both observations demanded.
const ALL: GitStatusRefreshDemand = GitStatusRefreshDemand {
    branch: true,
    ahead_behind: true,
};

/// Observe with both fields demanded and no user configuration.
fn snapshot(
    cwd: &Path,
    cached: Option<&GitStatusCacheEntry>,
) -> (WorkspaceGitStatusSnapshot, Option<GitStatusCacheEntry>) {
    git_status_snapshot_for_cwd_with_demand(cwd, cached, ALL, &ConfigEnvironment::default())
}

/// A repository whose branch `main` tracks `origin/main`, written without running Git.
fn write_tracked_repository(root: &Path) {
    std::fs::create_dir_all(root.join(".git/refs/heads")).unwrap();
    std::fs::create_dir_all(root.join(".git/refs/remotes/origin")).unwrap();
    std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    std::fs::write(
        root.join(".git/refs/heads/main"),
        format!("{}\n", "1".repeat(40)),
    )
    .unwrap();
    std::fs::write(
        root.join(".git/refs/remotes/origin/main"),
        format!("{}\n", "2".repeat(40)),
    )
    .unwrap();
    std::fs::write(
        root.join(".git/config"),
        "[branch \"main\"]\n\tremote = origin\n\tmerge = refs/heads/main\n",
    )
    .unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn space_cache_key_keeps_a_non_utf8_checkout_path() {
    use std::os::unix::ffi::OsStringExt;
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory
        .path()
        .join(std::ffi::OsString::from_vec(b"repo\x80".to_vec()));
    write_tracked_repository(&root);
    let space = git_space_metadata(&root).unwrap();

    assert_eq!(
        git_status_cache_key_for_space(&space),
        std::fs::canonicalize(&root).unwrap()
    );
}

#[test]
fn cache_key_is_absent_below_an_invalid_git_marker() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let cwd = directory.path().join("workspace");
    let valid = directory.path().join("valid");
    std::fs::create_dir_all(directory.path().join(".git")).unwrap();
    std::fs::create_dir_all(&cwd).unwrap();
    write_tracked_repository(&valid);

    assert_eq!(git_status_cache_key(&cwd), None);
    assert_eq!(
        git_status_cache_key(&valid),
        Some(std::fs::canonicalize(&valid).unwrap())
    );
}

#[test]
fn cache_keys_differ_per_linked_checkout() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let common = directory.path().join("repo/.git");
    let mut keys = Vec::new();
    for name in ["one", "two"] {
        let git_dir = common.join("worktrees").join(name);
        let checkout = directory.path().join(name);
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::create_dir_all(&checkout).unwrap();
        std::fs::write(
            checkout.join(".git"),
            format!("gitdir: {}\n", git_dir.display()),
        )
        .unwrap();
        std::fs::write(git_dir.join("HEAD"), format!("ref: refs/heads/{name}\n")).unwrap();
        keys.push(git_status_cache_key(&checkout));
    }

    assert!(keys.iter().all(Option::is_some));
    assert_ne!(keys[0], keys[1]);
}

#[test]
fn cached_non_repository_result_is_reused_until_its_retry_time() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let cwd = directory.path().join("deep/nested");
    std::fs::create_dir_all(&cwd).unwrap();
    let (initial, entry) = snapshot(&cwd, None);
    let entry = entry.unwrap();
    assert!(entry.fingerprint.is_none());
    std::fs::remove_dir_all(directory.path().join("deep")).unwrap();

    let (cached, update) = snapshot(&cwd, Some(&entry));

    assert_eq!(cached, initial);
    let update = update.unwrap();
    assert_eq!(update.snapshot, entry.snapshot);
    assert_eq!(update.retry_after, entry.retry_after);
    assert!(initial.space.is_none());
    assert_eq!(initial.auto_label, "nested");
}

#[test]
fn expired_non_repository_result_finds_a_repository_created_in_place() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    let (_, entry) = snapshot(&root, None);
    let mut entry = entry.unwrap();
    entry.retry_after = Instant::now().checked_sub(Duration::from_secs(1));
    write_tracked_repository(&root);

    let (found, update) = snapshot(&root, Some(&entry));

    assert_eq!(found.branch.as_deref(), Some("main"));
    assert!(update.is_some_and(|entry| entry.fingerprint.is_some()));
}

#[test]
fn repository_identity_clears_when_head_disappears() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    write_tracked_repository(&root);
    let (before, cached) = snapshot(&root, None);
    assert!(before.space.is_some());
    std::fs::remove_file(root.join(".git/HEAD")).unwrap();

    let (cleared, _) = snapshot(&root, cached.as_ref());

    assert_eq!(cleared.space, None);
}

#[test]
fn branch_only_demand_skips_ahead_behind() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    write_tracked_repository(&root);

    let (found, update) = git_status_snapshot_for_cwd_with_demand(
        &root,
        None,
        GitStatusRefreshDemand {
            branch: true,
            ahead_behind: false,
        },
        &ConfigEnvironment::default(),
    );

    assert_eq!(found.branch.as_deref(), Some("main"));
    assert_eq!(found.ahead_behind, None);
    let fingerprint = update.unwrap().fingerprint.unwrap();
    assert_eq!(fingerprint.upstream, None);
}

#[test]
fn matching_fingerprint_reuses_cached_ahead_behind() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    write_tracked_repository(&root);
    let (_, entry) = snapshot(&root, None);
    let mut entry = entry.unwrap();
    entry.snapshot.ahead_behind = Some((2, 1));

    let (reused, update) = snapshot(&root, Some(&entry));

    assert_eq!(reused.branch.as_deref(), Some("main"));
    assert_eq!(reused.ahead_behind, Some((2, 1)));
    assert_eq!(update.unwrap().snapshot.ahead_behind, Some((2, 1)));
}

#[test]
fn switching_branch_discards_cached_ahead_behind() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    write_tracked_repository(&root);
    let (_, entry) = snapshot(&root, None);
    let mut entry = entry.unwrap();
    entry.snapshot.ahead_behind = Some((4, 0));
    std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/feature\n").unwrap();
    std::fs::write(
        root.join(".git/refs/heads/feature"),
        format!("{}\n", "1".repeat(40)),
    )
    .unwrap();
    std::fs::write(
        root.join(".git/config"),
        "[branch \"feature\"]\n\tremote = origin\n\tmerge = refs/heads/main\n",
    )
    .unwrap();

    let (switched, _) = snapshot(&root, Some(&entry));

    assert_eq!(switched.branch.as_deref(), Some("feature"));
    assert_eq!(switched.ahead_behind, None);
}

#[test]
fn unsetting_upstream_clears_ahead_behind() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    write_tracked_repository(&root);
    let (_, entry) = snapshot(&root, None);
    let mut entry = entry.unwrap();
    entry.snapshot.ahead_behind = Some((0, 3));
    std::fs::write(root.join(".git/config"), "").unwrap();

    let (cleared, _) = snapshot(&root, Some(&entry));

    assert_eq!(cleared.branch.as_deref(), Some("main"));
    assert_eq!(cleared.ahead_behind, None);
}

#[test]
fn new_include_file_changes_the_upstream_remote() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    write_tracked_repository(&root);
    std::fs::write(
        root.join(".git/config"),
        "[branch \"main\"]\n\tremote = origin\n\tmerge = refs/heads/main\n[include]\n\tpath = branch.cfg\n",
    )
    .unwrap();
    let (_, entry) = snapshot(&root, None);
    std::fs::write(
        root.join(".git/branch.cfg"),
        "[branch \"main\"]\n\tremote = fork\n\tmerge = refs/heads/main\n",
    )
    .unwrap();

    let (_, update) = snapshot(&root, entry.as_ref());

    let upstream = update.unwrap().fingerprint.unwrap().upstream.unwrap();
    assert_eq!(upstream.remote, "fork");
}

#[test]
fn unchanged_configuration_files_are_not_read_again() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    write_tracked_repository(&root);
    let config = root.join(".git/config");
    let (_, entry) = snapshot(&root, None);
    let stamp = std::fs::metadata(&config).unwrap().modified().unwrap();
    std::fs::write(
        &config,
        "[branch \"main\"]\n\tremote = mirror\n\tmerge = refs/heads/main\n",
    )
    .unwrap();
    std::fs::File::options()
        .write(true)
        .open(&config)
        .unwrap()
        .set_modified(stamp)
        .unwrap();

    let (_, update) = snapshot(&root, entry.as_ref());

    let upstream = update.unwrap().fingerprint.unwrap().upstream.unwrap();
    assert_eq!(upstream.remote, "origin");
}

#[test]
fn upstream_oid_comes_from_packed_refs() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    write_tracked_repository(&root);
    std::fs::remove_file(root.join(".git/refs/remotes/origin/main")).unwrap();
    std::fs::write(
        root.join(".git/packed-refs"),
        format!(
            "# pack-refs with: peeled fully-peeled sorted\n{} refs/remotes/origin/main\n",
            "2".repeat(40)
        ),
    )
    .unwrap();

    let (_, entry) = snapshot(&root, None);

    let upstream = entry.unwrap().fingerprint.unwrap().upstream.unwrap();
    assert_eq!(upstream.oid, Some("2".repeat(40)));
    assert_eq!(upstream.full_ref, "refs/remotes/origin/main");
}

#[test]
fn packed_refs_search_skips_a_line_it_cannot_read() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    write_tracked_repository(&root);
    std::fs::remove_file(root.join(".git/refs/remotes/origin/main")).unwrap();
    std::fs::write(
        root.join(".git/packed-refs"),
        format!(
            "# pack-refs with: peeled fully-peeled sorted\nmalformed\n{} refs/remotes/origin/main\n",
            "2".repeat(40)
        ),
    )
    .unwrap();

    let (_, entry) = snapshot(&root, None);

    let upstream = entry.unwrap().fingerprint.unwrap().upstream.unwrap();
    assert_eq!(upstream.oid, Some("2".repeat(40)));
}

#[test]
fn detached_head_identity_is_the_commit() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    write_tracked_repository(&root);
    std::fs::write(root.join(".git/HEAD"), format!("{}\n", "3".repeat(40))).unwrap();

    let (detached, entry) = snapshot(&root, None);

    assert_eq!(detached.branch, None);
    assert_eq!(
        entry.unwrap().fingerprint.unwrap().head,
        GitHeadIdentity::Detached {
            oid: "3".repeat(40)
        }
    );
}

#[test]
fn linked_checkout_label_names_the_checkout() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let repo = directory.path().join("maestro");
    let checkout = directory.path().join("linked");
    init_repo_with_commit(&repo);
    run_git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "linked",
            checkout.to_str().unwrap(),
            "HEAD",
        ],
    );

    let (found, _) = snapshot(&checkout, None);

    assert_eq!(found.auto_label, "linked");
}

#[test]
fn reftable_head_identity_is_read_through_git() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    let reftable = std::process::Command::new("git")
        .args(["init", "--ref-format=reftable", "-b", "main"])
        .arg(&root)
        .output()
        .unwrap();
    if reftable.status.success() {
        run_git(&root, &["config", "user.email", "maestro@example.invalid"]);
        run_git(&root, &["config", "user.name", "Maestro Test"]);
        run_git(&root, &["commit", "--allow-empty", "-m", "initial"]);
    } else {
        // A version-0 repository ignores the extension, so Git still answers from files.
        init_repo_with_commit(&root);
        run_git(&root, &["branch", "-M", "main"]);
        let config = std::fs::read_to_string(root.join(".git/config")).unwrap();
        std::fs::write(
            root.join(".git/config"),
            format!("{config}[extensions]\n\trefStorage = reftable\n"),
        )
        .unwrap();
    }

    let (found, entry) = snapshot(&root, None);

    assert_eq!(found.branch.as_deref(), Some("main"));
    assert_eq!(
        entry.unwrap().fingerprint.unwrap().head,
        GitHeadIdentity::Branch {
            full_ref: "refs/heads/main".into(),
            short_name: "main".into(),
            oid: git_rev_parse_verify(&root, "HEAD"),
        }
    );
}

#[test]
fn new_commit_recomputes_ahead_behind() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let remote = directory.path().join("remote.git");
    let repo = directory.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    run_git(
        directory.path(),
        &["init", "--bare", remote.to_str().unwrap()],
    );
    init_repo_with_commit(&repo);
    run_git(&repo, &["branch", "-M", "main"]);
    run_git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    run_git(&repo, &["push", "-u", "origin", "main"]);
    let (initial, entry) = snapshot(&repo, None);
    assert_eq!(initial.ahead_behind, Some((0, 0)));
    run_git(&repo, &["commit", "--allow-empty", "-m", "ahead"]);

    let (updated, _) = snapshot(&repo, entry.as_ref());

    assert_eq!(updated.branch.as_deref(), Some("main"));
    assert_eq!(updated.ahead_behind, Some((1, 0)));
}
