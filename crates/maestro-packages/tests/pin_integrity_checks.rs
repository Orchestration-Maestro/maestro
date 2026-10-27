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
