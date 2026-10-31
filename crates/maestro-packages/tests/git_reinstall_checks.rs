//! Installing again over an existing Git checkout reconciles it with its upstream.
#![cfg(unix)]
use crate::native_git::{Repository, git};

/// Publishes `text` on branch `name`, creating the branch from `main` when it is new.
fn publish_branch(repo: &Repository, name: &str, text: &str) {
    git(&repo.seed, &["checkout", "-B", name]);
    repo.commit_here(text, name);
    git(&repo.seed, &["checkout", "main"]);
}

/// Reinstalling with a moved explicit ref fetches first and ends at the new commit.
#[test]
fn reinstall_with_a_moved_ref_checks_out_the_new_commit() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let repo = Repository::new();
    publish_branch(&repo, "v1", "one");
    publish_branch(&repo, "v2", "two");
    git(&repo.target, &["fetch", "origin"]);
    git(&repo.target, &["checkout", "v1"]);
    publish_branch(&repo, "v2", "three");
    repo.install("git:github.com/acme/tool@v2");
    assert_eq!(
        git(&repo.target, &["rev-parse", "HEAD"]),
        git(&repo.remote, &["rev-parse", "v2"])
    );
    assert_eq!(
        std::fs::read_to_string(repo.target.join("content")).unwrap(),
        "three"
    );
}

/// A local branch of the requested name follows its upstream when it moved.
#[test]
fn reinstall_with_the_current_ref_follows_its_upstream_move() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let repo = Repository::new();
    publish_branch(&repo, "v2", "two");
    git(&repo.target, &["fetch", "origin"]);
    git(&repo.target, &["checkout", "v2"]);
    publish_branch(&repo, "v2", "three");
    repo.install("git:github.com/acme/tool@v2");
    assert_eq!(
        git(&repo.target, &["rev-parse", "HEAD"]),
        git(&repo.remote, &["rev-parse", "v2"])
    );
}

/// Reinstalling an unpinned package leaves HEAD at the commit upstream gained.
#[test]
fn reinstall_without_a_ref_refreshes_to_the_upstream_head() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let repo = Repository::new();
    repo.commit("next");
    repo.install("git:github.com/acme/tool");
    repo.assert_remote("next");
}
