//! Installing again over an existing Git checkout reconciles it with its upstream.
#![cfg(unix)]
use crate::native_git::{git, retry::RetryFixture};
use maestro_packages::PackageManager;

/// Publishes `text` on branch `name` of the fixture's remote, creating it from `main` when new.
fn publish_branch(fixture: &RetryFixture, name: &str, text: &str) {
    let seed = &fixture.repo.seed;
    git(seed, &["checkout", "-B", name]);
    std::fs::write(seed.join("content"), text).unwrap();
    git(seed, &["add", "content"]);
    git(seed, &["commit", "-m", text]);
    git(seed, &["push", "--force", "origin", name]);
    git(seed, &["checkout", "main"]);
}

/// Installs `source` again over the fixture's checkout.
fn reinstall(fixture: &RetryFixture, source: &str) {
    fixture.run(fixture.manager.install(source, None)).unwrap();
}

/// The checkout's HEAD and the remote's `name`.
fn heads(fixture: &RetryFixture, name: &str) -> (String, String) {
    (
        git(&fixture.repo.target, &["rev-parse", "HEAD"]),
        git(&fixture.repo.remote, &["rev-parse", name]),
    )
}

/// Reinstalling with a moved explicit ref fetches first and ends at the new commit.
#[test]
fn reinstall_with_a_moved_ref_checks_out_the_new_commit() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let fixture = RetryFixture::new("ordinary");
    publish_branch(&fixture, "v1", "one");
    publish_branch(&fixture, "v2", "two");
    git(&fixture.repo.target, &["fetch", "origin"]);
    git(&fixture.repo.target, &["checkout", "v1"]);
    publish_branch(&fixture, "v2", "three");
    reinstall(&fixture, "git:github.com/acme/tool@v2");
    let (head, remote) = heads(&fixture, "v2");
    assert_eq!(head, remote);
    assert_eq!(
        std::fs::read_to_string(fixture.repo.target.join("content")).unwrap(),
        "three"
    );
}

/// The checkout keeps its own branch, which moves to the fetched commit.
#[test]
fn reinstall_moves_the_selected_branch_to_the_fetched_commit() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let fixture = RetryFixture::new("ordinary");
    publish_branch(&fixture, "v2", "two");
    reinstall(&fixture, "git:github.com/acme/tool@v2");
    let (head, remote) = heads(&fixture, "v2");
    assert_eq!(head, remote);
    assert_eq!(
        git(&fixture.repo.target, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "main"
    );
}

/// A tag that moved upstream resolves to its new commit although the checkout holds the old tag.
#[test]
fn reinstall_with_a_moved_tag_checks_out_the_new_commit() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let fixture = RetryFixture::new("ordinary");
    let remote = &fixture.repo.remote;
    publish_branch(&fixture, "source", "two");
    git(remote, &["tag", "--force", "v2", "source"]);
    git(&fixture.repo.target, &["fetch", "origin", "--tags"]);
    git(&fixture.repo.target, &["checkout", "v2"]);
    publish_branch(&fixture, "source", "three");
    git(remote, &["tag", "--force", "v2", "source"]);
    reinstall(&fixture, "git:github.com/acme/tool@v2");
    let (head, remote) = heads(&fixture, "v2");
    assert_eq!(head, remote);
    assert_eq!(
        std::fs::read_to_string(fixture.repo.target.join("content")).unwrap(),
        "three"
    );
}

/// A local branch of the requested name follows its upstream when it moved.
#[test]
fn reinstall_with_the_current_ref_follows_its_upstream_move() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let fixture = RetryFixture::new("ordinary");
    publish_branch(&fixture, "v2", "two");
    git(&fixture.repo.target, &["fetch", "origin"]);
    git(&fixture.repo.target, &["checkout", "v2"]);
    publish_branch(&fixture, "v2", "three");
    reinstall(&fixture, "git:github.com/acme/tool@v2");
    let (head, remote) = heads(&fixture, "v2");
    assert_eq!(head, remote);
}

/// Reinstalling an unpinned package leaves HEAD at the commit upstream gained.
#[test]
fn reinstall_without_a_ref_refreshes_to_the_upstream_head() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let fixture = RetryFixture::new("ordinary");
    fixture.repo.commit("next");
    reinstall(&fixture, "git:github.com/acme/tool");
    let (head, remote) = heads(&fixture, "main");
    assert_eq!(head, remote);
    assert_ne!(head, fixture.repo.original);
}
