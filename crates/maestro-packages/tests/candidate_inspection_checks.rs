//! Candidate reads preserve installed files and never prepare dependencies.
#![cfg(unix)]
use crate::native_git::{git, retry::RetryFixture};
use maestro_packages::{NativePackageOperations, PackageManager, PackageOperations, SourceScope};
use std::{path::Path, rc::Rc};

#[test]
fn candidate_reads_alternate_pins_without_installing_or_mutating_active_files() {
    let Some(tree) = crate::resolution_support::case_in_child("candidates", |_| vec![]) else {
        return;
    };
    let fixture = RetryFixture::new("ordinary");
    fixture.use_local_clone();
    enable_native_dependencies(&fixture);
    seed_candidate_releases(&fixture);
    let temporary = fixture
        .run(fixture.manager.resolve_extension_sources(
            &["git:github.com/acme/tool@v1".into()],
            Some(SourceScope::Temporary),
        ))
        .unwrap()
        .packages
        .remove(0)
        .installed_path;
    assert!(Path::new(&temporary).join("sentinel").is_file());
    std::fs::remove_file(Path::new(&temporary).join("sentinel")).unwrap();
    let active = std::fs::read(fixture.repo.target.join("content")).unwrap();
    let settings = fixture.settings.borrow().get_global_settings();
    let layout = entries(Path::new(&temporary).parent().unwrap());
    for pin in ["v2", "v1", "v2"] {
        let source = format!("git:github.com/acme/tool@{pin}");
        let files = fixture
            .run(fixture.manager.read_candidate_files(
                &source,
                &[
                    "content".into(),
                    "missing-policy.json".into(),
                    "sentinel".into(),
                ],
            ))
            .unwrap();
        let expected = if pin == "v1" {
            b"first candidate".to_vec()
        } else {
            vec![0, 255, 10, 13]
        };
        assert_eq!(files, vec![Some(expected), None, None]);
        assert_eq!(entries(Path::new(&temporary).parent().unwrap()), layout);
    }
    assert_eq!(
        std::fs::read(fixture.repo.target.join("content")).unwrap(),
        active
    );
    assert_eq!(
        git(&fixture.repo.target, &["rev-parse", "HEAD"]),
        fixture.repo.original
    );
    assert_eq!(
        std::fs::read_to_string(Path::new(&temporary).join("content")).unwrap(),
        "first candidate"
    );
    assert_eq!(fixture.settings.borrow().get_global_settings(), settings);
    assert!(!Path::new(&temporary).join("sentinel").exists());
    assert!(tree.join("temp").is_dir());
}

/// Lets the fixture's existing native Git transport also execute the configured dependency installer.
fn enable_native_dependencies(fixture: &RetryFixture) {
    fixture.settings.borrow_mut().set_npm_command(Some(
        ["/bin/sh", "lifecycle.sh"]
            .map(|arg| maestro_settings::SettingsListEntry::String(arg.into()))
            .to_vec(),
    ));
    let hook = fixture.effects.run_hook.borrow().clone().unwrap();
    let native = Rc::new(NativePackageOperations::with_spawn(
        Rc::new(|| false),
        |operation| {
            drop(tokio::task::spawn_local(operation));
            Ok(())
        },
    ));
    *fixture.effects.run_hook.borrow_mut() = Some(Rc::new(move |command, args, cwd| {
        if command == "git" {
            return hook(command, args, cwd);
        }
        let (native, command, args, cwd) = (
            native.clone(),
            command.to_owned(),
            args.to_vec(),
            cwd.map(str::to_owned),
        );
        Box::pin(async move { native.run_command(&command, &args, cwd.as_deref()).await })
    }));
}

/// Lists siblings so leaked candidate staging directories are observable.
fn entries(path: &Path) -> Vec<std::ffi::OsString> {
    let mut names = std::fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[test]
fn candidate_failures_clean_staging_and_reject_escaping_paths() {
    let Some(_) = crate::resolution_support::case_in_child("failures", |_| vec![]) else {
        return;
    };
    let fixture = RetryFixture::new("ordinary");
    fixture.use_local_clone();
    let paths = ["content".into()];
    let source = format!("git:github.com/acme/tool@{}", fixture.repo.original);
    assert_eq!(
        fixture
            .run(fixture.manager.read_candidate_files(&source, &paths))
            .unwrap(),
        vec![Some(b"manifest".to_vec())]
    );
    let clone = fixture
        .effects
        .calls
        .borrow()
        .iter()
        .find(|(_, args)| args.iter().any(|arg| arg == "clone"))
        .unwrap()
        .1
        .last()
        .unwrap()
        .clone();
    let parent = Path::new(&clone).parent().unwrap();
    assert!(entries(parent).is_empty());
    assert!(
        fixture
            .run(
                fixture
                    .manager
                    .read_candidate_files("git:github.com/acme/tool@absent", &paths)
            )
            .is_err()
    );
    assert!(entries(parent).is_empty());
    *fixture.fail.borrow_mut() = "clone".into();
    assert!(
        fixture
            .run(fixture.manager.read_candidate_files(&source, &paths))
            .is_err()
    );
    assert!(entries(parent).is_empty());
    assert_candidate_path_refusals(&fixture, &source);
    assert!(entries(parent).is_empty());
    assert!(fixture.dependencies.borrow().is_empty());
}

#[test]
fn candidate_inspection_offline_starts_no_command() {
    let (manager, _, effects) = crate::support::manager(&serde_json::json!({}));
    let manager = manager.with_offline(Some(true));
    let result = crate::runtime::run(
        manager.read_candidate_files("git:github.com/acme/tool@v1", &["content".into()]),
    );
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotConnected);
    assert!(effects.calls.borrow().is_empty());
}

/// Publishes two tagged candidates with a real install-script sentinel.
fn seed_candidate_releases(fixture: &RetryFixture) {
    let manifest = r#"{"name":"candidate-fixture","version":"1.0.0","private":true,"scripts":{"preinstall":"sh lifecycle.sh"}}"#;
    std::fs::write(fixture.repo.seed.join("package.json"), manifest).unwrap();
    std::fs::write(
        fixture.repo.seed.join("lifecycle.sh"),
        "printf installed > sentinel\n",
    )
    .unwrap();
    git(&fixture.repo.seed, &["add", "package.json", "lifecycle.sh"]);
    fixture.repo.commit("first candidate");
    git(&fixture.repo.seed, &["tag", "v1"]);
    std::fs::write(fixture.repo.seed.join("content"), [0, 255, 10, 13]).unwrap();
    git(&fixture.repo.seed, &["add", "content"]);
    git(&fixture.repo.seed, &["commit", "-m", "binary candidate"]);
    git(&fixture.repo.seed, &["tag", "v2"]);
    git(&fixture.repo.seed, &["push", "origin", "main", "--tags"]);
}

/// Rejects invalid operands and links that would expose bytes outside the candidate.
fn assert_candidate_path_refusals(fixture: &RetryFixture, source: &str) {
    let paths = ["content".into()];
    for path in ["../outside", "/absolute"] {
        assert_eq!(
            fixture
                .run(fixture.manager.read_candidate_files(source, &[path.into()]))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
    }
    for source in ["npm:pkg", "./local", "git:github.com/acme/tool"] {
        assert_eq!(
            fixture
                .run(fixture.manager.read_candidate_files(source, &paths))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
    }
    std::os::unix::fs::symlink(
        fixture.repo.target.join("content"),
        fixture.repo.seed.join("outside"),
    )
    .unwrap();
    git(&fixture.repo.seed, &["add", "outside"]);
    fixture.repo.commit("outside link");
    assert_eq!(
        fixture
            .run(
                fixture
                    .manager
                    .read_candidate_files("git:github.com/acme/tool@main", &["outside".into()])
            )
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
}

#[test]
fn candidate_pin_cannot_resolve_only_to_a_tracked_path() {
    let Some(_) = crate::resolution_support::case_in_child("path-pin", |_| vec![]) else {
        return;
    };
    let fixture = RetryFixture::new("ordinary");
    fixture.use_local_clone();
    assert!(
        fixture
            .run(
                fixture
                    .manager
                    .read_candidate_files("git:github.com/acme/tool@content", &["content".into()],)
            )
            .is_err()
    );
}

#[test]
fn candidate_inspection_disables_inherited_checkout_hooks() {
    use std::os::unix::fs::PermissionsExt;
    let Some(tree) = crate::resolution_support::case_in_child("hooks", |base| {
        vec![
            ("GIT_CONFIG_COUNT", Some("1".into())),
            ("GIT_CONFIG_KEY_0", Some("core.hooksPath".into())),
            ("GIT_CONFIG_VALUE_0", Some(format!("{base}/hooks"))),
        ]
    }) else {
        return;
    };
    let fixture = RetryFixture::new("ordinary");
    fixture.use_local_clone();
    let hooks = tree.join("hooks");
    std::fs::create_dir(&hooks).unwrap();
    let hook = hooks.join("post-checkout");
    std::fs::write(
        &hook,
        "#!/bin/sh\nprintf ran >> \"$MAESTRO_TEST_TREE/hook-sentinel\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let source = format!("git:github.com/acme/tool@{}", fixture.repo.original);
    assert_eq!(
        fixture
            .run(
                fixture
                    .manager
                    .read_candidate_files(&source, &["content".into()])
            )
            .unwrap(),
        vec![Some(b"manifest".to_vec())]
    );
    assert!(!tree.join("hook-sentinel").exists());
    fixture
        .run(
            fixture
                .manager
                .resolve_extension_sources(&[source], Some(SourceScope::Temporary)),
        )
        .unwrap();
    assert!(tree.join("hook-sentinel").is_file());
}

#[test]
fn candidate_reads_non_default_branch_pins() {
    let Some(_) = crate::resolution_support::case_in_child("branch-pin", |_| vec![]) else {
        return;
    };
    let fixture = RetryFixture::new("ordinary");
    fixture.use_local_clone();
    git(&fixture.repo.seed, &["checkout", "-b", "new/slash"]);
    fixture.repo.commit("branch candidate");
    git(&fixture.repo.seed, &["push", "origin", "new/slash"]);
    let files = fixture
        .run(
            fixture
                .manager
                .read_candidate_files("git:github.com/acme/tool@new/slash", &["content".into()]),
        )
        .unwrap();
    assert_eq!(files, vec![Some(b"branch candidate".to_vec())]);
}

#[test]
fn candidate_reads_local_branch_before_colliding_tag_like_installation() {
    let Some(_) = crate::resolution_support::case_in_child("colliding-pin", |_| vec![]) else {
        return;
    };
    let fixture = RetryFixture::new("ordinary");
    fixture.use_local_clone();
    fixture.repo.commit("branch candidate");
    git(&fixture.repo.seed, &["tag", "main", &fixture.repo.original]);
    git(&fixture.repo.seed, &["push", "origin", "--tags"]);
    let source = "git:github.com/acme/tool@main";
    let installed = fixture
        .run(
            fixture
                .manager
                .resolve_extension_sources(&[source.into()], Some(SourceScope::Temporary)),
        )
        .unwrap()
        .packages
        .remove(0)
        .installed_path;
    let files = fixture
        .run(
            fixture
                .manager
                .read_candidate_files(source, &["content".into()]),
        )
        .unwrap();
    assert_eq!(
        files,
        vec![Some(
            std::fs::read(Path::new(&installed).join("content")).unwrap()
        )]
    );
    assert_eq!(files, vec![Some(b"branch candidate".to_vec())]);
}
