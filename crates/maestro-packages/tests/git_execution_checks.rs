//! Configured Git refresh through the public package manager.
#![cfg(test)]
#![cfg(unix)]
use crate::native_git;
use crate::support;
use crate::update_corpus;
use maestro_packages::PackageManager;
use serde_json::json;
use std::rc::Rc;

/// Completes the update of a checkout without a manifest and joins its admitted workers.
fn run(responses: &[&str]) -> Rc<support::Effects> {
    let responses = responses.iter().map(|text| Ok((*text).into())).collect();
    let (effects, result, _tree) = controlled_refresh(responses, without_manifest);
    result.unwrap();
    effects
}

#[test]
fn git_fetch_compares_refreshed_heads_before_replacement() {
    let effects = run(&["origin/feature/slash", "before", "local", "changed"]);
    assert_eq!(
        effects
            .calls
            .borrow()
            .iter()
            .map(|(_, args)| args.clone())
            .collect::<Vec<_>>(),
        [
            vec!["rev-parse", "--abbrev-ref", "@{upstream}"],
            vec!["rev-parse", "@{upstream}"],
            vec![
                "fetch",
                "--prune",
                "--no-tags",
                "origin",
                "+refs/heads/feature/slash:refs/remotes/origin/feature/slash"
            ],
            vec!["rev-parse", "HEAD"],
            vec!["rev-parse", "@{upstream}"],
            vec!["rev-parse", "HEAD"],
            vec!["rev-parse", "--symbolic-full-name", "HEAD"],
            vec!["reset", "--hard", "@{upstream}"],
            vec!["clean", "-fdx"]
        ]
    );
    let current = run(&["origin/main", "stale", "same", "same"]);
    assert_eq!(current.calls.borrow().len(), 5);
    assert_eq!(
        current.calls.borrow()[2].1,
        [
            "fetch",
            "--prune",
            "--no-tags",
            "origin",
            "+refs/heads/main:refs/remotes/origin/main"
        ]
    );
}

#[test]
fn git_native_current_preserves_dirty_and_untracked_contents() {
    let repo = native_git::Repository::new();
    std::fs::write(repo.target.join("content"), "dirty").unwrap();
    std::fs::write(repo.target.join("untracked"), "retained").unwrap();
    std::fs::write(repo.target.join("package.json"), "{\"dependencies\":{}}").unwrap();
    repo.update(false);
    assert!(
        std::fs::read_to_string(repo.target.join(".git/FETCH_HEAD"))
            .unwrap()
            .contains("branch 'main'")
    );
    assert_eq!(
        native_git::git(&repo.target, &["rev-parse", "HEAD"]),
        repo.original
    );
    assert_eq!(
        std::fs::read_to_string(repo.target.join("content")).unwrap(),
        "dirty"
    );
    assert_eq!(
        std::fs::read_to_string(repo.target.join("untracked")).unwrap(),
        "retained"
    );
}

#[test]
fn git_native_new_commits_replace_head_and_contents() {
    for count in [1, 3] {
        let repo = native_git::Repository::new();
        std::fs::write(repo.target.join("content"), "dirty").unwrap();
        std::fs::write(repo.target.join("untracked"), "removed").unwrap();
        std::fs::write(repo.target.join(".gitignore"), "ignored\n").unwrap();
        std::fs::write(repo.target.join("ignored"), "removed").unwrap();
        for index in 0..count {
            repo.commit(&format!("commit-{index}"));
        }
        repo.update(false);
        repo.assert_remote(&format!("commit-{}", count - 1));
        assert!(!repo.target.join("untracked").exists());
        assert!(!repo.target.join("ignored").exists());
    }
}

#[test]
fn git_native_detached_and_non_origin_use_origin_head() {
    for detached in [true, false] {
        let repo = native_git::Repository::new();
        if detached {
            native_git::git(&repo.target, &["checkout", "--detach"]);
        } else {
            native_git::git(
                &repo.target,
                &["remote", "add", "other", repo.remote.to_str().unwrap()],
            );
            native_git::git(&repo.target, &["fetch", "other"]);
            native_git::git(&repo.target, &["branch", "--set-upstream-to=other/main"]);
        }
        repo.commit("updated");
        repo.update(false);
        repo.assert_remote("updated");
    }
}

#[test]
fn git_native_rewritten_history_replaces_installed_commits() {
    for scenario in ["rewritten", "removed", "orphan"] {
        let repo = native_git::Repository::new();
        match scenario {
            "rewritten" => {
                native_git::git(
                    &repo.seed,
                    &["commit", "--amend", "--allow-empty", "-m", "rewritten"],
                );
            }
            "removed" => {
                repo.commit("installed-then-removed");
                repo.update(false);
                native_git::git(&repo.seed, &["reset", "--hard", &repo.original]);
            }
            _ => {
                native_git::git(&repo.seed, &["checkout", "--orphan", "replacement"]);
                native_git::git(&repo.seed, &["rm", "-rf", "."]);
                std::fs::write(repo.seed.join("other"), "unrelated").unwrap();
                native_git::git(&repo.seed, &["add", "other"]);
                native_git::git(&repo.seed, &["commit", "-m", "orphan"]);
                native_git::git(&repo.seed, &["branch", "-M", "main"]);
            }
        }
        repo.commit(scenario);
        std::fs::write(repo.target.join("sentinel"), "removed").unwrap();
        repo.update(false);
        repo.assert_remote(scenario);
        assert!(!repo.target.join("sentinel").exists());
    }
}

#[test]
fn git_native_pin_and_user_scope_do_not_acquire_project_contents() {
    let repo = native_git::Repository::new();
    repo.commit("updated");
    repo.update(true);
    assert_eq!(
        native_git::git(&repo.target, &["rev-parse", "HEAD"]),
        repo.original
    );
    assert_eq!(
        std::fs::read_to_string(repo.target.join("content")).unwrap(),
        "initial"
    );
    repo.update(false);
    repo.assert_remote("updated");
}

/// The user checkout of the configured source in a configured tree.
const CHECKOUT: &str = "/home/reader/agent/git/github.com/acme/tool";
/// Runs one controlled Git refresh of an installed checkout with a manifest and joins every
/// accepted worker, including failures.
fn controlled_refresh(
    responses: Vec<std::io::Result<String>>,
    configure: impl FnOnce(
        &Rc<support::Effects>,
        &Rc<std::cell::RefCell<maestro_settings::SettingsManager>>,
        &str,
    ),
) -> (
    Rc<support::Effects>,
    std::io::Result<()>,
    crate::scratch::Scratch,
) {
    let (manager, settings, effects, tree) = support::installed(
        &json!({"packages":["git:github.com/acme/tool"]}),
        &json!([]),
    );
    let checkout = support::below(tree.path(), CHECKOUT);
    std::fs::write(format!("{checkout}/package.json"), "{}").unwrap();
    *effects.capture_outputs.borrow_mut() = responses.into();
    configure(&effects, &settings, &checkout);
    let local = Rc::new(tokio::task::LocalSet::new());
    *effects.local.borrow_mut() = Rc::downgrade(&local);
    let runtime = crate::runtime::runtime();
    let result = runtime.block_on(local.run_until(manager.update(None)));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    (effects, result, tree)
}
/// Removes the checkout's manifest, so the refresh installs no dependencies.
fn without_manifest(
    _: &Rc<support::Effects>,
    _: &Rc<std::cell::RefCell<maestro_settings::SettingsManager>>,
    checkout: &str,
) {
    std::fs::remove_file(format!("{checkout}/package.json")).unwrap();
}

#[test]
fn git_targets_preserve_upstream_fallback_boundaries() {
    for upstream in ["other/main", "origin/", "originish/main", ""] {
        let responses = [
            upstream,
            "origin-head",
            "refs/remotes/origin/feature/slash",
            "same",
            "same",
        ]
        .into_iter()
        .map(|text| Ok(text.to_owned()))
        .collect();
        let (effects, result, _tree) = controlled_refresh(responses, without_manifest);
        result.unwrap();
        assert_eq!(
            effects.calls.borrow()[1].1,
            ["remote", "set-head", "origin", "-a"]
        );
        assert_eq!(
            effects.calls.borrow()[4].1,
            [
                "fetch",
                "--prune",
                "--no-tags",
                "origin",
                "+refs/heads/feature/slash:refs/remotes/origin/feature/slash"
            ]
        );
    }
    failed_upstream_probes();
}

#[test]
fn git_target_text_uses_shared_whitespace_rules() {
    let bom = run(&[
        "\u{feff}origin/main\u{feff}",
        "before",
        "\u{feff}same\u{feff}",
        "same",
    ]);
    assert_eq!(bom.calls.borrow().len(), 5);
    assert_eq!(
        bom.calls.borrow()[2].1[4],
        "+refs/heads/main:refs/remotes/origin/main"
    );
    let control = run(&["origin/main", "before", "\u{85}same\u{85}", "same"]);
    assert_eq!(control.calls.borrow().last().unwrap().1, ["clean", "-fdx"]);
    let fallback = run(&[
        "\u{85}origin/main\u{85}",
        "origin-head",
        "else/refs/remotes/origin/main",
        "same",
        "same",
    ]);
    assert_eq!(
        fallback.calls.borrow()[4].1[4],
        "+refs/heads/else/refs/remotes/origin/main:refs/remotes/origin/else/refs/remotes/origin/main"
    );
}

#[test]
fn git_dependency_install_uses_live_command_configuration() {
    use maestro_settings::SettingsListEntry;
    for configured in [
        None,
        Some(vec![]),
        Some(vec![
            SettingsListEntry::String("runner".into()),
            SettingsListEntry::String("prefix".into()),
        ]),
    ] {
        let expected = if configured.as_ref().is_some_and(|args| !args.is_empty()) {
            ("runner".into(), vec!["prefix".into(), "install".into()])
        } else {
            ("npm".into(), vec!["install".into(), "--omit=dev".into()])
        };
        let responses = ["origin/main", "before", "local", "changed"]
            .into_iter()
            .map(|text| Ok(text.into()))
            .collect();
        let (effects, result, _tree) = controlled_refresh(responses, |effects, settings, _| {
            *effects.run_hook.borrow_mut() = Some(edit_after_clean(settings, configured));
        });
        result.unwrap();
        assert_eq!(effects.calls.borrow().last().unwrap(), &expected);
    }
    let responses = ["origin/main", "before", "local", "changed"]
        .into_iter()
        .map(|text| Ok(text.into()))
        .collect();
    let (effects, result, _tree) = controlled_refresh(responses, |effects, settings, checkout| {
        without_manifest(effects, settings, checkout);
        settings
            .borrow_mut()
            .set_npm_command(Some(vec![SettingsListEntry::String(String::new())]));
    });
    result.unwrap();
    assert_eq!(effects.calls.borrow().last().unwrap().1, ["clean", "-fdx"]);
}

#[test]
fn missing_unpinned_git_content_uses_existing_install() {
    use maestro_packages::{ProgressAction, ProgressEventType};
    clone_acquires_dependencies();
    for failure in [false, true] {
        let events = Rc::new(std::cell::RefCell::new(Vec::new()));
        let (manager, _, effects, tree) = support::uninstalled(
            &json!({"packages":["git:github.com/acme/tool"]}),
            &json!([]),
        );
        let observed = events.clone();
        manager.set_progress_callback(Some(Rc::new(move |event| {
            observed
                .borrow_mut()
                .push((event.action, event.r#type, event.source.clone()));
            Ok(())
        })));
        if failure {
            *effects.run_hook.borrow_mut() = Some(failing_command("clone", "clone failed"));
        }
        let local = Rc::new(tokio::task::LocalSet::new());
        *effects.local.borrow_mut() = Rc::downgrade(&local);
        let runtime = crate::runtime::runtime();
        let result = runtime.block_on(local.run_until(manager.update(None)));
        runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
        assert_eq!(result.is_err(), failure);
        assert_eq!(
            spelled(&effects, &tree),
            [(
                "git".into(),
                vec![
                    "clone".into(),
                    "https://github.com/acme/tool".into(),
                    "/home/reader/agent/git/github.com/acme/.controlled-stage".into()
                ]
            )]
        );
        assert_eq!(
            *events.borrow(),
            [
                (
                    ProgressAction::Update,
                    ProgressEventType::Start,
                    "git:github.com/acme/tool".into()
                ),
                (
                    ProgressAction::Update,
                    if failure {
                        ProgressEventType::Error
                    } else {
                        ProgressEventType::Complete
                    },
                    "git:github.com/acme/tool".into()
                )
            ]
        );
    }
}

#[test]
fn git_failures_recover_only_after_mutation() {
    for failed in [
        "origin",
        "fetch",
        "HEAD",
        "refreshed",
        "reset",
        "clean",
        "install",
    ] {
        let (effects, result, _tree) =
            controlled_refresh(failed_responses(failed), |effects, _, _| {
                *effects.run_hook.borrow_mut() = Some(failing_command(failed, "command failed"));
            });
        assert_eq!(
            result
                .unwrap_err()
                .to_string()
                .split("; recovery failed:")
                .next()
                .unwrap(),
            match failed {
                "origin" => "origin failed",
                "HEAD" | "refreshed" => "head failed",
                _ => "command failed",
            }
        );
        let calls = effects.calls.borrow();
        let last = match failed {
            "origin" | "HEAD" | "refreshed" => "rev-parse",
            "reset" | "clean" | "install" => "reset",
            other => other,
        };
        assert_eq!(calls.last().unwrap().1[0], last);
        assert_eq!(
            calls.len(),
            match failed {
                "origin" | "fetch" => 3,
                "HEAD" => 4,
                "refreshed" => 5,
                "reset" => 10,
                "clean" => 11,
                _ => 12,
            }
        );
        assert!(calls.iter().all(|(_, args)| args[0] != "pull"));
    }
}

/// Exercises failed upstream probes and ignored fallback effects.
fn failed_upstream_probes() {
    for initial_head_failure in [false, true] {
        let mut responses = if initial_head_failure {
            vec![Ok("origin/main".into())]
        } else {
            vec![]
        };
        responses.push(Err(std::io::Error::other("upstream unavailable")));
        responses.extend([
            Ok("origin-head".into()),
            Err(std::io::Error::other("symbolic unavailable")),
            Ok("same".into()),
            Ok("same".into()),
        ]);
        let (effects, result, _tree) = controlled_refresh(responses, |effects, _, _| {
            *effects.run_hook.borrow_mut() =
                Some(failing_command("remote", "set-head unavailable"));
        });
        result.unwrap();
        assert!(effects.calls.borrow().iter().any(|(_, args)| args
            == &[
                "fetch",
                "--prune",
                "--no-tags",
                "origin",
                "+HEAD:refs/remotes/origin/HEAD"
            ]));
    }
}

/// Edits command configuration after cleanup, before dependency command selection.
fn edit_after_clean(
    settings: &Rc<std::cell::RefCell<maestro_settings::SettingsManager>>,
    configured: Option<Vec<maestro_settings::SettingsListEntry>>,
) -> support::RunHook {
    let weak = Rc::downgrade(settings);
    let configured = Rc::new(std::cell::RefCell::new(Some(configured)));
    Rc::new(move |_, args, _| {
        if args[0] == "clean" {
            weak.upgrade()
                .unwrap()
                .borrow_mut()
                .set_npm_command(configured.borrow_mut().take().unwrap());
        }
        Box::pin(async { Ok(Some(0)) })
    })
}

/// Fails one inherited operation, leaving captured probes unaffected.
fn failing_command(failed: &'static str, message: &'static str) -> support::RunHook {
    Rc::new(move |_, args, _| {
        let fails = args[0] == failed;
        Box::pin(async move {
            if fails {
                Err(std::io::Error::other(message))
            } else {
                Ok(Some(0))
            }
        })
    })
}
/// Captured results with one failure at the selected phase.
fn failed_responses(failed: &str) -> Vec<std::io::Result<String>> {
    if failed == "origin" {
        return vec![
            Err(std::io::Error::other("upstream failed")),
            Err(std::io::Error::other("origin failed")),
        ];
    }
    ["origin/main", "before", "local", "changed"]
        .into_iter()
        .enumerate()
        .map(|(index, text)| {
            if (failed == "HEAD" && index == 2) || (failed == "refreshed" && index == 3) {
                Err(std::io::Error::other("head failed"))
            } else {
                Ok(text.into())
            }
        })
        .collect()
}

#[test]
fn configured_update_matches_recorded_observations() {
    update_corpus::replay();
}

/// A successful clone makes the manifest visible to the existing dependency installer.
fn clone_acquires_dependencies() {
    let (effects, result, tree) = controlled_refresh(Vec::new(), |effects, _, checkout| {
        std::fs::remove_dir_all(checkout).unwrap();
        *effects.run_hook.borrow_mut() = Some(Rc::new(move |_, args, _| {
            if args[0] == "clone" {
                std::fs::write(format!("{}/package.json", args[2]), "{}").unwrap();
            }
            Box::pin(async { Ok(Some(0)) })
        }));
    });
    result.unwrap();
    assert_eq!(
        spelled(&effects, &tree),
        [
            (
                "git".into(),
                vec![
                    "clone".into(),
                    "https://github.com/acme/tool".into(),
                    "/home/reader/agent/git/github.com/acme/.controlled-stage".into()
                ]
            ),
            ("npm".into(), vec!["install".into(), "--omit=dev".into()]),
        ]
    );
}

#[test]
fn current_git_checkout_with_manifest_skips_dependency_install() {
    let responses = ["origin/main", "stale", "same", "same"]
        .into_iter()
        .map(|text| Ok(text.into()))
        .collect();
    let (effects, result, _tree) = controlled_refresh(responses, |_, _, _| {});
    result.unwrap();
    assert_eq!(effects.calls.borrow().len(), 5);
    assert!(
        effects
            .calls
            .borrow()
            .iter()
            .all(|(command, _)| command == "git")
    );
}
/// The commands run so far, with paths spelled as the fixture names them.
fn spelled(
    effects: &support::Effects,
    tree: &crate::scratch::Scratch,
) -> Vec<(String, Vec<String>)> {
    let base = tree.path().to_str().unwrap();
    let spell = |text: &str| support::recorded_stages(&text.replace(base, ""));
    effects
        .calls
        .borrow()
        .iter()
        .map(|(program, args)| (program.clone(), args.iter().map(|arg| spell(arg)).collect()))
        .collect()
}
