//! Explicit install and removal through scripted children over disposable native files.
#![cfg(test)]
use crate::native_support;
use crate::sandbox;
#[cfg(windows)]
use crate::scratch;
use maestro_packages::{
    InstalledSourceScope::{Project, User},
    PackageManager, ProgressAction, ProgressCallback, ProgressEventType as Phase, parse_git_url,
};
use maestro_settings::{
    InMemorySettingsStorage, PackageSource, SettingsListEntry, SettingsScope, SettingsStorage,
    SettingsStorageError, SettingsUpdate,
};
use sandbox::{Call, Fixture, block_on, poll_once};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
};

/// The ignore file text written into new managed roots.
const IGNORE: &str = "*\n!.gitignore\n";
/// The manifest text written into new project npm roots.
const MANIFEST: &str = "{\n  \"name\": \"maestro-extensions\",\n  \"private\": true\n}";

/// The arguments of one launch.
fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}
/// A launch without a working directory.
fn call(command: &str, items: &[&str]) -> Call {
    Call {
        command: command.into(),
        args: args(items),
        cwd: None,
    }
}

#[test]
fn npm_specs_keep_trimmed_argv_and_names() {
    let cases = [
        ("npm:pkg", "pkg", "pkg"),
        ("npm:@scope/pkg", "@scope/pkg", "@scope/pkg"),
        ("npm:pkg@1.2.3", "pkg@1.2.3", "pkg"),
        ("npm:@scope/pkg@^1.0.0", "@scope/pkg@^1.0.0", "@scope/pkg"),
        ("npm:pkg@latest", "pkg@latest", "pkg"),
        ("npm:a@b@c", "a@b@c", "a"),
        ("npm:pkg@", "pkg@", "pkg@"),
        ("npm:@", "@", "@"),
        ("npm:@@1", "@@1", "@@1"),
        ("npm:foo/bar@2", "foo/bar@2", "foo/bar"),
        ("npm:pkg@1\n2", "pkg@1\n2", "pkg@1\n2"),
        ("npm:pkg@1\r2", "pkg@1\r2", "pkg@1\r2"),
        ("npm:pkg@1\u{2028}2", "pkg@1\u{2028}2", "pkg@1\u{2028}2"),
        ("npm:pkg@1\u{2029}2", "pkg@1\u{2029}2", "pkg@1\u{2029}2"),
        ("npm:  pkg@1\n", "pkg@1", "pkg"),
        ("npm:\u{feff}pkg@1\u{feff}", "pkg@1", "pkg"),
        ("npm:pkg\u{85}", "pkg\u{85}", "pkg\u{85}"),
        ("npm:", "", ""),
    ];
    for (source, spec, name) in cases {
        let fixture = Fixture::new(&json!({}));
        block_on(fixture.manager.install(source, None)).unwrap();
        block_on(fixture.manager.remove(source, None)).unwrap();
        assert_eq!(
            fixture.calls(),
            [
                call("npm", &["install", "-g", spec]),
                call("npm", &["uninstall", "-g", name])
            ],
            "case {source:?}"
        );
    }
}

#[test]
fn npm_command_selection_runs_after_progress() {
    let selections = [
        (json!({}), "npm", vec![]),
        (json!({"npmCommand": []}), "npm", vec![]),
        (json!({"npmCommand": ["bun"]}), "bun", vec![]),
        (
            json!({"npmCommand": ["pnpm", "--silent", "bun"]}),
            "pnpm",
            vec!["--silent", "bun"],
        ),
    ];
    for (settings, command, leading) in selections {
        let fixture = Fixture::new(&settings);
        block_on(fixture.manager.install("npm:pkg", None)).unwrap();
        let mut expected = leading.clone();
        expected.extend(["install", "-g", "pkg"]);
        assert_eq!(fixture.calls(), [call(command, &expected)]);
    }
    let fixture = Fixture::new(&json!({"npmCommand": [""]}));
    let error = block_on(fixture.manager.install("npm:pkg", None)).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Invalid npmCommand: first array entry must be a non-empty command"
    );
    assert_eq!(
        fixture.phases(),
        [
            (Phase::Start, Some("Installing npm:pkg...".into())),
            (Phase::Error, Some(error.to_string()))
        ]
    );
    for malformed in [json!([4]), json!(["npm", 5])] {
        let fixture = Fixture::new(&json!({ "npmCommand": malformed }));
        let error = block_on(fixture.manager.install("npm:pkg", None)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(fixture.calls().is_empty());
    }
    let fixture = Fixture::new(&json!({"npmCommand": [""]}));
    let settings = fixture.settings.clone();
    fixture
        .manager
        .set_progress_callback(Some(Rc::new(move |event| {
            if event.r#type == Phase::Start {
                settings
                    .borrow_mut()
                    .set_npm_command(Some(vec![SettingsListEntry::String("changed".into())]));
            }
            Ok(())
        })));
    block_on(fixture.manager.install("npm:pkg", None)).unwrap();
    assert_eq!(
        fixture.calls(),
        [call("changed", &["install", "-g", "pkg"])]
    );
}

#[test]
fn project_npm_files_are_exact_and_existing_files_survive() {
    let fixture = Fixture::new(&json!({}));
    let root = fixture.project(".maestro/npm");
    block_on(fixture.manager.install("npm:@scope/pkg@1", Some(Project))).unwrap();
    assert_eq!(sandbox::read(format!("{root}/.gitignore")), IGNORE);
    assert_eq!(sandbox::read(format!("{root}/package.json")), MANIFEST);
    assert_eq!(
        fixture.calls(),
        [call("npm", &["install", "@scope/pkg@1", "--prefix", &root])]
    );
    for (ignore, manifest) in [(true, true), (true, false), (false, true)] {
        let fixture = Fixture::new(&json!({}));
        let root = fixture.project(".maestro/npm");
        if ignore {
            native_support::write(format!("{root}/.gitignore"), "kept ignore");
        }
        if manifest {
            native_support::write(format!("{root}/package.json"), "{ kept");
        }
        block_on(fixture.manager.install("npm:pkg", Some(Project))).unwrap();
        let read = |name: &str| sandbox::read(format!("{root}/{name}"));
        assert_eq!(
            read(".gitignore"),
            if ignore { "kept ignore" } else { IGNORE }
        );
        assert_eq!(
            read("package.json"),
            if manifest { "{ kept" } else { MANIFEST }
        );
    }
    let fixture = Fixture::new(&json!({"npmCommand": [""]}));
    let error = block_on(fixture.manager.install("npm:pkg", Some(Project))).unwrap_err();
    assert!(error.to_string().starts_with("Invalid npmCommand"));
    let root = fixture.project(".maestro/npm");
    assert_eq!(sandbox::read(format!("{root}/.gitignore")), IGNORE);
    assert_eq!(sandbox::read(format!("{root}/package.json")), MANIFEST);
}

#[test]
fn project_creation_failure_keeps_prior_effects() {
    let root = |fixture: &Fixture| fixture.project(".maestro/npm");
    #[cfg(unix)]
    {
        let fixture = Fixture::new(&json!({}));
        let result = locked(&fixture.project(".maestro"), || {
            block_on(fixture.manager.install("npm:pkg", Some(Project)))
        });
        if let Some(result) = result {
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
            assert!(fixture.calls().is_empty());
            assert!(!std::path::Path::new(&root(&fixture)).exists());
        }
    }

    let fixture = Fixture::new(&json!({}));
    fixture.script.results.borrow_mut().push_back(Ok(Some(1)));
    let error = block_on(fixture.manager.install("npm:pkg", Some(Project))).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "npm install pkg --prefix {} failed with code 1",
            root(&fixture)
        )
    );
    assert_eq!(
        sandbox::read(format!("{}/.gitignore", root(&fixture))),
        IGNORE
    );
    assert_eq!(
        sandbox::read(format!("{}/package.json", root(&fixture))),
        MANIFEST
    );
}

#[test]
fn git_source_forms_choose_clone_target_and_checkout() {
    let forms = [
        ("https://github.com/user/repo", None),
        ("ssh://git@github.com/user/repo", None),
        ("git:git@github.com:user/repo", None),
        ("git:github.com/user/repo", None),
        ("git:git@github.com:user/repo@v1", Some("v1")),
        ("https://github.com/user/repo#main", Some("main")),
    ];
    for (source, reference) in forms {
        let git =
            parse_git_url(source).unwrap_or_else(|| panic!("form {source:?} is a repository"));
        assert_eq!(git.r#ref.as_deref(), reference, "case {source:?}");
        assert_eq!(git.pinned, reference.is_some());
        let fixture = Fixture::new(&json!({}));
        block_on(fixture.manager.install(source, None)).unwrap();
        let target = fixture.agent(&format!("git/{}/{}", git.host, git.path));
        let stage = fixture.calls()[0].args[2].clone();
        assert_ne!(stage, target);
        let mut expected = vec![call("git", &["clone", &git.repo, &stage])];
        if let Some(reference) = reference {
            expected.push(Call {
                cwd: Some(stage.clone()),
                ..call("git", &["checkout", reference])
            });
        }
        assert_eq!(fixture.calls(), expected, "case {source:?}");
        assert_eq!(
            (git.host.as_str(), git.path.as_str()),
            ("github.com", "user/repo")
        );
    }
    let fixture = Fixture::new(&json!({}));
    for (source, added) in [
        ("https://github.com/user/repo", true),
        ("ssh://git@github.com/user/repo", false),
        ("git:git@github.com:user/repo", false),
    ] {
        assert_eq!(
            fixture
                .manager
                .add_source_to_settings(source, None)
                .unwrap(),
            added
        );
    }
}

#[test]
fn unprefixed_shorthands_validate_as_local_paths() {
    for source in ["git@github.com:user/repo", "github.com/user/repo"] {
        let fixture = Fixture::new(&json!({}));
        let error = block_on(fixture.manager.install(source, None)).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("Path does not exist: {}", fixture.project(source)),
            "case {source:?}"
        );
        assert!(fixture.calls().is_empty());
    }
}

/// Makes the clone create the target directory, and optionally its manifest.
pub(crate) fn clone_creates_target(fixture: &Fixture, manifest: bool) {
    fixture
        .script
        .hooks
        .borrow_mut()
        .push_back(Box::new(move |call| {
            let target = call.args.last().unwrap();
            std::fs::create_dir_all(target).unwrap();
            if manifest {
                std::fs::write(format!("{target}/package.json"), "not json").unwrap();
            }
        }));
}

#[test]
fn controlled_repository_installs_checks_out_and_removes() {
    let source = "https://example.test/team/repo@v1";
    let fixture = Fixture::new(&json!({}));
    fixture
        .script
        .hooks
        .borrow_mut()
        .push_back(Box::new(|call| {
            let target = call.args.last().unwrap();
            std::fs::create_dir_all(target).unwrap();
            std::fs::write(format!("{target}/marker"), "one").unwrap();
        }));
    block_on(fixture.manager.install(source, None)).unwrap();
    let target = fixture.agent("git/example.test/team/repo");
    assert_eq!(sandbox::read(format!("{target}/marker")), "one");
    assert_eq!(sandbox::read(fixture.agent("git/.gitignore")), IGNORE);
    let stage = fixture.calls()[0].args[2].clone();
    assert_ne!(stage, target);
    assert_eq!(
        fixture.calls(),
        vec![
            call("git", &["clone", "https://example.test/team/repo", &stage]),
            Call {
                cwd: Some(stage),
                ..call("git", &["checkout", "v1"])
            },
        ]
    );
    block_on(fixture.manager.remove(source, None)).unwrap();
    assert!(!std::path::Path::new(&fixture.agent("git/example.test")).exists());
    assert!(std::path::Path::new(&fixture.agent("git/.gitignore")).exists());
}

#[test]
fn git_dependencies_use_live_configured_command() {
    let selections = [
        (json!({}), "npm", vec!["install", "--omit=dev"]),
        (
            json!({"npmCommand": []}),
            "npm",
            vec!["install", "--omit=dev"],
        ),
        (json!({"npmCommand": ["bun"]}), "bun", vec!["install"]),
        (json!({"npmCommand": ["npm"]}), "npm", vec!["install"]),
        (
            json!({"npmCommand": ["pnpm", "--x"]}),
            "pnpm",
            vec!["--x", "install"],
        ),
    ];
    let source = "https://github.com/user/repo";
    for (settings, command, expected) in selections {
        let fixture = Fixture::new(&settings);
        clone_creates_target(&fixture, true);
        block_on(fixture.manager.install(source, None)).unwrap();
        let calls = fixture.calls();
        let target = calls[0].args[2].clone();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[1],
            Call {
                cwd: Some(target),
                ..call(command, &expected)
            }
        );
    }
    let fixture = Fixture::new(&json!({"npmCommand": [""]}));
    clone_creates_target(&fixture, true);
    let error = block_on(fixture.manager.install(source, None)).unwrap_err();
    assert!(error.to_string().starts_with("Invalid npmCommand"));
    assert_eq!(fixture.calls().len(), 1);
    assert!(!std::path::Path::new(&fixture.agent("git/github.com/user/repo")).exists());

    let fixture = Fixture::new(&json!({"npmCommand": [""]}));
    clone_creates_target(&fixture, false);
    block_on(fixture.manager.install(source, None)).unwrap();
    assert_eq!(fixture.calls().len(), 1);

    let fixture = Fixture::new(&json!({}));
    let settings = fixture.settings.clone();
    fixture
        .script
        .hooks
        .borrow_mut()
        .push_back(Box::new(move |call| {
            let target = call.args.last().unwrap();
            std::fs::create_dir_all(target).unwrap();
            std::fs::write(format!("{target}/package.json"), "x").unwrap();
            settings
                .borrow_mut()
                .set_npm_command(Some(vec![SettingsListEntry::String("late".into())]));
        }));
    block_on(fixture.manager.install(source, None)).unwrap();
    assert_eq!(fixture.calls()[1].command, "late");
    assert_eq!(fixture.calls()[1].args, ["install"]);
}

#[test]
fn unpinned_existing_git_target_does_not_read_command_or_modify_files() {
    for as_file in [false, true] {
        let fixture = Fixture::new(&json!({"npmCommand": [""]}));
        let target = fixture.agent("git/github.com/user/repo");
        if as_file {
            native_support::write(&target, "a file");
        } else {
            native_support::write(format!("{target}/package.json"), "keep");
        }
        let source = "https://github.com/user/repo";
        block_on(fixture.manager.install(source, None)).unwrap();
        assert!(fixture.calls().is_empty());
        assert!(!std::path::Path::new(&fixture.agent("git/.gitignore")).exists());
        assert_eq!(
            fixture.phases(),
            [
                (Phase::Start, Some(format!("Installing {source}..."))),
                (Phase::Complete, None)
            ]
        );
    }
}

#[test]
fn git_project_paths_preserve_parent_and_ignore_files() {
    let cases = [
        (
            "https://github.com/user/repo",
            Some(Project),
            ".maestro/git",
            "github.com/user",
        ),
        (
            "https://gitlab.com/group/sub/repo",
            Some(Project),
            ".maestro/git",
            "gitlab.com/group/sub",
        ),
        ("https://github.com/user/repo", None, "", "github.com/user"),
    ];
    for (source, scope, base, parent) in cases {
        let fixture = Fixture::new(&json!({}));
        let root = if scope.is_some() {
            fixture.project(base)
        } else {
            fixture.agent("git")
        };
        native_support::write(format!("{root}/.gitignore"), "kept");
        let seen = Rc::new(Cell::new(false));
        let witness = seen.clone();
        let parent_dir = format!("{root}/{parent}");
        fixture
            .script
            .hooks
            .borrow_mut()
            .push_back(Box::new(move |_| {
                witness.set(std::path::Path::new(&parent_dir).is_dir());
            }));
        block_on(fixture.manager.install(source, scope)).unwrap();
        assert!(seen.get(), "case {source:?}");
        assert_eq!(sandbox::read(format!("{root}/.gitignore")), "kept");
        assert_eq!(fixture.calls()[0].cwd, None);
    }
    let fixture = Fixture::new(&json!({}));
    block_on(
        fixture
            .manager
            .install("https://github.com/user/repo", Some(Project)),
    )
    .unwrap();
    assert_eq!(
        sandbox::read(fixture.project(".maestro/git/.gitignore")),
        IGNORE
    );
}

/// Runs `operation` while the directory `path`, created when missing, refuses writes;
/// `None` when this process is not refused, as in a privileged run.
#[cfg(unix)]
fn locked<T>(path: &str, operation: impl FnOnce() -> T) -> Option<T> {
    std::fs::create_dir_all(path).unwrap();
    let _restore = maestro_test_directory::deny(path, 0o555).unwrap()?;
    Some(operation())
}

#[cfg(unix)]
#[test]
fn git_preparation_failures_keep_exactly_the_completed_effects() {
    let source = "https://github.com/user/repo#v1";
    // The directory that refuses writes, and the surviving state: (git root, .gitignore,
    // parent directory).
    let failures = [
        ("", (false, false, false)),
        ("git", (true, false, false)),
        ("git/github.com", (true, true, false)),
    ];
    for (suffix, (root, ignore, parent)) in failures {
        let fixture = Fixture::new(&json!({}));
        let Some(result) = locked(&fixture.agent(suffix), || {
            block_on(fixture.manager.install(source, None))
        }) else {
            return;
        };
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert!(fixture.calls().is_empty(), "case {suffix}");
        let exists = |path: &str| std::path::Path::new(&fixture.agent(path)).exists();
        assert_eq!(exists("git"), root, "root after {suffix}");
        assert_eq!(exists("git/.gitignore"), ignore, "ignore after {suffix}");
        assert_eq!(
            exists("git/github.com/user"),
            parent,
            "parent after {suffix}"
        );
        assert!(!exists("git/github.com/user/repo"));
    }
}

#[test]
fn git_failures_stop_after_the_completed_effect() {
    let source = "https://github.com/user/repo#v1";
    let fixture = Fixture::new(&json!({}));
    fixture.script.results.borrow_mut().push_back(Ok(Some(128)));
    assert!(block_on(fixture.manager.install(source, None)).is_err());
    assert_eq!(fixture.calls().len(), 1);
    assert_eq!(sandbox::read(fixture.agent("git/.gitignore")), IGNORE);
    assert!(std::path::Path::new(&fixture.agent("git/github.com/user")).is_dir());
    assert!(!std::path::Path::new(&fixture.agent("git/github.com/user/repo")).exists());

    let fixture = Fixture::new(&json!({}));
    clone_creates_target(&fixture, true);
    fixture
        .script
        .results
        .borrow_mut()
        .extend([Ok(Some(0)), Ok(Some(1))]);
    let error = block_on(fixture.manager.install(source, None)).unwrap_err();
    assert!(error.to_string().ends_with("failed with code 1"));
    assert_eq!(fixture.calls().len(), 2);
    assert_eq!(sandbox::read(fixture.agent("git/.gitignore")), IGNORE);
    assert!(!std::path::Path::new(&fixture.agent("git/github.com/user/repo")).exists());

    let fixture = Fixture::new(&json!({}));
    clone_creates_target(&fixture, true);
    fixture
        .script
        .results
        .borrow_mut()
        .extend([Ok(Some(0)), Ok(Some(0)), Ok(Some(2))]);
    let error = block_on(fixture.manager.install(source, None)).unwrap_err();
    assert!(error.to_string().ends_with("failed with code 2"));
    assert_eq!(fixture.calls().len(), 3);
    assert!(
        !std::path::Path::new(&fixture.agent("git/github.com/user/repo/package.json")).exists()
    );
}

#[test]
fn local_install_uses_cwd_without_copying() {
    for scope in [None, Some(Project)] {
        let fixture = Fixture::new(&json!({}));
        native_support::write(fixture.project("file.txt"), "bytes");
        std::fs::create_dir_all(fixture.project("dir")).unwrap();
        let absolute = fixture.project("dir");
        for source in ["./file.txt", "  ./dir  ", "dir", &absolute, "", "   "] {
            block_on(fixture.manager.install(source, scope)).unwrap();
        }
        assert!(fixture.calls().is_empty());
        assert_eq!(sandbox::read(fixture.project("file.txt")), "bytes");
        assert!(!std::path::Path::new(&fixture.agent("")).exists());
        assert!(!std::path::Path::new(&fixture.project(".maestro")).exists());
    }
}

#[test]
fn local_missing_path_reports_resolved_error() {
    let fixture = Fixture::new(&json!({}));
    let error = block_on(fixture.manager.install("./absent", None)).unwrap_err();
    let message = format!("Path does not exist: {}", fixture.project("absent"));
    assert_eq!(error.to_string(), message);
    let events = fixture.events.borrow();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].r#type, Phase::Error);
    assert_eq!(events[1].message.as_deref(), Some(message.as_str()));
    assert_eq!(events[1].source, "./absent");
}

#[test]
fn local_remove_has_no_filesystem_or_command_effect() {
    let fixture = Fixture::new(&json!({}));
    native_support::write(fixture.project("here/file.txt"), "bytes");
    for source in ["./here", "./gone", "~/gone", "relative"] {
        block_on(fixture.manager.remove(source, None)).unwrap();
        block_on(fixture.manager.remove(source, Some(Project))).unwrap();
    }
    assert_eq!(sandbox::read(fixture.project("here/file.txt")), "bytes");
    assert!(fixture.calls().is_empty());
    let phases = fixture.phases();
    assert_eq!(phases.len(), 16);
    assert_eq!(phases[0], (Phase::Start, Some("Removing ./here...".into())));
    assert_eq!(phases[1], (Phase::Complete, None));
}

/// Marks the child process of the offline-policy test.
const OFFLINE_CHILD: &str = "MAESTRO_ACQUISITION_OFFLINE_CHILD";

#[test]
fn explicit_install_does_not_obey_offline_resolution_policy() {
    if std::env::var_os(OFFLINE_CHILD).is_some() {
        let fixture = Fixture::new(&json!({}));
        block_on(fixture.manager.install("npm:pkg", None)).unwrap();
        assert_eq!(fixture.calls(), [call("npm", &["install", "-g", "pkg"])]);
        return;
    }
    let output = maestro_test_process::TestProcess::current(
        "explicit_install_does_not_obey_offline_resolution_policy",
    )
    .unwrap()
    .env(OFFLINE_CHILD, "1")
    .env("MAESTRO_OFFLINE", "yes")
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn npm_removal_uses_name_and_skips_missing_project_root() {
    let fixture = Fixture::new(&json!({}));
    block_on(fixture.manager.remove("npm:@s/pkg@1", None)).unwrap();
    assert_eq!(
        fixture.calls(),
        [call("npm", &["uninstall", "-g", "@s/pkg"])]
    );

    let fixture = Fixture::new(&json!({"npmCommand": [""]}));
    block_on(fixture.manager.remove("npm:pkg", Some(Project))).unwrap();
    assert!(fixture.calls().is_empty());
    assert!(!std::path::Path::new(&fixture.project("")).exists());

    let fixture = Fixture::new(&json!({}));
    let root = fixture.project(".maestro/npm");
    std::fs::create_dir_all(&root).unwrap();
    block_on(fixture.manager.remove("npm:pkg@2", Some(Project))).unwrap();
    assert_eq!(
        fixture.calls(),
        [call("npm", &["uninstall", "pkg", "--prefix", &root])]
    );
    assert!(!std::path::Path::new(&format!("{root}/package.json")).exists());
}

#[test]
fn git_removal_prunes_only_empty_parents_inside_root() {
    let source = "https://github.com/user/repo";
    let fixture = Fixture::new(&json!({}));
    let root = fixture.agent("git");
    native_support::write(format!("{root}/.gitignore"), IGNORE);
    native_support::write(format!("{root}/github.com/user/repo/file"), "x");
    native_support::write(format!("{root}/github.com/other/keep/file"), "x");
    block_on(fixture.manager.remove(source, None)).unwrap();
    assert!(!std::path::Path::new(&format!("{root}/github.com/user")).exists());
    assert_eq!(
        sandbox::read(format!("{root}/github.com/other/keep/file")),
        "x"
    );

    let fixture = Fixture::new(&json!({}));
    let root = fixture.agent("git");
    native_support::write(format!("{root}/.gitignore"), IGNORE);
    native_support::write(format!("{root}/github.com/user/repo/file"), "x");
    block_on(fixture.manager.remove(source, None)).unwrap();
    assert!(!std::path::Path::new(&format!("{root}/github.com")).exists());
    assert_eq!(sandbox::read(format!("{root}/.gitignore")), IGNORE);

    let fixture = Fixture::new(&json!({}));
    let root = fixture.agent("git");
    native_support::write(format!("{root}/github.com/user/sibling/file"), "x");
    block_on(fixture.manager.remove(source, None)).unwrap();
    assert!(std::path::Path::new(&format!("{root}/github.com/user/sibling/file")).exists());
}

#[cfg(unix)]
#[test]
fn git_removal_keeps_dangling_links_and_link_referents() {
    use std::os::unix::fs::symlink;
    let source = "https://github.com/user/repo";
    let fixture = Fixture::new(&json!({}));
    let target = fixture.agent("git/github.com/user/repo");
    std::fs::create_dir_all(fixture.agent("git/github.com/user")).unwrap();
    symlink(fixture.path("absent"), &target).unwrap();
    block_on(fixture.manager.remove(source, None)).unwrap();
    assert!(std::fs::symlink_metadata(&target).is_ok());

    let fixture = Fixture::new(&json!({}));
    let target = fixture.agent("git/github.com/user/repo");
    native_support::write(fixture.path("referent/file"), "kept");
    std::fs::create_dir_all(fixture.agent("git/github.com/user")).unwrap();
    symlink(fixture.path("referent"), &target).unwrap();
    block_on(fixture.manager.remove(source, None)).unwrap();
    assert!(std::fs::symlink_metadata(&target).is_err());
    assert_eq!(sandbox::read(fixture.path("referent/file")), "kept");
}

#[cfg(unix)]
#[test]
fn git_pruning_distinguishes_read_and_remove_failures() {
    let source = "https://github.com/user/repo";
    let build = || {
        let fixture = Fixture::new(&json!({}));
        native_support::write(fixture.agent("git/github.com/user/repo/file"), "x");
        fixture
    };
    let fixture = build();
    let Some(result) = locked(&fixture.agent("git/github.com/user/repo"), || {
        block_on(fixture.manager.remove(source, None))
    }) else {
        return;
    };
    assert!(result.is_err());
    assert!(std::path::Path::new(&fixture.agent("git/github.com/user/repo/file")).exists());
    assert_eq!(fixture.phases().last().unwrap().0, Phase::Error);

    let fixture = build();
    let parent = fixture.agent("git/github.com/user");
    // Write and search permission without read: the checkout goes, the listing fails.
    let Some(restore) = maestro_test_directory::deny(&parent, 0o300).unwrap() else {
        return;
    };
    let result = block_on(fixture.manager.remove(source, None));
    drop(restore);
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
    assert!(!std::path::Path::new(&fixture.agent("git/github.com/user/repo")).exists());
    assert!(std::path::Path::new(&parent).exists());

    let fixture = build();
    let Some(result) = locked(&fixture.agent("git/github.com"), || {
        block_on(fixture.manager.remove(source, None))
    }) else {
        return;
    };
    result.unwrap();
    assert!(!std::path::Path::new(&fixture.agent("git/github.com/user/repo")).exists());
    assert!(std::path::Path::new(&fixture.agent("git/github.com/user")).exists());
    assert!(std::path::Path::new(&fixture.agent("git/github.com")).exists());
}

/// A callback that fails on the chosen phases and records every event.
fn failing_on(fixture: &Fixture, failing: &'static [Phase]) {
    let events = fixture.events.clone();
    fixture
        .manager
        .set_progress_callback(Some(Rc::new(move |event| {
            events.borrow_mut().push(event.clone());
            if failing.contains(&event.r#type) {
                Err(io::Error::other(format!(
                    "{:?} callback failed",
                    event.r#type
                )))
            } else {
                Ok(())
            }
        })));
}

/// Runs one explicit operation through the trait object.
fn operate(fixture: &Fixture, remove: bool) -> io::Result<()> {
    let manager: &dyn PackageManager = &*fixture.manager;
    block_on(if remove {
        manager.remove("npm:pkg", None)
    } else {
        manager.install("npm:pkg", None)
    })
}

#[test]
fn progress_callback_errors_follow_authored_catch_boundaries() {
    for remove in [false, true] {
        callback_failures(remove);
    }
}

/// Exercises every callback failure boundary of one operation.
fn callback_failures(remove: bool) {
    let verb = if remove { "Removing" } else { "Installing" };
    let start = (Phase::Start, Some(format!("{verb} npm:pkg...")));

    let fixture = Fixture::new(&json!({}));
    failing_on(&fixture, &[Phase::Start]);
    let error = operate(&fixture, remove).unwrap_err();
    assert_eq!(error.to_string(), "Start callback failed");
    assert!(fixture.calls().is_empty());
    assert_eq!(fixture.phases(), std::slice::from_ref(&start));

    let fixture = Fixture::new(&json!({}));
    fixture.script.results.borrow_mut().push_back(Ok(Some(3)));
    let error = operate(&fixture, remove).unwrap_err().to_string();
    assert!(error.ends_with("failed with code 3"));
    assert_eq!(
        fixture.phases(),
        [start.clone(), (Phase::Error, Some(error))]
    );
    let action = if remove {
        ProgressAction::Remove
    } else {
        ProgressAction::Install
    };
    let events = fixture.events.borrow();
    assert!(
        events
            .iter()
            .all(|event| event.action == action && event.source == "npm:pkg")
    );

    let fixture = Fixture::new(&json!({}));
    failing_on(&fixture, &[Phase::Complete]);
    let error = operate(&fixture, remove).unwrap_err();
    assert_eq!(error.to_string(), "Complete callback failed");
    assert_eq!(fixture.calls().len(), 1);
    let failed = (Phase::Error, Some("Complete callback failed".into()));
    assert_eq!(fixture.phases(), [start, (Phase::Complete, None), failed]);

    let fixture = Fixture::new(&json!({}));
    failing_on(&fixture, &[Phase::Error]);
    fixture.script.results.borrow_mut().push_back(Ok(Some(3)));
    let error = operate(&fixture, remove).unwrap_err();
    assert_eq!(error.to_string(), "Error callback failed");

    let fixture = Fixture::new(&json!({}));
    fixture.manager.set_progress_callback(None);
    operate(&fixture, remove).unwrap();
    assert_eq!(fixture.calls().len(), 1);
    assert!(fixture.events.borrow().is_empty());
}

#[test]
fn progress_listener_replacement_is_live_for_same_operation() {
    let fixture = Fixture::new(&json!({}));
    let second = sandbox::Events::default();
    let (manager, replacement) = (Rc::downgrade(&fixture.manager), sandbox::recorder(&second));
    fixture
        .manager
        .set_progress_callback(Some(Rc::new(move |_| {
            manager
                .upgrade()
                .unwrap()
                .set_progress_callback(Some(replacement.clone()));
            Ok(())
        })));
    block_on(fixture.manager.install("npm:pkg", None)).unwrap();
    assert_eq!(second.borrow().len(), 1);
    assert_eq!(second.borrow()[0].r#type, Phase::Complete);

    let fixture = Fixture::new(&json!({}));
    let manager = Rc::downgrade(&fixture.manager);
    fixture
        .manager
        .set_progress_callback(Some(Rc::new(move |_| {
            manager.upgrade().unwrap().set_progress_callback(None);
            Ok(())
        })));
    block_on(fixture.manager.install("npm:pkg", None)).unwrap();

    for clear in [false, true] {
        let fixture = Fixture::new(&json!({}));
        fixture.script.held.set(true);
        let mut operation = fixture.manager.install("npm:pkg", None);
        assert!(poll_once(&mut operation).is_none());
        assert_eq!(fixture.calls().len(), 1);
        let late = sandbox::Events::default();
        fixture
            .manager
            .set_progress_callback((!clear).then(|| sandbox::recorder(&late)));
        fixture.script.held.set(false);
        poll_once(&mut operation).unwrap().unwrap();
        assert_eq!(fixture.phases().len(), 1);
        assert_eq!(late.borrow().len(), usize::from(!clear));
    }
}

#[test]
fn replaced_progress_callback_releases_its_last_handle() {
    struct Sentinel(Rc<Cell<bool>>);
    impl Drop for Sentinel {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let fixture = Fixture::new(&json!({}));
    let dropped = Rc::new(Cell::new(false));
    let alive_in_flight = Rc::new(Cell::new(false));
    let (sentinel, flag, weak) = (
        Sentinel(dropped.clone()),
        alive_in_flight.clone(),
        Rc::downgrade(&fixture.manager),
    );
    let callback: ProgressCallback = Rc::new(move |_| {
        let _ = &sentinel;
        weak.upgrade().unwrap().set_progress_callback(None);
        flag.set(!sentinel.0.get());
        Ok(())
    });
    let alias = callback.clone();
    fixture.manager.set_progress_callback(Some(callback));
    block_on(fixture.manager.install("npm:pkg", None)).unwrap();
    assert!(alive_in_flight.get());
    assert!(!dropped.get());
    alias(&maestro_packages::ProgressEvent {
        r#type: Phase::Progress,
        action: ProgressAction::Pull,
        source: "alias".into(),
        message: None,
    })
    .unwrap();
    drop(alias);
    assert!(dropped.get());
}

#[test]
fn replaced_progress_callback_without_alias_is_dropped_after_its_call_returns() {
    struct Sentinel(Rc<Cell<bool>>);
    impl Drop for Sentinel {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let fixture = Fixture::new(&json!({}));
    let dropped = Rc::new(Cell::new(false));
    let dropped_in_flight = Rc::new(Cell::new(true));
    let (sentinel, seen, weak) = (
        Sentinel(dropped.clone()),
        dropped_in_flight.clone(),
        Rc::downgrade(&fixture.manager),
    );
    fixture
        .manager
        .set_progress_callback(Some(Rc::new(move |_| {
            weak.upgrade().unwrap().set_progress_callback(None);
            seen.set(sentinel.0.get());
            Ok(())
        })));
    block_on(fixture.manager.install("npm:pkg", None)).unwrap();
    assert!(!dropped_in_flight.get());
    assert!(dropped.get());
}

/// Packages of the global settings.
fn stored(fixture: &Fixture) -> serde_json::Value {
    fixture
        .settings
        .borrow()
        .get_global_settings()
        .0
        .get("packages")
        .cloned()
        .unwrap_or(json!(null))
}

#[test]
fn manager_aliases_remain_usable_during_acquisition() {
    check_manager_aliases_during_acquisition(false);
}

/// Admission preserves callback reentry and held acquisition edits.
#[test]
fn admitted_manager_aliases_remain_usable_during_acquisition() {
    check_manager_aliases_during_acquisition(true);
}

/// Exercises live edits with and without an installed publisher.
fn check_manager_aliases_during_acquisition(admitted: bool) {
    let fixture = Fixture::new(&json!({"packages": ["npm:listed"]}));
    allow_package_edits(&fixture, admitted);
    let weak = Rc::downgrade(&fixture.manager);
    let uses = Rc::new(Cell::new(0));
    let counter = uses.clone();
    let replaced = sandbox::Events::default();
    let (completions, log, weak_again) = (Rc::new(Cell::new(0)), replaced.clone(), weak.clone());
    let completed = completions.clone();
    let replacement: ProgressCallback = Rc::new(move |event| {
        log.borrow_mut().push(event.clone());
        if event.r#type == Phase::Complete {
            let manager = weak_again.upgrade().unwrap();
            assert_eq!(manager.list_configured_packages()?.len(), 2);
            assert!(manager.get_installed_path("npm:x", User).is_ok());
            completed.set(completed.get() + 1);
        }
        Ok(())
    });
    fixture
        .manager
        .set_progress_callback(Some(Rc::new(move |event| {
            let manager = weak.upgrade().unwrap();
            if event.r#type == Phase::Start {
                assert_eq!(manager.list_configured_packages()?.len(), 1);
                assert!(manager.get_installed_path("npm:x", User).is_ok());
                assert!(manager.add_source_to_settings("npm:during", None)?);
                manager.set_progress_callback(Some(replacement.clone()));
                counter.set(counter.get() + 1);
            }
            Ok(())
        })));
    fixture.script.held.set(true);
    let mut operation = fixture.manager.install_and_persist("npm:pkg", None);
    assert!(poll_once(&mut operation).is_none());
    assert_eq!(fixture.calls().len(), 1);
    assert_eq!(uses.get(), 1);
    assert_eq!(fixture.manager.list_configured_packages().unwrap().len(), 2);
    assert!(fixture.manager.get_installed_path("npm:x", User).is_ok());
    assert!(
        fixture
            .manager
            .remove_source_from_settings("npm:during", None)
            .unwrap()
    );
    assert!(
        fixture
            .manager
            .add_source_to_settings("npm:held", None)
            .unwrap()
    );
    fixture.script.held.set(false);
    poll_once(&mut operation).unwrap().unwrap();
    assert_eq!(
        stored(&fixture),
        json!(["npm:listed", "npm:held", "npm:pkg"])
    );
    assert_eq!(completions.get(), 1);
    assert_eq!(replaced.borrow().len(), 1);
    assert_eq!(fixture.script.captures.borrow().len(), 1);
}

#[test]
fn root_capture_does_not_hold_the_cache_across_reentry() {
    let fixture = Fixture::new(&json!({}));
    let weak = Rc::downgrade(&fixture.manager);
    *fixture.script.on_capture.borrow_mut() = Some(Box::new(move || {
        let inner = weak.upgrade().unwrap().get_installed_path("npm:pkg", User);
        assert!(inner.is_ok());
    }));
    fixture.manager.get_installed_path("npm:pkg", User).unwrap();
    assert_eq!(fixture.script.captures.borrow().len(), 2);
    fixture
        .manager
        .get_installed_path("npm:other", User)
        .unwrap();
    assert_eq!(fixture.script.captures.borrow().len(), 2);
}

#[test]
fn persistence_happens_after_completed_acquisition() {
    let fixture = Fixture::new(&json!({}));
    let settings = fixture.settings.clone();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    fixture
        .manager
        .set_progress_callback(Some(Rc::new(move |event| {
            let has = settings
                .borrow()
                .get_global_settings()
                .0
                .contains_key("packages");
            log.borrow_mut().push((event.r#type, has));
            Ok(())
        })));
    let manager: &dyn PackageManager = &*fixture.manager;
    block_on(manager.install_and_persist("npm:pkg", None)).unwrap();
    assert_eq!(
        *seen.borrow(),
        [(Phase::Start, false), (Phase::Complete, false)]
    );
    assert_eq!(stored(&fixture), json!(["npm:pkg"]));
    block_on(manager.install_and_persist("npm:pkg", None)).unwrap();
    assert_eq!(stored(&fixture), json!(["npm:pkg"]));
    assert!(block_on(manager.remove_and_persist("npm:pkg", None)).unwrap());
    assert!(!block_on(manager.remove_and_persist("npm:pkg", None)).unwrap());
    assert_eq!(stored(&fixture), json!([]));

    native_support::write(fixture.project("pkgdir/file"), "x");
    block_on(manager.install_and_persist("./pkgdir", Some(Project))).unwrap();
    let project = fixture.settings.borrow().get_project_settings().0["packages"].clone();
    // The stored source is the native relative path, as the reference stores it.
    let relative = format!("..{}pkgdir", std::path::MAIN_SEPARATOR);
    assert_eq!(project, json!([relative]));
    assert!(block_on(manager.remove_and_persist("./pkgdir", Some(Project))).unwrap());
    assert!(!block_on(manager.remove_and_persist("./pkgdir", Some(Project))).unwrap());
}

#[test]
fn persistence_failure_does_not_undo_completed_contents() {
    let source = "https://github.com/user/repo";
    let fixture = Fixture::new(&json!({"packages": [5]}));
    fixture
        .script
        .hooks
        .borrow_mut()
        .push_back(Box::new(|call| {
            let target = call.args.last().unwrap();
            native_support::write(format!("{target}/kept"), "cloned");
        }));
    let error = block_on(fixture.manager.install_and_persist(source, None)).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(fixture.calls().len(), 1);
    assert_eq!(
        sandbox::read(fixture.agent("git/github.com/user/repo/kept")),
        "cloned"
    );
    assert_eq!(fixture.phases().last().unwrap().0, Phase::Complete);
    assert_eq!(fixture.phases().len(), 2);

    let fixture = Fixture::new(&json!({"packages": [5]}));
    let target = fixture.agent("git/github.com/user/repo");
    native_support::write(format!("{target}/file"), "x");
    assert!(block_on(fixture.manager.remove_and_persist(source, None)).is_err());
    assert!(!std::path::Path::new(&target).exists());
    assert_eq!(fixture.phases().last().unwrap().0, Phase::Complete);
}

#[test]
fn progress_failure_prevents_later_settings_mutation() {
    let storage = Arc::new(Storage::default());
    let fixture = Fixture::with_storage(storage.clone());
    failing_on(&fixture, &[Phase::Complete]);
    assert!(block_on(fixture.manager.install_and_persist("npm:pkg", None)).is_err());
    assert!(block_on(fixture.manager.remove_and_persist("npm:pkg", None)).is_err());
    assert_eq!(fixture.calls().len(), 2);
    assert_eq!(stored(&fixture), json!(null));
    fixture.flush();
    assert_eq!(storage.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn failed_acquisition_does_not_publish_settings() {
    let storage = Arc::new(Storage::default());
    let fixture = Fixture::with_storage(storage.clone());
    block_on(fixture.manager.install_and_persist("npm:old", None)).unwrap();
    fixture.flush();
    assert_eq!(storage.writes.load(Ordering::SeqCst), 1);
    fixture
        .script
        .results
        .borrow_mut()
        .extend([Ok(Some(1)), Ok(Some(1))]);
    assert!(block_on(fixture.manager.install_and_persist("npm:new", None)).is_err());
    assert!(block_on(fixture.manager.remove_and_persist("npm:old", None)).is_err());
    assert_eq!(stored(&fixture), json!(["npm:old"]));
    fixture.flush();
    assert_eq!(storage.writes.load(Ordering::SeqCst), 1);
}

#[test]
fn persistence_reads_settings_after_acquisition() {
    check_persistence_after_acquisition(false);
}

/// Admitted installation and removal retain edits accepted while waiting.
#[test]
fn admitted_persistence_reads_settings_after_acquisition() {
    check_persistence_after_acquisition(true);
}

/// Exercises late reads without changing their original effect order.
fn check_persistence_after_acquisition(admitted: bool) {
    let fixture = Fixture::new(
        &json!({"packages": [{"source": "npm:keep", "extensions": ["x"], "extra": 1}]}),
    );
    allow_package_edits(&fixture, admitted);
    fixture.script.held.set(true);
    let mut operation = fixture.manager.install_and_persist("npm:added", None);
    assert!(poll_once(&mut operation).is_none());
    fixture.settings.borrow_mut().set_packages(vec![
        PackageSource::Source("npm:alias".into()),
        PackageSource::Unknown(json!({"source": "npm:keep", "extra": 2})),
    ]);
    fixture.script.held.set(false);
    poll_once(&mut operation).unwrap().unwrap();
    assert_eq!(
        stored(&fixture),
        json!(["npm:alias", {"source": "npm:keep", "extra": 2}, "npm:added"])
    );

    fixture.script.held.set(true);
    let mut operation = fixture.manager.remove_and_persist("npm:added", None);
    assert!(poll_once(&mut operation).is_none());
    fixture.settings.borrow_mut().set_packages(vec![
        PackageSource::Source("npm:added".into()),
        PackageSource::Source("npm:late".into()),
        PackageSource::Unknown(json!({"source": "npm:added", "tail": true})),
    ]);
    fixture.script.held.set(false);
    assert!(poll_once(&mut operation).unwrap().unwrap());
    assert_eq!(stored(&fixture), json!(["npm:late"]));
}

#[test]
fn persistence_publishes_without_waiting_for_flush() {
    let (storage, admission, release) = Storage::gated();
    let fixture = Fixture::with_storage(storage.clone());
    block_on(fixture.manager.install_and_persist("npm:pkg", None)).unwrap();
    admission.recv().unwrap();
    assert_eq!(stored(&fixture), json!(["npm:pkg"]));
    assert_eq!(storage.writes.load(Ordering::SeqCst), 0);
    assert_eq!(storage.document(), None);
    release.send(()).unwrap();
    fixture.flush();
    assert_eq!(storage.writes.load(Ordering::SeqCst), 1);
    assert!(storage.document().unwrap().contains("npm:pkg"));
    assert!(block_on(fixture.manager.remove_and_persist("npm:pkg", None)).unwrap());
    admission.recv().unwrap();
    assert_eq!(storage.writes.load(Ordering::SeqCst), 1);
    release.send(()).unwrap();
    fixture.flush();
    assert_eq!(storage.writes.load(Ordering::SeqCst), 2);
    assert!(!storage.document().unwrap().contains("npm:pkg"));
}

#[cfg(unix)]
#[test]
fn local_install_reads_ambient_paths_only_when_needed() {
    let removed = crate::resolution_support::case_in_removed_cwd("removed", |_| vec![]);
    if removed.is_none() {
        return;
    }
    let fixture = Fixture::new(&json!({}));
    let absolute = fixture.path("none");
    let error = block_on(fixture.manager.install(&absolute, None)).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("Path does not exist: {absolute}")
    );

    let relative = Fixture::over(
        maestro_settings::SettingsManager::in_memory(maestro_settings::Settings::default()),
        Some("relative"),
        false,
    );
    let error = block_on(relative.manager.install("./x", None)).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert_eq!(
        relative.phases().last().unwrap(),
        &(Phase::Error, Some(error.to_string()))
    );
}

/// Admission and release of one held write.
struct Gate {
    /// Signals that a write reached the storage.
    admitted: Mutex<Sender<()>>,
    /// Blocks the write until the test releases it.
    release: Mutex<Receiver<()>>,
}
/// Storage that counts published writes and can hold a write before it publishes.
#[derive(Default)]
struct Storage {
    /// The raw-text storage.
    inner: InMemorySettingsStorage,
    /// Writes published to the inner storage.
    writes: AtomicUsize,
    /// Holds every write until released, when present.
    gate: Option<Gate>,
}
impl Storage {
    /// Storage whose writes wait for the returned sender after signalling the receiver.
    fn gated() -> (Arc<Self>, Receiver<()>, Sender<()>) {
        let (admitted, admission) = channel();
        let (release, released) = channel();
        let storage = Self {
            gate: Some(Gate {
                admitted: Mutex::new(admitted),
                release: Mutex::new(released),
            }),
            ..Self::default()
        };
        (Arc::new(storage), admission, release)
    }
    /// Signals admission, then blocks until the test releases the write.
    fn hold_write(&self) {
        if let Some(gate) = &self.gate {
            gate.admitted.lock().unwrap().send(()).unwrap();
            gate.release.lock().unwrap().recv().unwrap();
        }
    }
    /// The global document as stored.
    fn document(&self) -> Option<String> {
        let mut text = None;
        self.inner
            .with_lock(SettingsScope::Global, &mut |current| {
                text = current.map(str::to_owned);
                Ok(None)
            })
            .unwrap();
        text
    }
}
impl SettingsStorage for Storage {
    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        let mut wrote = false;
        self.inner.with_lock(scope, &mut |current| {
            let next = update(current)?;
            if next.is_some() {
                wrote = true;
                self.hold_write();
            }
            Ok(next)
        })?;
        if wrote {
            self.writes.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

/// A project cleanup whose base is drive-relative reads the working directory once for both groups.
#[cfg(windows)]
#[test]
fn grouped_cleanup_keeps_drive_observation_policy() {
    let ambient = std::env::current_dir().unwrap();
    let scratch = scratch::Scratch::new_in(&ambient).unwrap();
    let root = scratch.path().to_string_lossy().replace('\\', "/");
    let ambient = ambient.to_string_lossy();
    if ambient.as_bytes().get(1) != Some(&b':') {
        return;
    }
    let none = maestro_path::Cwd {
        current: "",
        drive_directories: &[],
    };
    let relative = maestro_path::relative(&ambient, &format!("{root}/project"), &none);
    let letter = ambient.chars().next().unwrap().to_ascii_lowercase();
    let settings =
        maestro_settings::SettingsManager::in_memory(maestro_settings::Settings::default());
    let fixture = Fixture::over(settings, Some(&format!("{letter}:{relative}")), false);
    let git = format!("{root}/project/.maestro/git/github.com");
    native_support::write(format!("{git}/user/repo/file"), "x");
    native_support::write(format!("{git}/other/keep"), "x");
    block_on(
        fixture
            .manager
            .remove("https://github.com/user/repo", Some(Project)),
    )
    .unwrap();
    assert!(!std::path::Path::new(&format!("{git}/user")).exists());
    assert!(std::path::Path::new(&format!("{git}/other/keep")).is_file());
}

#[test]
fn git_parent_cleanup_uses_resolved_component_containment() {
    let hidden = parse_git_url("https://github.com/..hidden/repo").unwrap();
    assert_eq!(hidden.path, "..hidden/repo");

    let fixture = Fixture::relative_agent();
    native_support::write(fixture.agent("git/github.com/user/repo/file"), "x");
    block_on(fixture.manager.remove("https://github.com/user/repo", None)).unwrap();
    assert!(!std::path::Path::new(&fixture.agent("git/github.com")).exists());
    assert!(std::path::Path::new(&fixture.agent("git")).is_dir());

    // An absolute agent directory never needs the removed working directory.
    #[cfg(unix)]
    if crate::resolution_support::case_in_removed_cwd("absolute", |_| vec![]).is_some() {
        let fixture = Fixture::new(&json!({}));
        native_support::write(fixture.agent("git/github.com/user/repo/file"), "x");
        block_on(fixture.manager.remove("https://github.com/user/repo", None)).unwrap();
        assert!(!std::path::Path::new(&fixture.agent("git/github.com")).exists());
    }

    let fixture = Fixture::new(&json!({}));
    native_support::write(fixture.agent("git/github.com/..hidden/repo/file"), "x");
    block_on(
        fixture
            .manager
            .remove("https://github.com/..hidden/repo", None),
    )
    .unwrap();
    assert!(!std::path::Path::new(&fixture.agent("git/github.com")).exists());
    assert!(std::path::Path::new(&fixture.agent("git")).is_dir());
}

#[test]
fn git_dot_segments_never_select_a_checkout() {
    for (source, selected) in [
        ("git:example.com/a/../../../sibling/repo", "sibling/repo"),
        ("git:example.com/owner/./repo", "git/example.com/owner/repo"),
        (
            "git:git@github.com:../../git-cache/team/repo",
            "git-cache/team/repo",
        ),
        ("https://github.com/..%2F..%2Fsibling/repo", "sibling/repo"),
        ("git:../owner/repo", "owner/repo"),
        ("git:git@..:owner/repo", "owner/repo"),
    ] {
        rejected_git_source_preserves_checkout(source, selected);
    }
    for (source, path) in [
        ("git:example.com/group/sub/repo", "group/sub/repo"),
        ("git:example.com/owner/repo/", "owner/repo/"),
        ("git:example.com/owner//repo", "owner//repo"),
    ] {
        assert_eq!(parse_git_url(source).unwrap().path, path, "case {source:?}");
    }
}

/// Exercises each operation after source admission with an existing checkout sentinel.
fn rejected_git_source_preserves_checkout(source: &str, selected: &str) {
    assert_eq!(parse_git_url(source), None, "case {source:?}");
    let fixture = Fixture::new(&json!({"packages": [source]}));
    let file = fixture.agent(&format!("{selected}/file"));
    native_support::write(&file, "x");
    assert!(block_on(fixture.manager.install(source, None)).is_err());
    assert!(std::path::Path::new(&file).is_file(), "install {source:?}");
    assert!(fixture.calls().is_empty(), "install {source:?}");
    block_on(Rc::clone(&fixture.manager).update(None)).unwrap();
    assert!(std::path::Path::new(&file).is_file(), "update {source:?}");
    assert!(fixture.calls().is_empty(), "update {source:?}");
    block_on(fixture.manager.remove(source, None)).unwrap();
    assert!(std::path::Path::new(&file).is_file(), "case {source:?}");
    assert!(fixture.calls().is_empty(), "case {source:?}");
}

#[cfg(windows)]
#[test]
fn git_windows_dot_segments_never_select_a_checkout() {
    for (source, selected) in [
        (r"git:example.com/a\..\..\..\sibling/repo", "sibling/repo"),
        (r"git:..\../owner/repo", "../owner/repo"),
        ("https://github.com/..%5C..%5Csibling/repo", "sibling/repo"),
    ] {
        rejected_git_source_preserves_checkout(source, selected);
    }
}

/// Refuses the package preference edit before a managed source can run.
struct RefusePackageEdit;
impl maestro_settings::SettingsDecider for RefusePackageEdit {
    fn decide(&self, _: &maestro_settings::DecisionRequest<'_>) -> Result<(), String> {
        Err("release refuses packages".into())
    }
}

/// Allows package intent but refuses its derived preference edit.
struct RefusePackageSettingsEdit;
impl maestro_settings::SettingsDecider for RefusePackageSettingsEdit {
    fn decide(&self, request: &maestro_settings::DecisionRequest<'_>) -> Result<(), String> {
        if matches!(
            request,
            maestro_settings::DecisionRequest::SettingsEdit { .. }
        ) {
            Err("release refuses package settings".into())
        } else {
            Ok(())
        }
    }
}

/// No child, progress callback or project file precedes a refused package edit.
#[test]
fn refused_package_edits_have_no_acquisition_or_removal_effects() {
    for scope in [User, Project] {
        for remove in [false, true] {
            let fixture = Fixture::new(&json!({"packages":["npm:pkg"]}));
            fixture
                .settings
                .borrow_mut()
                .register_decider(Arc::new(RefusePackageSettingsEdit))
                .unwrap();
            let before = fixture.settings.borrow().get_global_settings();
            let result = if remove {
                block_on(fixture.manager.remove_and_persist("npm:pkg", Some(scope))).map(drop)
            } else {
                block_on(fixture.manager.install_and_persist("npm:pkg", Some(scope)))
            };
            assert_eq!(
                result.unwrap_err().to_string(),
                "release refuses package settings"
            );
            assert!(fixture.calls().is_empty());
            assert!(fixture.phases().is_empty());
            assert!(!std::path::Path::new(&fixture.project(".maestro/npm")).exists());
            assert_eq!(fixture.settings.borrow().get_global_settings(), before);
        }
    }
}

/// A scoped persisted policy with one required package and optional package lock.
fn package_layer(locked: bool) -> maestro_settings::BaseSettingsLayer {
    maestro_settings::BaseSettingsLayer {
        settings: serde_json::from_value(json!({"packages":["npm:pkg@1"]})).unwrap(),
        locked: if locked {
            ["packages".into()].into()
        } else {
            std::collections::BTreeSet::default()
        },
        source: "organization@1".into(),
        recorded_at: "2026-10-28".into(),
    }
}

/// Every public acquisition/removal surface checks its governing layer before progress.
#[test]
fn package_policy_checks_scope_and_required_identity_before_effects() {
    for (project_layer, scope) in [
        (false, User),
        (false, Project),
        (true, User),
        (true, Project),
    ] {
        for (persist, remove) in [(false, false), (false, true), (true, false), (true, true)] {
            let fixture = Fixture::new(&json!({"packages":["npm:pkg@2"]}));
            fixture
                .settings
                .borrow_mut()
                .publish_base_layer(SettingsScope::Global, Some(package_layer(true)));
            if project_layer {
                let mut layer = package_layer(false);
                layer.settings = maestro_settings::Settings::default();
                fixture
                    .settings
                    .borrow_mut()
                    .publish_base_layer(SettingsScope::Project, Some(layer));
            }
            let source = "npm:pkg@2";
            let result = match (persist, remove) {
                (true, true) => {
                    block_on(fixture.manager.remove_and_persist(source, Some(scope))).map(drop)
                }
                (true, false) => block_on(fixture.manager.install_and_persist(source, Some(scope))),
                (false, true) => block_on(fixture.manager.remove(source, Some(scope))),
                (false, false) => block_on(fixture.manager.install(source, Some(scope))),
            };
            if project_layer && scope == Project {
                result.unwrap();
            } else {
                let error = result.unwrap_err().to_string();
                assert!(error.contains("organization@1"), "{error}");
                assert!(error.contains("2026-10-28"), "{error}");
                assert!(fixture.calls().is_empty());
                assert!(fixture.phases().is_empty());
                assert!(!std::path::Path::new(&fixture.project(".maestro/npm")).exists());
            }
        }
    }
}

/// Direct SDK operations also ask the installed decider before emitting progress.
#[test]
fn direct_package_operations_ask_decider_before_effects() {
    let fixture = Fixture::new(&json!({}));
    fixture
        .settings
        .borrow_mut()
        .register_decider(Arc::new(RefusePackageEdit))
        .unwrap();
    for remove in [false, true] {
        let result = if remove {
            block_on(fixture.manager.remove("npm:pkg", None))
        } else {
            block_on(fixture.manager.install("npm:pkg", None))
        };
        assert_eq!(result.unwrap_err().to_string(), "release refuses packages");
        assert!(fixture.calls().is_empty());
        assert!(fixture.phases().is_empty());
    }
}

/// Allows explicit package operations and their preference edits.
struct AllowPackageEdit;
impl maestro_settings::SettingsDecider for AllowPackageEdit {
    fn decide(&self, _: &maestro_settings::DecisionRequest<'_>) -> Result<(), String> {
        Ok(())
    }
}

/// A current checkout publishes the requested spelling whether already configured or absent.
#[test]
fn admitted_current_git_checkout_publishes_requested_source() {
    for configured in [false, true] {
        let initial = if configured {
            json!({"packages":["git:github.com/user/repo#kept"]})
        } else {
            json!({})
        };
        let fixture = Fixture::new(&initial);
        native_support::write(fixture.agent("git/github.com/user/repo/kept"), "contents");
        fixture
            .settings
            .borrow_mut()
            .register_decider(Arc::new(AllowPackageEdit))
            .unwrap();
        let effects = Rc::new(crate::support::Effects::default());
        *effects.capture_outputs.borrow_mut() = ["origin/main", "tip", "same", "same"]
            .map(|text| Ok(text.to_owned()))
            .into();
        let manager = maestro_packages::DefaultPackageManager::new(
            maestro_packages::PackageManagerOptions {
                cwd: fixture.project(""),
                agent_dir: fixture.agent(""),
                settings_manager: fixture.settings.clone(),
            },
            crate::support::Controlled(effects.clone()),
        );

        block_on(manager.install_and_persist("git:github.com/user/repo", None)).unwrap();

        assert_eq!(stored(&fixture), json!(["git:github.com/user/repo"]));
        let verbs: Vec<_> = effects
            .calls
            .borrow()
            .iter()
            .map(|(_, args)| args[0].clone())
            .collect();
        assert_eq!(
            verbs,
            ["rev-parse", "rev-parse", "fetch", "rev-parse", "rev-parse"]
        );
        assert_eq!(
            sandbox::read(fixture.agent("git/github.com/user/repo/kept")),
            "contents"
        );
    }
}

/// An admitted equivalent spelling replaces its row without changing its pin or filters.
#[test]
fn admitted_source_spelling_replaces_same_pin_and_preserves_rows() {
    for scope in [User, Project] {
        let source = "npm: pkg@1";
        let entries = json!([
            "npm:before",
            {"source":"npm:pkg@1", "skills":[], "unknown":7},
            "npm:after"
        ]);
        let fixture = Fixture::new(&json!({"packages":entries}));
        if scope == Project {
            fixture.settings.borrow_mut().set_project_packages(
                entries
                    .as_array()
                    .unwrap()
                    .iter()
                    .cloned()
                    .map(PackageSource::Unknown)
                    .collect(),
            );
        }
        fixture
            .settings
            .borrow_mut()
            .register_decider(Arc::new(AllowPackageEdit))
            .unwrap();

        block_on(fixture.manager.install_and_persist(source, Some(scope))).unwrap();

        let settings = fixture.settings.borrow();
        let updated = match scope {
            User => settings.get_global_settings(),
            Project => settings.get_project_settings(),
        };
        assert_eq!(
            updated.0["packages"],
            json!([
                "npm:before",
                {"source":source, "skills":[], "unknown":7},
                "npm:after"
            ])
        );
        if scope == Project {
            assert_eq!(settings.get_global_settings().0["packages"], entries);
        }
    }
}

/// Captures package intent and its exact derived edit in one ordered observation.
struct OrderedPackageDecider(Arc<Mutex<Vec<&'static str>>>);
impl maestro_settings::SettingsDecider for OrderedPackageDecider {
    fn decide(&self, request: &maestro_settings::DecisionRequest<'_>) -> Result<(), String> {
        let event = match request {
            maestro_settings::DecisionRequest::PackageInstall { .. } => "intent",
            maestro_settings::DecisionRequest::SettingsEdit { .. } => "edit",
            _ => "other",
        };
        self.0.lock().unwrap().push(event);
        Ok(())
    }
}

/// Admission precedes progress; allowed contents still finish before preferences publish.
#[test]
fn allowed_package_admission_keeps_progress_before_preference_publication() {
    for registered in [false, true] {
        let fixture = Fixture::new(&json!({}));
        let events = Arc::new(Mutex::new(Vec::new()));
        if registered {
            fixture
                .settings
                .borrow_mut()
                .register_decider(Arc::new(OrderedPackageDecider(events.clone())))
                .unwrap();
        }
        let log = events.clone();
        let settings = fixture.settings.clone();
        fixture
            .manager
            .set_progress_callback(Some(Rc::new(move |event| {
                assert!(
                    !settings
                        .borrow()
                        .get_global_settings()
                        .0
                        .contains_key("packages")
                );
                log.lock().unwrap().push(if event.r#type == Phase::Start {
                    "start"
                } else {
                    "complete"
                });
                Ok(())
            })));
        block_on(fixture.manager.install_and_persist("npm:pkg", None)).unwrap();
        assert_eq!(
            *events.lock().unwrap(),
            if registered {
                vec!["intent", "edit", "start", "complete"]
            } else {
                vec!["start", "complete"]
            }
        );
        assert_eq!(stored(&fixture), json!(["npm:pkg"]));
    }
}

/// Borrow contention is recoverable before either explicit package operation.
#[test]
fn package_admission_returns_borrow_errors_before_effects() {
    for remove in [false, true] {
        let fixture = Fixture::new(&json!({}));
        let _guard = fixture.settings.borrow_mut();
        let result = if remove {
            block_on(fixture.manager.remove("npm:pkg", None))
        } else {
            block_on(fixture.manager.install("npm:pkg", None))
        };
        assert!(result.is_err());
        assert!(fixture.calls().is_empty());
        assert!(fixture.phases().is_empty());
    }
}

/// A shared guard retained during acquisition returns an error at publication.
#[test]
fn admitted_package_publication_returns_borrow_errors() {
    for remove in [false, true] {
        let fixture = Fixture::new(&json!({"packages":["npm:pkg"]}));
        fixture
            .settings
            .borrow_mut()
            .register_decider(Arc::new(AllowPackageEdit))
            .unwrap();
        fixture.script.held.set(true);
        let mut operation = if remove {
            fixture.manager.remove_and_persist("npm:pkg", None)
        } else {
            Box::pin(async {
                fixture
                    .manager
                    .install_and_persist("npm:new", None)
                    .await
                    .map(|()| true)
            })
        };
        assert!(poll_once(&mut operation).is_none());
        let guard = fixture.settings.borrow();
        fixture.script.held.set(false);
        assert!(poll_once(&mut operation).unwrap().is_err());
        assert_eq!(
            guard.get_global_settings().0.get("packages"),
            Some(&json!(["npm:pkg"]))
        );
    }
}

/// Installs the same allowing publisher only for the admitted variants.
fn allow_package_edits(fixture: &Fixture, admitted: bool) {
    if admitted {
        fixture
            .settings
            .borrow_mut()
            .register_decider(Arc::new(AllowPackageEdit))
            .unwrap();
    }
}

/// Admission covers the intended delta even when preflight finds it already satisfied.
#[test]
fn admitted_noop_package_intents_recompute_after_acquisition() {
    for admitted in [false, true] {
        check_noop_package_intents(admitted);
    }
}

/// Exercises each satisfied intent while another operation changes its entry.
fn check_noop_package_intents(admitted: bool) {
    for remove in [false, true] {
        let initial = if remove {
            json!([])
        } else {
            json!(["npm:pkg"])
        };
        let fixture = Fixture::new(&json!({"packages": initial}));
        allow_package_edits(&fixture, admitted);
        fixture.script.held.set(true);
        let mut operation = if remove {
            fixture.manager.remove_and_persist("npm:pkg", None)
        } else {
            Box::pin(async {
                fixture
                    .manager
                    .install_and_persist("npm:pkg", None)
                    .await
                    .map(|()| true)
            })
        };
        assert!(poll_once(&mut operation).is_none());
        fixture.settings.borrow_mut().set_packages(if remove {
            vec![PackageSource::Source("npm:pkg".into())]
        } else {
            Vec::new()
        });
        fixture.script.held.set(false);
        assert!(poll_once(&mut operation).unwrap().unwrap());
        assert_eq!(stored(&fixture), initial);
    }
}

/// A publisher can forbid an explicit install even when the entry already exists.
#[test]
fn forbidden_existing_package_install_has_no_effects() {
    let fixture = Fixture::new(&json!({"packages":["npm:pkg"]}));
    fixture
        .settings
        .borrow_mut()
        .register_decider(Arc::new(RefusePackageEdit))
        .unwrap();
    assert!(block_on(fixture.manager.install_and_persist("npm:pkg", None)).is_err());
    assert!(fixture.calls().is_empty());
    assert!(fixture.phases().is_empty());
    assert_eq!(stored(&fixture), json!(["npm:pkg"]));
}
