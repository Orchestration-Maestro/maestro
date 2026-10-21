//! Native repository identity, labels and Git fallback observations.
#![cfg(test)]
#[path = "repository_support/mod.rs"]
mod support;
use maestro_daemon::git::discovery::{
    automatic_workspace_label, derive_label_from_cwd, git_branch, git_repo_root,
    git_rev_parse_verify, git_space_metadata, git_worktree_info,
};
use std::path::{Path, PathBuf};
use support::{run_git, temp_test_dir};

#[test]
fn repository_branch_reads_head_from_standard_repo() {
    let root = temp_test_dir("standard-repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();

    assert_eq!(git_branch(&root).as_deref(), Some("main"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn repository_branch_reads_head_from_worktree_gitdir_file() {
    let root = temp_test_dir("worktree");
    let worktree_git_dir = root.join(".bare/worktrees/feature");
    std::fs::create_dir_all(&worktree_git_dir).unwrap();
    std::fs::write(root.join(".git"), "gitdir: .bare/worktrees/feature\n").unwrap();
    std::fs::write(worktree_git_dir.join("HEAD"), "ref: refs/heads/feature\n").unwrap();

    assert_eq!(git_branch(&root).as_deref(), Some("feature"));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn repository_branch_returns_none_for_detached_head() {
    let root = temp_test_dir("detached-head");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/HEAD"), "3e1b9a8d\n").unwrap();

    assert_eq!(git_branch(&root), None);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn repository_branch_reads_symbolic_head_from_reftable_repo() {
    let root = temp_test_dir("reftable-branch");
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
    let scratch = temp_test_dir("invalid-git-root");
    let cwd = scratch.join("workspace");
    std::fs::create_dir_all(scratch.join(".git")).unwrap();
    std::fs::create_dir_all(&cwd).unwrap();

    assert_eq!(git_repo_root(&cwd), None);

    std::fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn repository_root_ignores_standalone_non_bare_git_dir_layout() {
    let root = temp_test_dir("standalone-non-bare-git-dir");
    std::fs::write(root.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    std::fs::create_dir_all(root.join("objects")).unwrap();
    std::fs::create_dir_all(root.join("refs")).unwrap();
    std::fs::write(root.join("config"), "[core]\n\tbare = false\n").unwrap();

    assert_eq!(git_repo_root(&root.join("refs")), None);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn daemon_git_space_metadata_supports_standalone_bare_repo() {
    let bare = temp_test_dir("bare-space");
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
    let (scratch, bare, checkout) = create_bare_repo_with_linked_worktree("bare-linked-labels");

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

    std::fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn daemon_embedded_dot_bare_source_and_checkout_use_container_repo_name() {
    let scratch = temp_test_dir("embedded-dot-bare");
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
    let root = temp_test_dir("bare-dot-git");
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
    let root = temp_test_dir("label-repo");
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
    let root = temp_test_dir("label-plain");
    let label = root.file_name().and_then(|name| name.to_str()).unwrap();

    assert_eq!(derive_label_from_cwd(Path::new(&root), None), label);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn daemon_git_rev_parse_verify_reads_reftable_refs() {
    let root = temp_test_dir("reftable-ref-oid");
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

fn create_bare_repo_with_linked_worktree(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let scratch = temp_test_dir(name);
    let seed = scratch.join("seed");
    let bare = scratch.join(".bare");
    let checkout = scratch.join("feature");
    support::init_repo_with_commit(&seed);
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
    (scratch, bare, checkout)
}

/// Canonical native path with the source fallback on missing paths.
fn canonicalize_best_effort_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[test]
fn repository_native_links_keep_unicode_trim_and_stop_at_missing_head() {
    let scratch = temp_test_dir("native-boundaries");
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
        None,
        "a broken link cannot select the outer repository"
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
        None,
        "an unreadable shared metadata link stops discovery"
    );
    std::fs::remove_dir_all(scratch).unwrap();
}
