//! Native repository identity, labels and Git fallback observations.
#![cfg(test)]
use crate::repository_support::{init_repo_with_commit, run_git};
use maestro_daemon::git::discovery::{
    automatic_workspace_label, derive_label_from_cwd, git_branch, git_repo_root,
    git_rev_parse_verify, git_space_metadata, git_worktree_info,
};
use std::path::{Path, PathBuf};

#[test]
fn repository_branch_reads_head_from_standard_repo() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();

    assert_eq!(git_branch(&root).as_deref(), Some("main"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn repository_branch_reads_head_from_worktree_gitdir_file() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    let worktree_git_dir = root.join(".bare/worktrees/feature");
    std::fs::create_dir_all(&worktree_git_dir).unwrap();
    std::fs::write(root.join(".git"), "gitdir: .bare/worktrees/feature\n").unwrap();
    std::fs::write(worktree_git_dir.join("HEAD"), "ref: refs/heads/feature\n").unwrap();

    assert_eq!(git_branch(&root).as_deref(), Some("feature"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn repository_branch_returns_none_for_detached_head() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/HEAD"), "3e1b9a8d\n").unwrap();

    assert_eq!(git_branch(&root), None);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn repository_branch_reads_symbolic_head_from_reftable_repo() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    let root_arg = root.to_string_lossy().to_string();
    let output = std::process::Command::new("git")
        .args(["init", "--ref-format=reftable", "-b", "main", &root_arg])
        .output()
        .unwrap();
    if !output.status.success() {
        std::fs::remove_dir_all(root).unwrap();
        return;
    }

    assert_eq!(git_branch(&root).as_deref(), Some("main"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn repository_root_ignores_invalid_git_marker() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let scratch = directory.path().to_path_buf();
    let cwd = scratch.join("workspace");
    std::fs::create_dir_all(scratch.join(".git")).unwrap();
    std::fs::create_dir_all(&cwd).unwrap();

    assert_eq!(git_repo_root(&cwd), None);

    std::fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn repository_root_ignores_standalone_non_bare_git_dir_layout() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    std::fs::write(root.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    std::fs::create_dir_all(root.join("objects")).unwrap();
    std::fs::create_dir_all(root.join("refs")).unwrap();
    std::fs::write(root.join("config"), "[core]\n\tbare = false\n").unwrap();

    assert_eq!(git_repo_root(&root.join("refs")), None);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn daemon_git_space_metadata_supports_standalone_bare_repo() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let bare = directory.path().to_path_buf();
    run_git(&bare, &["init", "--bare", "."]);
    let nested = bare.join("refs");

    let info = git_worktree_info(&nested).expect("bare repo should be discovered");
    assert!(info.is_bare);
    assert!(!info.is_linked_worktree);
    assert_eq!(info.git_dir, canonicalize_best_effort_path(&bare));

    let metadata = git_space_metadata(&nested).expect("bare repo should map to a git space");
    assert_eq!(
        canonicalize_best_effort_path(&metadata.repo_root),
        canonicalize_best_effort_path(&bare)
    );
    assert!(!metadata.is_linked_worktree);

    std::fs::remove_dir_all(bare).unwrap();
}

#[test]
fn daemon_bare_source_and_linked_checkout_share_repo_name_but_not_auto_label() {
    let (scratch, bare, checkout) = create_bare_repo_with_linked_worktree();

    let bare_space = git_space_metadata(&bare).unwrap();
    let checkout_space = git_space_metadata(&checkout).unwrap();
    let bare_auto_label = automatic_workspace_label(&bare, &bare_space.repo_root, None);
    let checkout_auto_label = automatic_workspace_label(&checkout, &checkout_space.repo_root, None);

    assert_eq!(bare_space.key, checkout_space.key);
    assert_eq!(bare_space.repo_name, ".bare");
    assert_eq!(checkout_space.repo_name, bare_space.repo_name);
    assert_eq!(bare_auto_label, bare.file_name().unwrap().to_str().unwrap());
    assert_eq!(
        checkout_auto_label,
        checkout.file_name().unwrap().to_str().unwrap()
    );

    std::fs::remove_dir_all(scratch.path()).unwrap();
}

#[test]
fn daemon_embedded_dot_bare_source_and_checkout_use_container_repo_name() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let scratch = directory.path().to_path_buf();
    let seed = scratch.join("seed");
    let repo = scratch.join("reported-repo");
    let bare = repo.join(".bare");
    let checkout = repo.join("develop");
    std::fs::create_dir_all(&seed).unwrap();
    std::fs::create_dir_all(&repo).unwrap();
    run_git(&seed, &["init", "--quiet"]);
    run_git(&seed, &["config", "user.email", "maestro@example.invalid"]);
    run_git(&seed, &["config", "user.name", "Maestro Test"]);
    run_git(
        &seed,
        &["commit", "--quiet", "--allow-empty", "-m", "initial"],
    );
    run_git(
        &scratch,
        &[
            "clone",
            "--quiet",
            "--bare",
            seed.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    std::fs::write(repo.join(".git"), "gitdir: ./.bare\n").unwrap();
    run_git(
        &bare,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "develop",
            checkout.to_str().unwrap(),
            "HEAD",
        ],
    );

    let source = git_space_metadata(&repo).unwrap();
    let linked = git_space_metadata(&checkout).unwrap();

    assert_eq!(source.repo_name, "reported-repo");
    assert_eq!(linked.repo_name, source.repo_name);

    std::fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn daemon_git_space_metadata_marks_bare_dot_git_repo() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    run_git(&root, &["init", "--bare", ".git"]);

    let info = git_worktree_info(&root).expect("bare .git repo should be discovered");
    assert!(info.is_bare);
    assert!(!info.is_linked_worktree);
    assert_eq!(
        info.git_dir,
        canonicalize_best_effort_path(&root.join(".git"))
    );

    let metadata = git_space_metadata(&root).expect("bare .git repo should map to a git space");
    assert_eq!(
        canonicalize_best_effort_path(&metadata.repo_root),
        canonicalize_best_effort_path(&root)
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn daemon_derive_label_prefers_repo_root_name() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    let nested = root.join("nested");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    std::fs::create_dir_all(&nested).unwrap();

    assert_eq!(
        derive_label_from_cwd(&nested, None),
        root.file_name().and_then(|name| name.to_str()).unwrap()
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn daemon_derive_label_uses_path_name_outside_git() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    let label = root.file_name().and_then(|name| name.to_str()).unwrap();

    assert_eq!(derive_label_from_cwd(Path::new(&root), None), label);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn daemon_git_rev_parse_verify_reads_reftable_refs() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    let root_arg = root.to_string_lossy().to_string();
    let output = std::process::Command::new("git")
        .args(["init", "--ref-format=reftable", "-b", "main", &root_arg])
        .output()
        .unwrap();
    if !output.status.success() {
        std::fs::remove_dir_all(root).unwrap();
        return;
    }

    run_git(&root, &["config", "user.email", "maestro@example.invalid"]);
    run_git(&root, &["config", "user.name", "Maestro Test"]);
    run_git(&root, &["commit", "--allow-empty", "-m", "initial"]);

    let head_oid = git_rev_parse_verify(&root, "HEAD").unwrap();

    assert_eq!(
        git_rev_parse_verify(&root, "refs/heads/main").as_deref(),
        Some(head_oid.as_str())
    );

    std::fs::remove_dir_all(root).unwrap();
}

fn create_bare_repo_with_linked_worktree() -> (maestro_test_directory::Scratch, PathBuf, PathBuf) {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let scratch = directory.path().to_path_buf();
    let seed = scratch.join("seed");
    let bare = scratch.join(".bare");
    let checkout = scratch.join("feature");
    init_repo_with_commit(&seed);
    run_git(
        &scratch,
        &[
            "clone",
            "--quiet",
            "--bare",
            seed.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    run_git(
        &bare,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "feature",
            checkout.to_str().unwrap(),
            "HEAD",
        ],
    );
    (directory, bare, checkout)
}

/// Canonical native path with the source fallback on missing paths.
fn canonicalize_best_effort_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[test]
fn repository_native_links_keep_unicode_trim_and_climb_past_missing_head() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let scratch = directory.path().to_path_buf();
    let outer = scratch.join("outer");
    let linked = outer.join("linked");
    std::fs::create_dir_all(outer.join(".git")).unwrap();
    std::fs::write(outer.join(".git/HEAD"), "ref: refs/heads/outer\n").unwrap();
    std::fs::create_dir_all(&linked).unwrap();
    std::fs::write(
        linked.join(".git"),
        "\u{feff}gitdir: \u{a0}../../metadata\u{2028}",
    )
    .unwrap();
    assert_eq!(
        git_repo_root(&linked),
        Some(outer),
        "an incomplete nested repository must not hide the outer repository"
    );
    std::fs::create_dir(scratch.join("metadata")).unwrap();
    std::fs::write(
        scratch.join("metadata/HEAD"),
        "\u{feff}ref: refs/heads/topic\u{2028}",
    )
    .unwrap();
    assert_eq!(git_branch(&linked).as_deref(), Some("topic"));
    assert_eq!(git_repo_root(&linked), Some(linked.clone()));
    std::fs::create_dir(scratch.join("metadata/commondir")).unwrap();
    assert_eq!(
        git_repo_root(&linked),
        Some(linked.clone()),
        "unreadable shared metadata retains the checkout's own metadata"
    );
    assert_eq!(git_branch(&linked).as_deref(), Some("topic"));
    std::fs::remove_dir_all(scratch).unwrap();
}

#[cfg(unix)]
#[test]
fn repository_reftable_queries_use_native_process_owner() {
    use std::os::unix::fs::PermissionsExt;
    const NAME: &str = "repository_reftable_queries_use_native_process_owner";
    if let Some(root) = std::env::var_os("MAESTRO_REFTABLE_PROBE") {
        let root = PathBuf::from(root);
        assert_eq!(git_branch(&root).as_deref(), Some("topic"));
        assert_eq!(
            git_rev_parse_verify(&root, "HEAD").as_deref(),
            Some("0123456789abcdef")
        );
        assert_eq!(git_rev_parse_verify(&root, "missing"), None);
        return;
    }
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    std::fs::create_dir(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/.invalid\n").unwrap();
    std::fs::write(
        root.join(".git/config"),
        "[extensions]\nrefStorage = RefTable # selected storage\n",
    )
    .unwrap();
    std::fs::create_dir(root.join("bin")).unwrap();
    let executable = root.join("bin/git");
    std::fs::write(
        &executable,
        r#"#!/bin/sh
[ "$1" = -C ] && [ "$2" = "$MAESTRO_REFTABLE_PROBE" ] || exit 2
shift 2
case "$*" in
  'symbolic-ref --quiet --short HEAD') printf ' topic\n';;
  'rev-parse --verify HEAD') printf ' 0123456789abcdef\n';;
  'rev-parse --verify missing') printf 'discard this output\n'; exit 1;;
  *) exit 2;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let output = maestro_test_process::TestProcess::current(NAME)
        .unwrap()
        .env("MAESTRO_REFTABLE_PROBE", &root)
        .env("PATH", root.join("bin"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn repository_discovery_preserves_non_utf8_ancestors_and_relative_links() {
    use maestro_workspace::workspace::git::native::discover_git_paths;
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let scratch = directory.path().to_path_buf();
    let root = scratch.join(OsStr::from_bytes(b"checkout-\xff"));
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    let child = root.join("child");
    std::fs::create_dir(&child).unwrap();
    assert_eq!(discover_git_paths(&child).unwrap().repo_root, root);
    let alias = scratch.join("alias");
    std::os::unix::fs::symlink(&root, &alias).unwrap();
    assert_eq!(
        discover_git_paths(&alias.join("child")).unwrap().repo_root,
        alias
    );
    std::fs::write(child.join(".git"), "gitdir: ../.git\n").unwrap();
    assert_eq!(
        discover_git_paths(&alias.join("child")).unwrap().git_dir,
        alias.join(".git")
    );
    let metadata = scratch.join(OsStr::from_bytes(b"metadata-\xfe"));
    std::fs::create_dir(&metadata).unwrap();
    std::fs::write(metadata.join("HEAD"), "ref: refs/heads/topic\n").unwrap();
    std::fs::write(child.join(".git"), b"gitdir: ../../metadata-\xfe\n").unwrap();
    let discovered = discover_git_paths(&child).unwrap();
    assert_eq!(discovered.repo_root, child);
    assert_eq!(std::fs::canonicalize(discovered.git_dir).unwrap(), metadata);
    std::fs::remove_dir_all(scratch).unwrap();
}

/// Builds one valid enclosing checkout with a nested candidate.
#[cfg(unix)]
fn permission_discovery_fixture() -> (maestro_test_directory::Scratch, PathBuf, PathBuf) {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let outer = directory.path().join("outer");
    let inner = outer.join("inner");
    std::fs::create_dir_all(outer.join(".git")).unwrap();
    std::fs::write(outer.join(".git/HEAD"), "ref: refs/heads/outer\n").unwrap();
    std::fs::create_dir(&inner).unwrap();
    (directory, outer, inner)
}

#[cfg(unix)]
#[test]
fn unreadable_git_marker_keeps_workspace_ascent_and_footer_stop() {
    use maestro_workspace::workspace::git::native::{DiscoveryPolicy, discover_git_paths_with};
    let (_directory, outer, inner) = permission_discovery_fixture();
    let marker = inner.join(".git");
    std::fs::write(&marker, "gitdir: ../.git\n").unwrap();
    // A privileged run reads the marker anyway, so there is nothing to check.
    let Some(restore) = maestro_test_directory::deny(&marker, 0o0).unwrap() else {
        return;
    };
    let workspace = discover_git_paths_with(&inner, DiscoveryPolicy::Workspace);
    let footer = discover_git_paths_with(&inner, DiscoveryPolicy::Footer);
    drop(restore);
    assert_eq!(workspace.unwrap().repo_root, outer);
    assert!(footer.is_none());
}

#[cfg(unix)]
#[test]
fn inaccessible_git_marker_stat_climbs_for_both_callers() {
    use maestro_workspace::workspace::git::native::{DiscoveryPolicy, discover_git_paths_with};
    let (_directory, outer, inner) = permission_discovery_fixture();
    // A privileged run looks inside anyway, so there is nothing to check.
    let Some(restore) = maestro_test_directory::deny(&inner, 0o0).unwrap() else {
        return;
    };
    let workspace = discover_git_paths_with(&inner, DiscoveryPolicy::Workspace);
    let footer = discover_git_paths_with(&inner, DiscoveryPolicy::Footer);
    drop(restore);
    assert_eq!(workspace.unwrap().repo_root, outer);
    assert_eq!(footer.unwrap().repo_root, outer);
}

#[cfg(unix)]
#[test]
fn inaccessible_optional_common_metadata_falls_back_for_both_callers() {
    use maestro_workspace::workspace::git::native::{DiscoveryPolicy, discover_git_paths_with};
    use std::os::unix::fs::symlink;
    let (directory, _outer, inner) = permission_discovery_fixture();
    let store = directory.path().join("store");
    let locked = directory.path().join("locked");
    std::fs::create_dir(&store).unwrap();
    std::fs::create_dir(&locked).unwrap();
    std::fs::write(store.join("HEAD"), "ref: refs/heads/topic\n").unwrap();
    std::fs::write(locked.join("common"), "..\n").unwrap();
    symlink(locked.join("common"), store.join("commondir")).unwrap();
    std::fs::write(inner.join(".git"), format!("gitdir: {}\n", store.display())).unwrap();
    // A privileged run follows the link anyway, so there is nothing to check.
    let Some(restore) = maestro_test_directory::deny(&locked, 0o0).unwrap() else {
        return;
    };
    let workspace = discover_git_paths_with(&inner, DiscoveryPolicy::Workspace);
    let footer = discover_git_paths_with(&inner, DiscoveryPolicy::Footer);
    drop(restore);
    assert_eq!(workspace.unwrap().git_common_dir, store);
    assert_eq!(footer.unwrap().git_common_dir, store);
}
