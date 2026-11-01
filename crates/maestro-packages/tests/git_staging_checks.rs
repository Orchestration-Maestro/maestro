//! Retry-safe acquisition at filesystem, command and settings boundaries.
use crate::sandbox::{Fixture, block_on};
use maestro_packages::PackageManager;
use serde_json::json;
use std::{io, path::Path};

#[test]
fn git_missing_preparation_is_not_published_at_command_failure() {
    for phase in 0..3 {
        missing_command_boundary(phase);
    }
}

/// Observes the real preparation operands at clone, checkout and dependency boundaries.
fn missing_command_boundary(phase: usize) {
    let fixture = Fixture::new(&json!({}));
    let target = fixture.agent("git/example.test/team/repo");
    for _ in 0..=phase {
        let observed = target.clone();
        fixture
            .script
            .hooks
            .borrow_mut()
            .push_back(Box::new(move |call| {
                let preparation = call
                    .cwd
                    .as_ref()
                    .unwrap_or_else(|| call.args.last().unwrap());
                assert_ne!(
                    preparation, &observed,
                    "preparation must run outside live target"
                );
                assert!(!Path::new(&observed).exists());
                std::fs::write(Path::new(preparation).join("package.json"), "{}").unwrap();
            }));
    }
    fixture
        .script
        .results
        .borrow_mut()
        .extend((0..phase).map(|_| Ok(Some(0))));
    fixture
        .script
        .results
        .borrow_mut()
        .push_back(Err(io::Error::other("preparation failed")));
    assert!(
        block_on(
            fixture
                .manager
                .install("https://example.test/team/repo@v1", None)
        )
        .is_err()
    );
    assert!(!Path::new(&target).exists());
}

#[test]
fn git_missing_install_cleans_failed_stage_and_retries() {
    for phase in 0..3 {
        missing_stage_failure_retries(phase);
    }
    #[cfg(unix)]
    missing_invalid_ref_retries_until_available();
}

/// Retries each preparation failure only after its owned stage has been removed.
fn missing_stage_failure_retries(phase: usize) {
    let fixture = Fixture::new(&json!({}));
    for _ in 0..=phase {
        fixture
            .script
            .hooks
            .borrow_mut()
            .push_back(Box::new(write_stage_manifest));
    }
    fixture
        .script
        .results
        .borrow_mut()
        .extend((0..phase).map(|_| Ok(Some(0))));
    fixture
        .script
        .results
        .borrow_mut()
        .push_back(Err(io::Error::other("preparation failed")));
    let source = "https://example.test/team/repo@v1";
    assert!(block_on(fixture.manager.install_and_persist(source, None)).is_err());
    let calls = fixture.calls();
    let stage = calls[0].args.last().unwrap();
    assert!(
        !Path::new(stage).exists(),
        "owned stage remains after phase {phase}"
    );
    assert!(!Path::new(&fixture.agent("git/example.test/team/repo")).exists());
    assert!(
        !fixture
            .settings
            .borrow()
            .get_global_settings()
            .0
            .contains_key("packages")
    );
    fixture
        .script
        .hooks
        .borrow_mut()
        .push_back(Box::new(write_stage_manifest));
    block_on(fixture.manager.install_and_persist(source, None)).unwrap();
    assert_eq!(
        fixture.calls()[phase + 1..]
            .iter()
            .map(|call| call.args[0].as_str())
            .collect::<Vec<_>>(),
        ["clone", "checkout", "install"]
    );
    assert_eq!(
        fixture
            .calls()
            .iter()
            .filter(|call| call.args[0] == "clone")
            .count(),
        2
    );
    assert_eq!(
        fixture.settings.borrow().get_global_settings().0["packages"],
        json!([source])
    );
}

/// Gives the scripted clone and preparation commands a real manifest to consume.
fn write_stage_manifest(call: &crate::sandbox::Call) {
    let stage = call
        .cwd
        .as_ref()
        .unwrap_or_else(|| call.args.last().unwrap());
    std::fs::write(Path::new(stage).join("package.json"), "{}").unwrap();
}

#[test]
fn repin_settings_replace_first_match_in_selected_scope() {
    settings_cases(&[
        0, 1, 6, 9, 14, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 45, 46,
    ]);
}

#[test]
fn npm_repin_survives_following_resolution() {
    let fixture = Fixture::new(&json!({"packages":[{"source":"npm:pkg@1", "skills":["kept"]}]}));
    block_on(fixture.manager.install_and_persist("npm:pkg@2", None)).unwrap();
    assert_eq!(
        fixture.settings.borrow().get_global_settings().0["packages"],
        json!([{"source":"npm:pkg@2", "skills":["kept"]}])
    );
    block_on(fixture.manager.resolve(None)).unwrap();
    assert!(
        !fixture
            .calls()
            .iter()
            .any(|call| call.args.iter().any(|arg| arg == "pkg@1"))
    );
    assert!(
        !fixture
            .manager
            .add_source_to_settings("npm:pkg@2", None)
            .unwrap()
    );
}

#[cfg(unix)]
#[test]
fn git_requested_ref_is_applied_before_dependencies() {
    let fixture = crate::native_git::retry::RetryFixture::new("ordinary");
    fixture.advance();
    let expected = crate::native_git::git(&fixture.repo.seed, &["rev-parse", "HEAD"]);
    fixture
        .run(
            fixture
                .manager
                .install("git:github.com/acme/tool@main", None),
        )
        .unwrap();
    assert_eq!(
        *fixture.dependencies.borrow(),
        [expected],
        "existing checkout must reach requested ref before dependencies"
    );
}

#[cfg(unix)]
#[test]
fn git_failed_dependency_restores_revision_at_mutation_owner() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let fixture = crate::native_git::retry::RetryFixture::new("ordinary");
    fixture.advance();
    *fixture.fail.borrow_mut() = "dependencies".into();
    assert!(fixture.run(fixture.manager.clone().update(None)).is_err());
    assert_eq!(
        crate::native_git::git(&fixture.repo.target, &["rev-parse", "HEAD"]),
        fixture.repo.original,
        "failed preparation must restore previous revision"
    );
}

#[cfg(unix)]
#[test]
fn git_update_failure_restores_head_and_retry_installs_dependencies() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    for layout in ["ordinary", "linked", "separate", "include"] {
        for phase in ["reset", "clean", "dependencies"] {
            check_update_retry(layout, phase);
        }
    }
}

/// A strict pure-settings input and its independently recorded expected observation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsCase {
    /// Inputs supplied to the scoped editor.
    input: SettingsInput,
    /// Complete editor and persistence observations.
    expected: SettingsExpected,
}
/// Persisted values, scope and authored new source.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsInput {
    /// Raw settings without eager package decoding.
    settings: serde_json::Value,
    /// Authored source.
    source: String,
    /// Selected settings scope.
    scope: String,
}
/// Expected editor result and raw selected-scope packages.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsExpected {
    /// Whether the editor changed settings, absent on error.
    changed: Option<bool>,
    /// Native error kind, absent on success.
    error: Option<String>,
    /// Entire packages value, including unread entries.
    packages: serde_json::Value,
    /// Setter scopes in order.
    writes: Vec<String>,
}
/// Replays disjoint rows through the public editor and the shared recording storage.
fn settings_cases(indices: &[usize]) {
    let cases: Vec<SettingsCase> =
        serde_json::from_str(include_str!("fixtures/repin-settings.json")).unwrap();
    assert_eq!(cases.len(), 50);
    for &index in indices {
        let case = &cases[index];
        check_settings_case(case, index);
    }
}

#[test]
fn repin_settings_short_circuit_keeps_later_invalid_entries_unread() {
    settings_cases(&[2, 3, 4, 5, 7, 8, 10, 11, 12, 13, 15, 16, 17, 18, 48, 49]);
    #[cfg(unix)]
    normalization_precedes_invalid_root();
}
/// A source needing the removed working directory fails before the invalid packages root.
#[cfg(unix)]
fn normalization_precedes_invalid_root() {
    if crate::resolution_support::case_in_removed_cwd("removed", |_| vec![]).is_none() {
        return;
    }
    let (_, settings, effects) = crate::support::manager(&json!({"packages":7}));
    let manager = maestro_packages::DefaultPackageManager::new(
        maestro_packages::PackageManagerOptions {
            cwd: "relative".into(),
            agent_dir: "relative-agent".into(),
            settings_manager: settings,
        },
        crate::support::Controlled(effects),
    );
    assert_eq!(
        manager
            .add_source_to_settings("./missing", None)
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound,
        "source normalization must precede consuming an invalid packages root"
    );
}

#[test]
fn repin_pin_comparison_uses_shared_whitespace_and_identity_rules() {
    settings_cases(&[43, 44]);
}

#[test]
fn repin_settings_preserve_filter_kinds_and_member_order() {
    settings_cases(&[19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 47]);
    for scope in [
        maestro_packages::InstalledSourceScope::User,
        maestro_packages::InstalledSourceScope::Project,
    ] {
        let fixture = Fixture::new(&json!({}));
        let original: serde_json::Value =
            serde_json::from_str(r#"{"before":-0.0,"source":"npm:pkg@1","after":[true]}"#).unwrap();
        let entry = maestro_settings::PackageSource::Unknown(original);
        match scope {
            maestro_packages::InstalledSourceScope::User => {
                fixture.settings.borrow_mut().set_packages(vec![entry]);
            }
            maestro_packages::InstalledSourceScope::Project => fixture
                .settings
                .borrow_mut()
                .set_project_packages(vec![entry]),
        }
        fixture
            .manager
            .add_source_to_settings("npm:pkg@2", Some(scope))
            .unwrap();
        let current = match scope {
            maestro_packages::InstalledSourceScope::User => {
                fixture.settings.borrow().get_global_settings()
            }
            maestro_packages::InstalledSourceScope::Project => {
                fixture.settings.borrow().get_project_settings()
            }
        };
        let object = current.0["packages"][0].as_object().unwrap();
        assert_eq!(
            object.keys().map(String::as_str).collect::<Vec<_>>(),
            ["before", "source", "after"]
        );
        assert!(object["before"].as_f64().unwrap().is_sign_negative());
    }
}

#[test]
fn git_missing_install_publishes_only_prepared_contents() {
    use maestro_packages::InstalledSourceScope::{Project, User};
    for scope in [User, Project] {
        for manifest in [false, true] {
            for reference in ["", "@v1"] {
                let fixture = Fixture::new(&json!({}));
                prepare_clone_contents(&fixture, manifest);
                let source = format!("https://example.test/team/repo{reference}");
                block_on(fixture.manager.install(&source, Some(scope))).unwrap();
                let target = fixture
                    .manager
                    .get_installed_path(&source, scope)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    std::fs::read_to_string(Path::new(&target).join("content")).unwrap(),
                    "prepared"
                );
                assert_eq!(
                    fixture
                        .calls()
                        .iter()
                        .filter(|call| call.command == "npm")
                        .count(),
                    usize::from(manifest)
                );
                let root = Path::new(&target).ancestors().nth(3).unwrap();
                assert_eq!(
                    std::fs::read_to_string(root.join(".gitignore")).unwrap(),
                    "*\n!.gitignore\n"
                );
            }
        }
    }
}

#[test]
fn git_staging_collision_and_publication_failures_keep_owned_paths() {
    #[cfg(unix)]
    {
        let fixture = Fixture::new(&json!({}));
        let parent = fixture.agent("git/example.test/team");
        std::fs::create_dir_all(&parent).unwrap();
        if let Some(restore) = maestro_test_directory::deny(&parent, 0o555).unwrap() {
            let result = block_on(
                fixture
                    .manager
                    .install("https://example.test/team/repo", None),
            );
            drop(restore);
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
            assert_eq!(std::fs::read_dir(parent).unwrap().count(), 0);
        }
    }
    let collision_fixture = Fixture::new(&json!({}));
    // Another writer takes the target while the clone runs.
    let foreign = collision_fixture.agent("git/example.test/team/repo");
    collision_fixture
        .script
        .hooks
        .borrow_mut()
        .push_back(Box::new(move |_| {
            std::fs::create_dir(&foreign).unwrap();
            std::fs::write(Path::new(&foreign).join("foreign"), "retain").unwrap();
        }));
    assert!(
        block_on(
            collision_fixture
                .manager
                .install("https://example.test/team/repo", None)
        )
        .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(collision_fixture.agent("git/example.test/team/repo/foreign"))
            .unwrap(),
        "retain"
    );
    let parent = collision_fixture.agent("git/example.test/team");
    assert_eq!(std::fs::read_dir(parent).unwrap().count(), 1);
    let fixture = Fixture::new(&json!({}));
    let parent = fixture.agent("git/example.test/team");
    fixture.script.held.set(true);
    let mut pending = fixture
        .manager
        .install("https://example.test/team/repo", None);
    assert!(crate::sandbox::poll_once(&mut pending).is_none());
    assert_eq!(std::fs::read_dir(&parent).unwrap().count(), 1);
    drop(pending);
    assert_eq!(std::fs::read_dir(parent).unwrap().count(), 0);
}

#[test]
fn git_missing_acquisition_routes_share_staging() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    for route in ["install", "persist", "configured", "explicit", "update"] {
        let source = "https://example.test/team/repo";
        let fixture = Fixture::new(&if matches!(route, "configured" | "update") {
            json!({"packages":[source]})
        } else {
            json!({})
        });
        fixture
            .script
            .hooks
            .borrow_mut()
            .push_back(Box::new(|call| {
                std::fs::write(
                    Path::new(call.args.last().unwrap()).join("partial"),
                    "partial",
                )
                .unwrap();
            }));
        fixture
            .script
            .results
            .borrow_mut()
            .push_back(Err(io::Error::other("preparation rejected")));
        assert!(
            acquisition_route(&fixture, route, source).is_err(),
            "{route}"
        );
        assert!(!Path::new(&fixture.agent("git/example.test/team/repo")).exists());
        acquisition_route(&fixture, route, source).unwrap();
        assert_eq!(
            fixture
                .calls()
                .iter()
                .filter(|call| call.args[0] == "clone")
                .count(),
            2,
            "{route}"
        );
        assert_eq!(
            fixture
                .settings
                .borrow()
                .get_global_settings()
                .0
                .get("packages")
                .cloned(),
            if matches!(route, "persist" | "configured" | "update") {
                Some(json!([source]))
            } else {
                None
            }
        );
    }
}

/// Drives one public acquisition route without conflating its registration policy.
fn acquisition_route(fixture: &Fixture, route: &str, source: &str) -> io::Result<()> {
    match route {
        "install" => block_on(fixture.manager.install(source, None)),
        "persist" => block_on(fixture.manager.install_and_persist(source, None)),
        "configured" => block_on(fixture.manager.resolve(None)).map(drop),
        "explicit" => block_on(fixture.manager.resolve_extension_sources(
            &[source.into()],
            Some(maestro_resources::SourceScope::User),
        ))
        .map(drop),
        "update" => {
            let future = fixture.manager.clone().update(Some(source));
            crate::runtime::run(fixture.local.run_until(future))
        }
        _ => unreachable!(),
    }
}

#[test]
fn repin_failure_never_publishes_settings() {
    for failure in ["checkout", "dependencies", "npm", "completion"] {
        let fixture = Fixture::new(&json!({"packages":["npm:pkg@1"]}));
        let source = if failure == "checkout" || failure == "dependencies" {
            "https://example.test/team/repo@v1"
        } else {
            "npm:pkg@2"
        };
        if failure == "completion" {
            fixture
                .manager
                .set_progress_callback(Some(std::rc::Rc::new(reject_completion)));
        } else {
            let successful = match failure {
                "checkout" => 1,
                "dependencies" => 2,
                _ => 0,
            };
            if source.starts_with("https") {
                crate::acquisition_checks::clone_creates_target(&fixture, true);
            }
            fixture
                .script
                .results
                .borrow_mut()
                .extend((0..successful).map(|_| Ok(Some(0))));
            fixture
                .script
                .results
                .borrow_mut()
                .push_back(Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "acquisition rejected",
                )));
        }
        assert!(block_on(fixture.manager.install_and_persist(source, None)).is_err());
        assert_eq!(
            fixture.settings.borrow().get_global_settings().0["packages"],
            json!(["npm:pkg@1"])
        );
    }
}

#[cfg(unix)]
#[test]
fn repin_persistence_replaces_current_git_spelling_and_appends_absent_sources() {
    use maestro_packages::InstalledSourceScope::{Project, User};
    for scope in [User, Project] {
        for present in [false, true] {
            let (manager, settings, effects, tree) =
                crate::support::uninstalled(&json!({}), &json!([]));
            let entry =
                maestro_settings::PackageSource::Source("git:example.test/team/repo@old".into());
            let entries = present.then_some(entry).into_iter().collect();
            let target = match scope {
                User => {
                    settings.borrow_mut().set_packages(entries);
                    "/home/reader/agent/git/example.test/team/repo"
                }
                Project => {
                    settings.borrow_mut().set_project_packages(entries);
                    "/work/project/.maestro/git/example.test/team/repo"
                }
            };
            std::fs::create_dir_all(crate::support::below(tree.path(), target)).unwrap();
            // The refreshed head equals the installed one, so the checkout is current.
            *effects.capture_outputs.borrow_mut() = ["origin/main", "tip", "same", "same"]
                .map(|text| Ok(text.to_owned()))
                .into();
            let source = "https://example.test/team/repo";
            crate::runtime::runtime()
                .block_on(manager.install_and_persist(source, Some(scope)))
                .unwrap();
            let settings = match scope {
                User => settings.borrow().get_global_settings(),
                Project => settings.borrow().get_project_settings(),
            };
            assert_eq!(settings.0["packages"], json!([source]));
            let verbs: Vec<_> = effects
                .calls
                .borrow()
                .iter()
                .map(|(_, args)| args[0].clone())
                .collect();
            assert_eq!(
                verbs,
                ["rev-parse", "rev-parse", "fetch", "rev-parse", "rev-parse"],
                "an unpinned install refreshes the checkout and installs nothing"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn git_repin_ref_forms_retry_without_rewinding_requested_branches() {
    for layout in ["ordinary", "linked", "separate", "include"] {
        for kind in [
            "branch", "tag", "hash", "missing", "current", "no-ref", "detached",
        ] {
            check_repin_case(layout, kind);
        }
    }
}

#[cfg(unix)]
/// Exercises one complete native re-pin failure/retry transaction.
fn check_repin_case(layout: &str, kind: &str) {
    use crate::native_git::{git, retry::RetryFixture};
    let fixture = RetryFixture::new(layout);
    fixture.advance();
    let desired = git(&fixture.repo.seed, &["rev-parse", "HEAD"]);
    let reference = prepare_ref(&fixture, kind, &desired);
    let old = git(&fixture.repo.target, &["rev-parse", "HEAD"]);
    let selection = git(&fixture.repo.target, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let source = reference.as_ref().map_or_else(
        || "git:github.com/acme/tool".into(),
        |reference| format!("git:github.com/acme/tool@{reference}"),
    );
    *fixture.fail.borrow_mut() = "dependencies".into();
    let first = fixture.run(fixture.manager.install_and_persist(&source, None));
    if kind == "current" {
        first.unwrap();
        assert!(fixture.dependencies.borrow().is_empty());
        assert_eq!(
            fixture.settings.borrow().get_global_settings().0["packages"],
            json!([source])
        );
        return;
    }
    assert!(first.is_err(), "{layout}/{kind}");
    assert_repin_failure(&fixture, &old, &selection);
    if kind == "missing" {
        assert!(fixture.dependencies.borrow().is_empty());
        return;
    }
    let attempts = fixture.dependencies.borrow().len();
    fixture.fail.borrow_mut().clear();
    fixture
        .run(fixture.manager.install_and_persist(&source, None))
        .unwrap();
    assert_eq!(fixture.dependencies.borrow().len(), attempts + 1);
    assert_eq!(git(&fixture.repo.target, &["rev-parse", "HEAD"]), desired);
    assert_eq!(
        fixture.settings.borrow().get_global_settings().0["packages"],
        json!([source])
    );
}

#[cfg(unix)]
/// Publishes distinguishable ref forms on the remote with native Git.
fn prepare_ref(
    fixture: &crate::native_git::retry::RetryFixture,
    kind: &str,
    desired: &str,
) -> Option<String> {
    use crate::native_git::git;
    let remote = &fixture.repo.remote;
    match kind {
        "branch" | "detached" => {
            git(remote, &["branch", "new-pin", desired]);
            if kind == "detached" {
                git(&fixture.repo.target, &["checkout", "--detach"]);
            }
            Some("new-pin".into())
        }
        "tag" => {
            git(remote, &["tag", "release-tag", desired]);
            Some("release-tag".into())
        }
        "hash" => Some(desired.into()),
        "missing" => Some("not-a-ref".into()),
        "current" => {
            git(remote, &["tag", "current-tag", &fixture.repo.original]);
            Some("current-tag".into())
        }
        _ => None,
    }
}

#[cfg(unix)]
#[test]
fn git_repin_reset_failure_restores_original_selection() {
    for layout in ["ordinary", "linked", "separate", "include"] {
        for phase in ["reset", "after-reset"] {
            for detached in [false, true] {
                check_reset_failure(layout, phase, detached);
            }
        }
    }
    check_repin_probe_failure();
}

#[cfg(unix)]
#[test]
fn git_fetch_or_probe_failure_does_not_start_rollback() {
    use crate::native_git::{git, retry::RetryFixture};
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    for phase in ["fetch", "HEAD", "@{upstream}", "current"] {
        let fixture = RetryFixture::new("ordinary");
        if phase != "current" {
            fixture.advance();
        }
        std::fs::write(fixture.repo.target.join("content"), "dirty").unwrap();
        std::fs::write(fixture.repo.target.join("untracked"), "keep").unwrap();
        if phase == "fetch" {
            *fixture.fail.borrow_mut() = "fetch".into();
        }
        if matches!(phase, "HEAD" | "@{upstream}") {
            reject_probe(&fixture, phase);
        }
        let result = fixture.run(fixture.manager.clone().update(None));
        assert_eq!(result.is_ok(), phase == "current");
        assert_eq!(
            git(&fixture.repo.target, &["rev-parse", "HEAD"]),
            fixture.repo.original
        );
        assert_eq!(
            std::fs::read_to_string(fixture.repo.target.join("content")).unwrap(),
            "dirty"
        );
        assert_eq!(
            std::fs::read_to_string(fixture.repo.target.join("untracked")).unwrap(),
            "keep"
        );
        assert!(fixture.dependencies.borrow().is_empty());
        assert!(
            !fixture
                .effects
                .calls
                .borrow()
                .iter()
                .any(|(_, args)| matches!(args[0].as_str(), "reset" | "clean" | "symbolic-ref"))
        );
    }
    check_repin_fetch_failure();
}

#[cfg(unix)]
#[test]
fn git_recovery_io_failure_reports_original_kind_without_persisting() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    check_native_recovery_failure("locked");
    check_native_recovery_failure("selection-recovery");
    let staged = Fixture::new(&json!({}));
    // The clone fails after its staging parent stopped accepting removals.
    let locked = staged.agent("git/example.test/team");
    let denial = std::rc::Rc::new(std::cell::RefCell::new(None));
    let held = std::rc::Rc::clone(&denial);
    staged
        .script
        .hooks
        .borrow_mut()
        .push_back(Box::new(move |_| {
            let restore = maestro_test_directory::deny(&locked, 0o555).unwrap();
            *held.borrow_mut() = Some(restore);
        }));
    staged
        .script
        .results
        .borrow_mut()
        .push_back(Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "clone rejected",
        )));
    let error = block_on(
        staged
            .manager
            .install_and_persist("https://example.test/team/repo", None),
    )
    .unwrap_err();
    // A privileged run still removes the staged clone, so there is no recovery failure.
    let Some(restore) = denial.take().expect("the clone hook ran") else {
        return;
    };
    drop(restore);
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(error.to_string().contains("recovery failed"));
    assert!(
        staged
            .settings
            .borrow()
            .get_global_settings()
            .0
            .get("packages")
            .is_none()
    );
}

#[cfg(unix)]
#[test]
fn git_repin_live_configuration_reaches_selected_transport() {
    use crate::native_git::{git, retry::RetryFixture};
    for layout in ["ordinary", "linked", "separate", "include"] {
        let fixture = RetryFixture::new(layout);
        fixture.advance();
        let common_text = git(&fixture.repo.target, &["rev-parse", "--git-common-dir"]);
        let common = fixture.repo.target.join(&common_text);
        let marker = install_transport(&fixture, &common);
        git(
            &fixture.repo.remote,
            &[
                "branch",
                "fresh-branch",
                &git(&fixture.repo.seed, &["rev-parse", "HEAD"]),
            ],
        );
        *fixture.fail.borrow_mut() = "dependencies".into();
        assert!(
            fixture
                .run(
                    fixture
                        .manager
                        .install("git:github.com/acme/tool@fresh-branch", None)
                )
                .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(&marker).unwrap(),
            "called",
            "effective included upload-pack must actually run: {layout}"
        );
        fixture.fail.borrow_mut().clear();
        fixture
            .run(
                fixture
                    .manager
                    .install("git:github.com/acme/tool@fresh-branch", None),
            )
            .unwrap();
        assert_eq!(
            git(&fixture.repo.target, &["rev-parse", "--git-common-dir"]),
            common_text
        );
    }
}

#[cfg(unix)]
#[test]
fn git_temporary_refresh_recovers_then_suppresses_failure() {
    use crate::native_git::{git, retry::RetryFixture};
    if crate::resolution_support::case_in_child("temporary", |_| vec![]).is_none() {
        return;
    }
    let fixture = RetryFixture::new("ordinary");
    fixture.use_local_clone();
    let sources = ["git:github.com/acme/tool".into()];
    let result =
        fixture
            .run(fixture.manager.resolve_extension_sources(
                &sources,
                Some(maestro_resources::SourceScope::Temporary),
            ))
            .unwrap();
    assert_eq!(result.extensions.len(), 1);
    let resource = Path::new(&result.extensions[0].path);
    let target = resource.parent().unwrap().parent().unwrap();
    let previous = git(target, &["rev-parse", "HEAD"]);
    fixture.advance();
    *fixture.fail.borrow_mut() = "dependencies".into();
    let recovered =
        fixture
            .run(fixture.manager.resolve_extension_sources(
                &sources,
                Some(maestro_resources::SourceScope::Temporary),
            ))
            .unwrap();
    assert_eq!(recovered.extensions[0].path, result.extensions[0].path);
    assert_eq!(git(target, &["rev-parse", "HEAD"]), previous);
    assert_eq!(
        std::fs::read_to_string(resource).unwrap(),
        "export const retained = true;"
    );
    assert_eq!(fixture.dependencies.borrow().len(), 2);
}

#[cfg(unix)]
/// Checks failure on either side of the reset effect in one native layout.
fn check_reset_failure(layout: &str, phase: &str, detached: bool) {
    use crate::native_git::{git, retry::RetryFixture};
    let fixture = RetryFixture::new(layout);
    fixture.advance();
    git(
        &fixture.repo.remote,
        &[
            "branch",
            "new-pin",
            &git(&fixture.repo.seed, &["rev-parse", "HEAD"]),
        ],
    );
    if detached {
        git(&fixture.repo.target, &["checkout", "--detach"]);
    }
    let selection = git(&fixture.repo.target, &["rev-parse", "--abbrev-ref", "HEAD"]);
    *fixture.fail.borrow_mut() = phase.into();
    assert!(
        fixture
            .run(
                fixture
                    .manager
                    .install_and_persist("git:github.com/acme/tool@new-pin", None)
            )
            .is_err()
    );
    assert_eq!(
        git(&fixture.repo.target, &["rev-parse", "HEAD"]),
        fixture.repo.original
    );
    assert_eq!(
        git(&fixture.repo.target, &["rev-parse", "--abbrev-ref", "HEAD"]),
        selection
    );
    assert_eq!(
        fixture.settings.borrow().get_global_settings().0["packages"],
        json!(["git:github.com/acme/tool"])
    );
}

/// Seeds distinguishable files inside the actual clone destination.
fn prepare_clone_contents(fixture: &Fixture, manifest: bool) {
    fixture
        .script
        .hooks
        .borrow_mut()
        .push_back(Box::new(move |call| {
            let path = Path::new(call.args.last().unwrap());
            std::fs::write(path.join("content"), "prepared").unwrap();
            if manifest {
                std::fs::write(path.join("package.json"), "{}").unwrap();
            }
        }));
}

/// Fails only after acquisition has completed.
fn reject_completion(event: &maestro_packages::ProgressEvent) -> io::Result<()> {
    if event.r#type == maestro_packages::ProgressEventType::Complete {
        Err(io::Error::other("completion rejected"))
    } else {
        Ok(())
    }
}

#[cfg(unix)]
/// Rejects the comparison probe, not the earlier upstream-selector probe.
fn reject_probe(fixture: &crate::native_git::retry::RetryFixture, phase: &'static str) {
    let hook = fixture.effects.capture_hook.borrow().clone().unwrap();
    let count = std::rc::Rc::new(std::cell::Cell::new(0));
    *fixture.effects.capture_hook.borrow_mut() =
        Some(std::rc::Rc::new(move |command, args, cwd| {
            if args == ["rev-parse", phase] {
                count.set(count.get() + 1);
                if phase == "HEAD" || count.get() == 2 {
                    return Box::pin(std::future::ready(Err(io::Error::other("probe rejected"))));
                }
            }
            hook(command, args, cwd)
        }));
}

/// Asserts one complete raw-settings edit and its scoped persistence observation.
fn check_settings_case(case: &SettingsCase, index: usize) {
    let (manager, settings, storage) = settings_replay(case);
    let other = if case.input.scope == "user" {
        settings.borrow().get_project_settings()
    } else {
        settings.borrow().get_global_settings()
    };
    let result = manager.add_source_to_settings(
        &case.input.source,
        Some(if case.input.scope == "user" {
            maestro_packages::InstalledSourceScope::User
        } else {
            maestro_packages::InstalledSourceScope::Project
        }),
    );
    assert_eq!(
        result.as_ref().ok().copied(),
        case.expected.changed,
        "row {index}"
    );
    assert_eq!(
        result.err().map(|error| format!("{:?}", error.kind())),
        case.expected.error,
        "row {index}"
    );
    let (current, untouched) = if case.input.scope == "user" {
        (
            settings.borrow().get_global_settings(),
            settings.borrow().get_project_settings(),
        )
    } else {
        (
            settings.borrow().get_project_settings(),
            settings.borrow().get_global_settings(),
        )
    };
    assert_eq!(
        current
            .0
            .get("packages")
            .unwrap_or(&serde_json::Value::Null),
        &case.expected.packages,
        "row {index}"
    );
    assert_eq!(untouched, other);
    crate::runtime::run(settings.borrow().flush());
    assert_eq!(
        *storage.scopes.lock().unwrap(),
        case.expected.writes,
        "row {index}"
    );
}

#[cfg(unix)]
/// Installs a finite upload-pack shim through a child, not a writable executable descriptor.
fn install_transport(
    fixture: &crate::native_git::retry::RetryFixture,
    common: &Path,
) -> std::path::PathBuf {
    use crate::native_git::git;
    let marker = fixture.repo.scratch.path().join("transport-called");
    let source = fixture.repo.scratch.path().join("transport-source");
    let executable = fixture.repo.scratch.path().join("transport");
    assert!(Path::new("/usr/bin/git-upload-pack").is_file());
    std::fs::write(
        &source,
        format!(
            "#!/bin/sh\nprintf called >> '{}'\nexec /usr/bin/git-upload-pack \"$@\"\n",
            marker.display()
        ),
    )
    .unwrap();
    assert!(
        std::process::Command::new("/usr/bin/install")
            .args(["-m", "0700"])
            .arg(&source)
            .arg(&executable)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(
        common.join("transport.conf"),
        format!(
            "[remote \"origin\"]\n uploadpack = {}\n",
            executable.display()
        ),
    )
    .unwrap();
    git(
        &fixture.repo.target,
        &[
            "config",
            "--local",
            "--add",
            "include.path",
            "transport.conf",
        ],
    );
    marker
}

/// A replay manager with aliases to its real persistence observations.
type SettingsReplay = (
    maestro_packages::DefaultPackageManager<crate::support::Controlled>,
    std::rc::Rc<std::cell::RefCell<maestro_settings::SettingsManager>>,
    std::sync::Arc<crate::configured_source_checks::Storage>,
);
/// Seeds raw storage before creating the scoped settings owner.
fn settings_replay(case: &SettingsCase) -> SettingsReplay {
    use maestro_settings::{SettingsScope, SettingsStorage};
    let selected = if case.input.scope == "user" {
        SettingsScope::Global
    } else {
        SettingsScope::Project
    };
    let storage = std::sync::Arc::new(crate::configured_source_checks::Storage::default());
    let text = case.input.settings.to_string();
    storage
        .inner
        .with_lock(selected, &mut |_| Ok(Some(text.clone())))
        .unwrap();
    let settings = std::rc::Rc::new(std::cell::RefCell::new(
        maestro_settings::SettingsManager::from_storage(storage.clone()),
    ));
    let manager = maestro_packages::DefaultPackageManager::new(
        maestro_packages::PackageManagerOptions {
            cwd: "/fixture/project".into(),
            agent_dir: "/fixture/agent".into(),
            settings_manager: settings.clone(),
        },
        crate::support::Controlled(std::rc::Rc::new(crate::support::Effects::default())),
    );
    (manager, settings, storage)
}

#[cfg(unix)]
/// Checks the retained revision, selection and settings after a failed re-pin.
fn assert_repin_failure(
    fixture: &crate::native_git::retry::RetryFixture,
    old: &str,
    selection: &str,
) {
    use crate::native_git::git;
    assert_eq!(git(&fixture.repo.target, &["rev-parse", "HEAD"]), old);
    assert_eq!(
        git(&fixture.repo.target, &["rev-parse", "--abbrev-ref", "HEAD"]),
        selection
    );
    assert_eq!(
        fixture.settings.borrow().get_global_settings().0["packages"],
        json!(["git:github.com/acme/tool"])
    );
}

#[cfg(unix)]
/// A failed checkout must be reacquired until the requested ref actually exists upstream.
fn missing_invalid_ref_retries_until_available() {
    use crate::native_git::{git, retry::RetryFixture};
    let fixture = RetryFixture::new("ordinary");
    fixture.use_local_clone();
    std::fs::remove_dir_all(&fixture.repo.target).unwrap();
    let source = "git:github.com/acme/tool@eventual";
    for _ in 0..2 {
        assert!(
            fixture
                .run(fixture.manager.install_and_persist(source, None))
                .is_err()
        );
        assert!(!fixture.repo.target.exists());
    }
    git(
        &fixture.repo.remote,
        &["tag", "eventual", &fixture.repo.original],
    );
    fixture
        .run(fixture.manager.install_and_persist(source, None))
        .unwrap();
    assert_eq!(
        git(&fixture.repo.target, &["rev-parse", "HEAD"]),
        fixture.repo.original
    );
    assert_eq!(fixture.dependencies.borrow().len(), 1);
    assert_eq!(
        fixture.settings.borrow().get_global_settings().0["packages"],
        json!([source])
    );
}

#[cfg(unix)]
/// Drives each mutation failure through the public update and a completed retry.
fn check_update_retry(layout: &str, phase: &str) {
    let fixture = crate::native_git::retry::RetryFixture::new(layout);
    fixture.advance();
    *fixture.fail.borrow_mut() = phase.into();
    assert!(fixture.run(fixture.manager.clone().update(None)).is_err());
    assert_eq!(
        crate::native_git::git(&fixture.repo.target, &["rev-parse", "HEAD"]),
        fixture.repo.original
    );
    let attempts = fixture.dependencies.borrow().len();
    fixture.fail.borrow_mut().clear();
    fixture.run(fixture.manager.clone().update(None)).unwrap();
    assert_eq!(
        fixture.dependencies.borrow().len(),
        attempts + 1,
        "public retry must prepare dependencies again: {layout}"
    );
    fixture.repo.assert_remote("changed");
}

#[cfg(unix)]
/// Confirms real reset locking and controlled selection-recovery failure remain observable.
fn check_native_recovery_failure(mode: &str) {
    use crate::native_git::{git, retry::RetryFixture};
    let fixture = RetryFixture::new("ordinary");
    fixture.advance();
    *fixture.fail.borrow_mut() = mode.into();
    let error = if mode == "locked" {
        fixture
            .run(fixture.manager.clone().update(None))
            .unwrap_err()
    } else {
        fixture
            .run(
                fixture
                    .manager
                    .install_and_persist("git:github.com/acme/tool@main", None),
            )
            .unwrap_err()
    };
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(error.to_string().contains("recovery failed"));
    assert_ne!(
        git(&fixture.repo.target, &["rev-parse", "HEAD"]),
        fixture.repo.original
    );
    assert!(error.get_ref().unwrap().source().is_some());
    assert_eq!(
        fixture.settings.borrow().get_global_settings().0["packages"],
        json!(["git:github.com/acme/tool"])
    );
    if mode == "locked" {
        std::fs::remove_file(fixture.repo.target.join(git(
            &fixture.repo.target,
            &["rev-parse", "--git-path", "index.lock"],
        )))
        .unwrap();
    }
}

#[cfg(unix)]
/// A fallible adapter probe must not be mistaken for a detached HEAD.
fn check_repin_probe_failure() {
    use crate::native_git::{git, retry::RetryFixture};
    let fixture = RetryFixture::new("ordinary");
    fixture.advance();
    let selection = git(&fixture.repo.target, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let hook = fixture.effects.capture_hook.borrow().clone().unwrap();
    *fixture.effects.capture_hook.borrow_mut() =
        Some(std::rc::Rc::new(move |command, args, cwd| {
            if args == ["rev-parse", "--symbolic-full-name", "HEAD"] {
                return Box::pin(std::future::ready(Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "probe access denied",
                ))));
            }
            hook(command, args, cwd)
        }));
    *fixture.fail.borrow_mut() = "dependencies".into();
    let error = fixture
        .run(
            fixture
                .manager
                .install_and_persist("git:github.com/acme/tool@main", None),
        )
        .unwrap_err();
    assert_eq!(
        error.kind(),
        io::ErrorKind::PermissionDenied,
        "probe access failure must retain its kind"
    );
    assert_eq!(
        git(&fixture.repo.target, &["rev-parse", "--abbrev-ref", "HEAD"]),
        selection,
        "failed selection probe must not turn an attached checkout into a detached one"
    );
    assert!(
        fixture.dependencies.borrow().is_empty(),
        "a failed state probe must stop before mutation"
    );
    assert!(
        !fixture
            .effects
            .calls
            .borrow()
            .iter()
            .any(|(_, args)| matches!(args[0].as_str(), "clean" | "reset" | "symbolic-ref")),
        "a failed probe must not start mutation or recovery effects"
    );
}

#[cfg(unix)]
/// A failed fetch of the requested ref leaves the dirty checkout untouched.
fn check_repin_fetch_failure() {
    use crate::native_git::{git, retry::RetryFixture};
    let fixture = RetryFixture::new("ordinary");
    fixture.advance();
    std::fs::write(fixture.repo.target.join("content"), "dirty").unwrap();
    std::fs::write(fixture.repo.target.join("untracked"), "retain").unwrap();
    *fixture.fail.borrow_mut() = "fetch".into();

    let error = fixture
        .run(
            fixture
                .manager
                .install_and_persist("git:github.com/acme/tool@main", None),
        )
        .unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(
        git(&fixture.repo.target, &["rev-parse", "HEAD"]),
        fixture.repo.original
    );
    assert_dirty_contents(&fixture);
    assert!(fixture.dependencies.borrow().is_empty());
    assert!(
        !fixture
            .effects
            .calls
            .borrow()
            .iter()
            .any(|(_, args)| matches!(args[0].as_str(), "reset" | "clean" | "symbolic-ref"))
    );
}

#[cfg(unix)]
#[test]
fn git_current_commit_preserves_selection_and_observes_completion_settings() {
    use maestro_packages::InstalledSourceScope::{Project, User};
    for scope in [User, Project] {
        for form in ["branch", "tag", "detached"] {
            for callback_edit in ["absent", "replace", "remove"] {
                check_current_selection(scope, form, callback_edit);
            }
        }
    }
}

#[cfg(unix)]
/// Moves the installed checkout below the project's managed Git root.
fn move_to_project_scope(fixture: &mut crate::native_git::retry::RetryFixture) {
    let path = fixture
        .repo
        .scratch
        .path()
        .join("cwd/.maestro/git/github.com/acme/tool");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::rename(&fixture.repo.target, &path).unwrap();
    fixture.repo.target = path;
}

#[cfg(unix)]
/// Preserves same-commit files while appending or observing a late registration.
fn check_current_selection(
    scope: maestro_packages::InstalledSourceScope,
    form: &str,
    callback_edit: &str,
) {
    use crate::native_git::{git, retry::RetryFixture};
    use maestro_packages::InstalledSourceScope::{Project, User};
    let mut fixture = RetryFixture::new("ordinary");
    if scope == Project {
        move_to_project_scope(&mut fixture);
    }
    let reference = match form {
        "branch" => "main".to_owned(),
        "tag" => {
            git(
                &fixture.repo.remote,
                &["tag", "current", &fixture.repo.original],
            );
            "current".to_owned()
        }
        _ => {
            git(&fixture.repo.target, &["checkout", "--detach"]);
            fixture.repo.original.clone()
        }
    };
    let selection = git(
        &fixture.repo.target,
        &["rev-parse", "--symbolic-full-name", "HEAD"],
    );
    std::fs::write(fixture.repo.target.join("content"), "dirty").unwrap();
    std::fs::write(fixture.repo.target.join("untracked"), "retain").unwrap();
    fixture.settings.borrow_mut().set_packages(vec![]);
    if callback_edit != "absent" {
        edit_settings_on_completion(&fixture, scope, callback_edit == "remove");
    }

    let source = format!("git:github.com/acme/tool@{reference}");
    fixture
        .run(fixture.manager.install_and_persist(&source, Some(scope)))
        .unwrap();
    let settings = match scope {
        User => fixture.settings.borrow().get_global_settings(),
        Project => fixture.settings.borrow().get_project_settings(),
    };
    assert_eq!(settings.0["packages"], json!([&source]));
    assert!(fixture.dependencies.borrow().is_empty());
    assert_eq!(
        git(
            &fixture.repo.target,
            &["rev-parse", "--symbolic-full-name", "HEAD"]
        ),
        selection
    );
    assert_dirty_contents(&fixture);
}

#[cfg(unix)]
/// Edits the selected settings scope only after acquisition completes.
fn edit_settings_on_completion(
    fixture: &crate::native_git::retry::RetryFixture,
    scope: maestro_packages::InstalledSourceScope,
    remove: bool,
) {
    use maestro_packages::InstalledSourceScope::{Project, User};

    if remove {
        let entries = vec![maestro_settings::PackageSource::Source(
            "git:github.com/acme/tool@before".into(),
        )];
        match scope {
            User => fixture.settings.borrow_mut().set_packages(entries),
            Project => fixture.settings.borrow_mut().set_project_packages(entries),
        }
    }
    let settings = fixture.settings.clone();
    fixture
        .manager
        .set_progress_callback(Some(std::rc::Rc::new(move |event| {
            if event.r#type == maestro_packages::ProgressEventType::Complete {
                let entries = if remove {
                    vec![]
                } else {
                    vec![maestro_settings::PackageSource::Source(
                        "git:github.com/acme/tool@callback".into(),
                    )]
                };
                match scope {
                    User => settings.borrow_mut().set_packages(entries),
                    Project => settings.borrow_mut().set_project_packages(entries),
                }
            }
            Ok(())
        })));
}

#[cfg(unix)]
/// Observes dirty and untracked files after a same-commit refresh.
fn assert_dirty_contents(fixture: &crate::native_git::retry::RetryFixture) {
    assert_eq!(
        std::fs::read_to_string(fixture.repo.target.join("content")).unwrap(),
        "dirty"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.repo.target.join("untracked")).unwrap(),
        "retain"
    );
}
