//! Native integrity scope, offline and destructive-target boundaries.
#![cfg(unix)]
use crate::native_git::{git, retry::RetryFixture};
use maestro_packages::PackageManager;

#[test]
fn offline_pin_repair_recovers_failed_commands_and_rejects_missing_commits() {
    use maestro_packages::{DefaultPackageManager, PackageManagerOptions};
    let fixture = RetryFixture::new("ordinary");
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: fixture
                .repo
                .scratch
                .path()
                .join("cwd")
                .to_str()
                .unwrap()
                .into(),
            agent_dir: fixture
                .repo
                .scratch
                .path()
                .join("agent")
                .to_str()
                .unwrap()
                .into(),
            settings_manager: fixture.settings.clone(),
        },
        crate::support::Controlled(fixture.effects.clone()),
    )
    .with_offline(Some(true));
    let source = format!("git:github.com/acme/tool@{}", fixture.repo.original);
    fixture.advance();
    git(&fixture.repo.target, &["fetch", "origin"]);
    git(&fixture.repo.target, &["reset", "--hard", "origin/main"]);
    let moved = git(&fixture.repo.target, &["rev-parse", "HEAD"]);
    let status = fixture.run(manager.verify_pin(&source, None)).unwrap();
    assert_ne!(status.expected_commit, status.head_commit);
    assert_eq!(status.changed_paths, ["content"]);
    *fixture.fail.borrow_mut() = "clean".into();
    assert!(fixture.run(manager.restore_pin(&source, None)).is_err());
    assert_eq!(git(&fixture.repo.target, &["rev-parse", "HEAD"]), moved);
    fixture.run(manager.restore_pin(&source, None)).unwrap();
    assert!(
        fixture
            .run(manager.verify_pin(&source, None))
            .unwrap()
            .is_clean()
    );
    assert!(
        fixture
            .run(manager.restore_pin("git:github.com/acme/tool@missing", None))
            .is_err()
    );
    std::fs::remove_dir_all(&fixture.repo.target).unwrap();
    assert!(fixture.run(manager.restore_pin(&source, None)).is_err());
    assert!(!fixture.repo.target.exists());
}

#[test]
fn integrity_refuses_local_sources_and_redirected_remote_checkouts() {
    let fixture = RetryFixture::new("ordinary");
    for source in [
        fixture.repo.seed.to_str().unwrap(),
        "npm:example@1",
        "git:github.com/acme/tool",
    ] {
        assert!(
            fixture
                .run(fixture.manager.verify_pin(source, None))
                .is_err()
        );
        assert!(
            fixture
                .run(fixture.manager.restore_pin(source, None))
                .is_err()
        );
    }
    let source = format!("git:github.com/acme/tool@{}", fixture.repo.original);
    std::fs::remove_dir_all(&fixture.repo.target).unwrap();
    std::os::unix::fs::symlink(&fixture.repo.seed, &fixture.repo.target).unwrap();
    assert!(
        fixture
            .run(fixture.manager.verify_pin(&source, None))
            .is_err()
    );
    assert!(
        fixture
            .run(fixture.manager.restore_pin(&source, None))
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(fixture.repo.seed.join("content")).unwrap(),
        "manifest"
    );
    for layout in ["linked", "separate"] {
        let fixture = RetryFixture::new(layout);
        assert!(
            fixture
                .run(fixture.manager.restore_pin(&source, None))
                .is_err()
        );
        assert!(fixture.repo.target.exists());
    }
}

#[test]
fn project_pin_integrity_does_not_mutate_user_checkout() {
    use maestro_packages::InstalledSourceScope;
    let fixture = RetryFixture::new("ordinary");
    let target = fixture
        .repo
        .scratch
        .path()
        .join("cwd/.maestro/git/github.com/acme/tool");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    git(
        fixture.repo.scratch.path(),
        &[
            "clone",
            fixture.repo.remote.to_str().unwrap(),
            target.to_str().unwrap(),
        ],
    );
    std::fs::write(target.join("content"), "project drift").unwrap();
    std::fs::write(fixture.repo.target.join("content"), "user drift").unwrap();
    let source = format!("git:github.com/acme/tool@{}", fixture.repo.original);
    fixture
        .run(
            fixture
                .manager
                .restore_pin(&source, Some(InstalledSourceScope::Project)),
        )
        .unwrap();
    assert!(
        fixture
            .run(
                fixture
                    .manager
                    .verify_pin(&source, Some(InstalledSourceScope::Project))
            )
            .unwrap()
            .is_clean()
    );
    assert_eq!(
        std::fs::read_to_string(fixture.repo.target.join("content")).unwrap(),
        "user drift"
    );
}
