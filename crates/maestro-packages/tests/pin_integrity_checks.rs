//! Managed pinned checkout integrity through native Git effects.
#![cfg(unix)]
use crate::native_git::{git, retry::RetryFixture};
use maestro_packages::PackageManager;

#[test]
fn pinned_checkout_reports_and_restores_all_same_head_drift() {
    let fixture = RetryFixture::new("ordinary");
    let source = format!("git:github.com/acme/tool@{}", fixture.repo.original);
    let path = &fixture.repo.target;
    std::fs::write(path.join("content"), "edited").unwrap();
    std::fs::remove_file(path.join("extensions/retained.ts")).unwrap();
    std::fs::write(path.join("added"), "staged").unwrap();
    git(path, &["add", "added"]);
    std::fs::write(path.join("untracked\nfile"), "untracked").unwrap();
    let status = fixture
        .run(fixture.manager.verify_pin(&source, None))
        .unwrap();
    assert!(!status.is_clean());
    assert_eq!(
        status.expected_commit.as_deref(),
        Some(fixture.repo.original.as_str())
    );
    assert_eq!(status.head_commit, status.expected_commit);
    assert_eq!(
        status.changed_paths,
        [
            "added",
            "content",
            "extensions/retained.ts",
            "untracked\nfile"
        ]
    );
    let before = fixture.settings.borrow().get_global_settings();
    fixture
        .run(fixture.manager.restore_pin(&source, None))
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(path.join("content")).unwrap(),
        "manifest"
    );
    assert!(path.join("extensions/retained.ts").exists());
    assert!(!path.join("added").exists());
    assert!(!path.join("untracked\nfile").exists());
    assert!(
        fixture
            .run(fixture.manager.verify_pin(&source, None))
            .unwrap()
            .is_clean()
    );
    assert_eq!(fixture.settings.borrow().get_global_settings(), before);
}

#[test]
fn unavailable_pin_reinstalls_without_losing_checkout_on_failure() {
    let fixture = RetryFixture::new("ordinary");
    fixture.use_local_clone();
    git(&fixture.repo.seed, &["tag", "release"]);
    git(&fixture.repo.seed, &["push", "origin", "release"]);
    let source = "git:github.com/acme/tool@release";
    let status = fixture
        .run(fixture.manager.verify_pin(source, None))
        .unwrap();
    assert!(status.expected_commit.is_none());
    std::fs::write(fixture.repo.target.join("content"), "retain until prepared").unwrap();
    *fixture.fail.borrow_mut() = "clone".into();
    assert!(
        fixture
            .run(fixture.manager.restore_pin(source, None))
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(fixture.repo.target.join("content")).unwrap(),
        "retain until prepared"
    );
    fixture
        .run(fixture.manager.restore_pin(source, None))
        .unwrap();
    assert!(
        fixture
            .run(fixture.manager.verify_pin(source, None))
            .unwrap()
            .is_clean()
    );
    std::fs::remove_dir_all(&fixture.repo.target).unwrap();
    assert!(
        fixture
            .run(fixture.manager.verify_pin(source, None))
            .unwrap()
            .head_commit
            .is_none()
    );
    fixture
        .run(fixture.manager.restore_pin(source, None))
        .unwrap();
    assert!(
        fixture
            .run(fixture.manager.verify_pin(source, None))
            .unwrap()
            .is_clean()
    );
    assert_eq!(
        std::fs::read_dir(fixture.repo.target.parent().unwrap())
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn pin_integrity_includes_index_only_drift_and_nested_untracked_repositories() {
    let fixture = RetryFixture::new("ordinary");
    let source = format!("git:github.com/acme/tool@{}", fixture.repo.original);
    let target = &fixture.repo.target;
    std::fs::write(target.join("index-only"), "added then removed").unwrap();
    git(target, &["add", "index-only"]);
    std::fs::remove_file(target.join("index-only")).unwrap();
    let status = fixture
        .run(fixture.manager.verify_pin(&source, None))
        .unwrap();
    assert_eq!(status.changed_paths, ["index-only"]);
    let nested = target.join("nested");
    std::fs::create_dir(&nested).unwrap();
    git(&nested, &["init"]);
    std::fs::write(nested.join("file"), "untracked repo").unwrap();
    fixture
        .run(fixture.manager.restore_pin(&source, None))
        .unwrap();
    assert!(!nested.exists());
    assert!(
        fixture
            .run(fixture.manager.verify_pin(&source, None))
            .unwrap()
            .is_clean()
    );
}

#[test]
fn reinstalled_pin_discards_dependency_generated_drift() {
    use std::rc::Rc;
    let fixture = RetryFixture::new("ordinary");
    fixture.use_local_clone();
    git(&fixture.repo.seed, &["tag", "release"]);
    git(&fixture.repo.seed, &["push", "origin", "release"]);
    let original = fixture.effects.run_hook.borrow().clone().unwrap();
    *fixture.effects.run_hook.borrow_mut() = Some(Rc::new(move |command, args, cwd| {
        let result = original(command, args, cwd);
        let cwd = cwd.map(std::path::PathBuf::from);
        let dependency = command != "git";
        Box::pin(async move {
            let status = result.await?;
            if dependency {
                let cwd = cwd.unwrap();
                std::fs::write(cwd.join("content"), "dependency drift").unwrap();
                std::fs::write(cwd.join("generated"), "generated").unwrap();
            }
            Ok(status)
        })
    }));
    let source = "git:github.com/acme/tool@release";
    fixture
        .run(fixture.manager.restore_pin(source, None))
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(fixture.repo.target.join("content")).unwrap(),
        "manifest"
    );
    assert!(!fixture.repo.target.join("generated").exists());
    assert!(
        fixture
            .run(fixture.manager.verify_pin(source, None))
            .unwrap()
            .is_clean()
    );
}

#[test]
fn pin_paths_preserve_leading_whitespace() {
    for name in [" leading", "\nleading"] {
        let fixture = RetryFixture::new("ordinary");
        let source = format!("git:github.com/acme/tool@{}", fixture.repo.original);
        std::fs::write(fixture.repo.target.join(name), "untracked").unwrap();
        let status = fixture
            .run(fixture.manager.verify_pin(&source, None))
            .unwrap();
        assert_eq!(status.changed_paths, [name]);
    }
}

#[test]
fn pin_paths_include_both_sides_of_staged_rename() {
    let fixture = RetryFixture::new("ordinary");
    let source = format!("git:github.com/acme/tool@{}", fixture.repo.original);
    git(&fixture.repo.target, &["mv", "content", "renamed"]);
    let status = fixture
        .run(fixture.manager.verify_pin(&source, None))
        .unwrap();
    assert_eq!(status.changed_paths, ["content", "renamed"]);
}

#[test]
fn pin_checks_do_not_trust_index_suppression_flags() {
    for flag in ["--assume-unchanged", "--skip-worktree"] {
        let fixture = RetryFixture::new("ordinary");
        fixture.use_local_clone();
        let source = format!("git:github.com/acme/tool@{}", fixture.repo.original);
        git(&fixture.repo.target, &["update-index", flag, "content"]);
        std::fs::write(fixture.repo.target.join("content"), "hidden edit").unwrap();
        let index = std::fs::read(fixture.repo.target.join(".git/index")).unwrap();
        let status = fixture
            .run(fixture.manager.verify_pin(&source, None))
            .unwrap();
        assert_eq!(
            std::fs::read(fixture.repo.target.join(".git/index")).unwrap(),
            index
        );
        assert_eq!(status.changed_paths, ["content"]);
        assert!(!status.is_clean());
        fixture
            .run(fixture.manager.restore_pin(&source, None))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(fixture.repo.target.join("content")).unwrap(),
            "manifest"
        );
        assert!(
            fixture
                .run(fixture.manager.verify_pin(&source, None))
                .unwrap()
                .is_clean()
        );
    }
}

#[test]
fn unavailable_pin_reports_tracked_drift_against_head() {
    let fixture = RetryFixture::new("ordinary");
    std::fs::write(fixture.repo.target.join("content"), "edited").unwrap();
    git(
        &fixture.repo.target,
        &["mv", "extensions/retained.ts", "renamed.ts"],
    );
    let status = fixture
        .run(
            fixture
                .manager
                .verify_pin("git:github.com/acme/tool@absent", None),
        )
        .unwrap();
    assert!(status.expected_commit.is_none());
    assert_eq!(
        status.changed_paths,
        ["content", "extensions/retained.ts", "renamed.ts"]
    );
}

#[test]
fn missing_project_pin_prepares_ignored_managed_root() {
    use maestro_packages::InstalledSourceScope;
    let fixture = RetryFixture::new("ordinary");
    fixture.use_local_clone();
    let source = format!("git:github.com/acme/tool@{}", fixture.repo.original);
    let cwd = fixture.repo.scratch.path().join("cwd");
    std::fs::create_dir(&cwd).unwrap();
    git(&cwd, &["init"]);
    fixture
        .run(
            fixture
                .manager
                .restore_pin(&source, Some(InstalledSourceScope::Project)),
        )
        .unwrap();
    assert!(cwd.join(".maestro/git/.gitignore").is_file());
    assert_eq!(
        git(&cwd, &["status", "--porcelain", "--untracked-files=all"]),
        "?? .maestro/git/.gitignore"
    );
}

#[test]
fn pin_restore_does_not_accept_remaining_submodule_edits() {
    let fixture = RetryFixture::new("ordinary");
    fixture.use_local_clone();
    git(
        &fixture.repo.target,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            fixture.repo.remote.to_str().unwrap(),
            "sub",
        ],
    );
    git(&fixture.repo.target, &["commit", "-am", "submodule"]);
    let commit = git(&fixture.repo.target, &["rev-parse", "HEAD"]);
    git(
        &fixture.repo.target,
        &["push", "origin", "HEAD:refs/heads/with-sub"],
    );
    let source = format!("git:github.com/acme/tool@{commit}");
    std::fs::write(fixture.repo.target.join("sub/content"), "nested edit").unwrap();
    assert!(
        !fixture
            .run(fixture.manager.verify_pin(&source, None))
            .unwrap()
            .is_clean()
    );
    fixture
        .run(fixture.manager.restore_pin(&source, None))
        .unwrap();
    assert!(
        fixture
            .run(fixture.manager.verify_pin(&source, None))
            .unwrap()
            .is_clean()
    );
    let nested = fixture.repo.target.join("sub/content");
    assert!(!nested.exists() || std::fs::read_to_string(nested).unwrap() != "nested edit");
}
