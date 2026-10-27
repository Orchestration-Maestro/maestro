//! Release discovery through the package manager's Git capture boundary.
#![cfg(unix)]
use crate::native_git::{git, retry::RetryFixture};
use maestro_packages::{GitRelease, PackageManager};
use std::rc::Rc;

#[test]
fn release_listing_peels_tags_without_selecting_or_installing() {
    let Some(_) = crate::resolution_support::case_in_child("online", |_| vec![]) else {
        return;
    };
    let fixture = RetryFixture::new("ordinary");
    git(&fixture.repo.remote, &["tag", "v1"]);
    git(&fixture.repo.remote, &["tag", "-a", "v2", "-m", "second"]);
    let hook = fixture.effects.capture_hook.borrow().clone().unwrap();
    let remote = fixture.repo.remote.to_string_lossy().into_owned();
    *fixture.effects.capture_hook.borrow_mut() = Some(Rc::new(move |command, args, cwd| {
        let mut args = args.to_vec();
        if args.first().is_some_and(|arg| arg == "ls-remote") {
            args[2].clone_from(&remote);
        }
        hook(command, &args, cwd)
    }));
    let releases = fixture
        .run(
            fixture
                .manager
                .list_releases("git:github.com/acme/tool@old"),
        )
        .unwrap();
    assert_eq!(
        releases,
        vec![
            GitRelease {
                name: "v1".into(),
                commit: fixture.repo.original.clone()
            },
            GitRelease {
                name: "v2".into(),
                commit: fixture.repo.original.clone()
            },
        ]
    );
    assert!(fixture.dependencies.borrow().is_empty());
    assert_eq!(
        git(&fixture.repo.target, &["rev-parse", "HEAD"]),
        fixture.repo.original
    );
}
