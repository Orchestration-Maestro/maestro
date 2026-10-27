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
