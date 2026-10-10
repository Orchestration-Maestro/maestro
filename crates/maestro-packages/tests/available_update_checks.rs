//! Available changes through the configured-source manager.
#![cfg(test)]
#![cfg(unix)]
mod native_support;
mod support;
use maestro_packages::{
    DefaultPackageManager, InstalledSourceScope, NativePackageOperations, PackageManagerOptions,
    PackageUpdateType,
};
use maestro_settings::{Settings, SettingsManager};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};

#[test]
fn native_and_controlled_checks_match_for_pinned_and_unpinned_sources() {
    let (manager, settings, effects) = support::manager(&json!({}));
    settings.borrow_mut().set_project_packages(vec![
        maestro_settings::PackageSource::Source("npm:example".into()),
        maestro_settings::PackageSource::Source("npm:pinned@1".into()),
    ]);
    effects.exists.set(true);
    let controlled = Rc::new(manager);
    let (runtime, local) = driver(&effects);
    let scratch = native_support::Scratch::new().unwrap();
    let cwd = scratch.0.to_string_lossy().into_owned();
    native_support::write(
        &format!("{cwd}/.maestro/npm/node_modules/example/package.json"),
        "{\"version\":\"1\"}",
    );
    let native_settings = Rc::new(RefCell::new(
        SettingsManager::in_memory(Settings::default()),
    ));
    native_settings.borrow_mut().set_project_packages(vec![
        maestro_settings::PackageSource::Source("npm:example".into()),
        maestro_settings::PackageSource::Source("npm:pinned@1".into()),
    ]);
    native_settings.borrow_mut().set_npm_command(Some(
        ["/bin/sh", "-c", "test \"$0\" = view && test \"$1\" = example && test \"$2\" = version && test \"$3\" = --json && printf '\"2\"'"]
            .map(|s| maestro_settings::SettingsListEntry::String(s.into()))
            .to_vec(),
    ));
    let native = native_manager(&cwd, native_settings.clone(), &local);
    let result = runtime.block_on(local.run_until(async {
        let controlled = controlled.check_for_available_updates().await.unwrap();
        let native = native.check_for_available_updates().await.unwrap();
        assert_eq!(controlled, native);
        native
    }));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].source, "npm:example");
    assert_eq!(result[0].display_name, "example");
    assert_eq!(result[0].r#type, PackageUpdateType::Npm);
    assert_eq!(result[0].scope, InstalledSourceScope::Project);
    assert_eq!(
        settings.borrow().get_project_settings().0["packages"],
        json!(["npm:example", "npm:pinned@1"])
    );
    assert_eq!(
        native_settings.borrow().get_project_settings().0["packages"],
        json!(["npm:example", "npm:pinned@1"])
    );
    assert_eq!(
        effects.calls.borrow().last().unwrap().1,
        ["view", "example", "version", "--json"]
    );
    replacement_manifest(
        &format!("{cwd}/.maestro/npm/node_modules/example/package.json"),
        &local,
        &runtime,
        &native,
        &result,
    );
}

/// A configured manager with controlled installed contents.
fn configured(
    user: &serde_json::Value,
    project: &serde_json::Value,
) -> (
    Rc<DefaultPackageManager<support::Controlled>>,
    Rc<RefCell<SettingsManager>>,
    Rc<support::Effects>,
) {
    let (manager, settings, effects) = support::manager(user);
    settings.borrow_mut().set_project_packages(
        project
            .as_array()
            .unwrap()
            .iter()
            .cloned()
            .map(maestro_settings::PackageSource::Unknown)
            .collect(),
    );
    effects.exists.set(true);
    (Rc::new(manager), settings, effects)
}
/// Drives the owned workers to completion, then closes their runtime.
fn check(
    manager: &Rc<DefaultPackageManager<support::Controlled>>,
    effects: &Rc<support::Effects>,
) -> std::io::Result<Vec<maestro_packages::PackageUpdate>> {
    let local = Rc::new(tokio::task::LocalSet::new());
    *effects.local.borrow_mut() = Rc::downgrade(&local);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = runtime.block_on(local.run_until(manager.check_for_available_updates()));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    result
}
/// Queues completed captured responses.
fn responses(effects: &support::Effects, values: &[&str]) {
    effects
        .capture_outputs
        .borrow_mut()
        .extend(values.iter().map(|v| Ok((*v).into())));
}
/// Ordered source and scope pairs returned to the caller.
fn sources(results: &[maestro_packages::PackageUpdate]) -> Vec<(&str, InstalledSourceScope)> {
    results
        .iter()
        .map(|r| (r.source.as_str(), r.scope))
        .collect()
}

#[test]
fn offline_values_short_circuit_before_settings() {
    for (value, offline) in [
        (None, false),
        (Some(""), false),
        (Some("0"), false),
        (Some("false"), false),
        (Some("1"), true),
        (Some("true"), true),
        (Some("TRUE"), true),
        (Some("yes"), true),
        (Some("YeS"), true),
        (Some(" yes"), false),
        (Some("true "), false),
        (Some("\u{feff}true"), false),
    ] {
        let (manager, _, effects) = configured(&json!({"packages":{}}), &json!([]));
        *effects.offline.borrow_mut() = value.map(str::to_owned);
        let result = check(&manager, &effects);
        assert_eq!(result.is_ok(), offline, "case {value:?}");
        assert!(effects.paths.borrow().is_empty());
        assert!(effects.calls.borrow().is_empty());
        assert_eq!(effects.spawns.get(), 0);
    }
}

#[test]
fn availability_reads_snapshots_then_project_first_identities() {
    let (manager, settings, effects) = configured(
        &json!({"packages":["npm:a","npm:u","git:git@github.com:org/repo.git","./local"]}),
        &json!(["npm:z", "npm:a", "https://github.com/org/repo", "./local"]),
    );
    *effects.capture_hook.borrow_mut() = Some(Rc::new(|cmd, args, _| {
        let text = if cmd == "npm" {
            "\"2\"".into()
        } else if args == ["rev-parse", "HEAD"] {
            "a".repeat(40)
        } else if args.get(1).is_some_and(|a| a == "--abbrev-ref") {
            "origin/main".into()
        } else {
            format!("{}\trefs/heads/main", "b".repeat(40))
        };
        Box::pin(async move { Ok(text) })
    }));
    let changed = settings.clone();
    *effects.on_command.borrow_mut() = Some(Box::new(move || {
        changed.borrow_mut().set_project_packages(vec![]);
    }));
    let result = check(&manager, &effects).unwrap();
    assert_eq!(
        sources(&result),
        [
            ("npm:z", InstalledSourceScope::Project),
            ("npm:a", InstalledSourceScope::Project),
            ("https://github.com/org/repo", InstalledSourceScope::Project),
            ("npm:u", InstalledSourceScope::User)
        ]
    );
    assert_eq!(
        settings.borrow().get_project_settings().0["packages"],
        json!([])
    );
    assert!(
        !effects
            .paths
            .borrow()
            .iter()
            .any(|p| p == "/home/reader/agent/git/github.com/org/repo")
    );
}

#[test]
fn availability_preserves_first_same_scope_source_and_pin_precedence() {
    let (manager, settings, effects) = configured(
        &json!({"packages":["npm:shadow","npm:missing"]}),
        &json!([{"source":"npm:first","skills":["x"]}, "npm:first@1", "npm:shadow@latest", "npm:missing"]),
    );
    *effects.capture_hook.borrow_mut() = Some(Rc::new(|_, args, _| {
        assert_eq!(args[1], "first");
        Box::pin(async { Ok("\"2\"".into()) })
    }));
    let absent = effects.clone();
    *effects.capture_hook.borrow_mut() = Some(Rc::new(move |_, args, _| {
        assert_eq!(args[1], "first");
        absent.exists.set(false);
        Box::pin(async { Ok("\"2\"".into()) })
    }));
    let original = settings.borrow().get_project_settings();
    let result = check(&manager, &effects).unwrap();
    assert_eq!(
        sources(&result),
        [("npm:first", InstalledSourceScope::Project)]
    );
    assert_eq!(effects.calls.borrow().len(), 1);
    assert_eq!(settings.borrow().get_project_settings(), original);
    effects.capture_hook.borrow_mut().take();
}

#[test]
fn availability_skips_local_pins_before_existence() {
    let pins = json!([
        "npm:fixed@1.0.0",
        "npm:tag@latest",
        "npm:range@^1",
        "npm:@scope/pkg@~2",
        "git:github.com/org/repo@main",
        "./local",
        "npm:control"
    ]);
    let (manager, _, effects) = configured(&json!({}), &pins);
    let result = check(&manager, &effects).unwrap();
    assert_eq!(
        sources(&result),
        [("npm:control", InstalledSourceScope::Project)]
    );
    assert_eq!(
        *effects.paths.borrow(),
        [
            "/work/project/.maestro/npm/node_modules/control",
            "/work/project/.maestro/npm/node_modules/control/package.json"
        ]
    );
}

#[test]
fn availability_ignores_missing_contents_without_installing() {
    let (manager, settings, effects) = configured(
        &json!({}),
        &json!(["npm:missing", "git:github.com/org/repo"]),
    );
    effects.exists.set(false);
    let initial = settings.borrow().get_project_settings();
    assert!(check(&manager, &effects).unwrap().is_empty());
    assert_eq!(effects.calls.borrow().len(), 0);
    assert_eq!(effects.paths.borrow().len(), 2);
    assert_eq!(settings.borrow().get_project_settings(), initial);
    let (empty, _, empty_effects) = configured(&json!({}), &json!([]));
    assert!(check(&empty, &empty_effects).unwrap().is_empty());
    assert_eq!(empty_effects.spawns.get(), 0);
}

#[test]
fn npm_availability_compares_exact_version_strings() {
    for (installed, latest, expected) in [
        ("1.0", "\"1.0\"", false),
        ("1.0", "\"2.0\"", true),
        ("1.0.0", "\"1.0\"", true),
        ("1", "\"\"", true),
        ("1", "\"\\u0031\"", false),
        (" 1 ", "\"1\"", true),
    ] {
        let (manager, _, effects) = configured(&json!({}), &json!(["npm:example"]));
        *effects.manifest.borrow_mut() = Some(json!({"version": installed}).to_string());
        responses(&effects, &[latest]);
        assert_eq!(
            check(&manager, &effects).unwrap().len(),
            usize::from(expected)
        );
        assert_eq!(effects.calls.borrow().len(), 1);
    }
}

#[test]
fn npm_version_files_preserve_selected_field_semantics() {
    for (manifest, expected, calls) in [
        (None, false, 0),
        (Some("{broken"), false, 0),
        (Some("{}"), false, 0),
        (Some("{\"version\":null}"), false, 0),
        (Some("{\"version\":\"\"}"), false, 0),
        (Some("{\"version\":0}"), false, 0),
        (Some("{\"version\":false}"), false, 0),
        (Some("{\"version\":true}"), false, 0),
        (Some("{\"version\":[]}"), false, 0),
        (Some("{\"version\":{}}"), false, 0),
        (Some("{\"version\":[\"1\"]}"), false, 0),
        (Some("{\"version\":1}"), false, 0),
        (Some("[]"), false, 0),
        (Some("null"), false, 0),
        (Some("3"), false, 0),
        (Some("\"text\""), false, 0),
        (
            Some("{\"unknown\":{\"deep\":[1,2]},\"version\":\"1\"}"),
            true,
            1,
        ),
        (Some("{\"version\":\"1\",\"version\":\"2\"}"), false, 1),
        (Some("{\"version\":\"1\",\"version\":null}"), false, 0),
        (Some("{\"\\u0076ersion\":\"1\"}"), true, 1),
    ] {
        let (manager, _, effects) = configured(&json!({}), &json!(["npm:example"]));
        *effects.manifest.borrow_mut() = manifest.map(str::to_owned);
        assert_eq!(
            check(&manager, &effects).unwrap().len(),
            usize::from(expected),
            "manifest case"
        );
        assert_eq!(effects.calls.borrow().len(), calls);
    }
}

#[test]
fn npm_lookup_failures_are_suppressed_without_effects() {
    for (latest, expected) in [
        (" \u{feff}\n", false),
        ("not-json", false),
        ("\u{85}\"2\"", false),
        ("null", false),
        ("false", false),
        ("123", false),
        ("{}", false),
        ("[]", false),
        (" \u{feff}\"2\"\r\n", true),
    ] {
        let (manager, _, effects) = configured(&json!({}), &json!(["npm:example"]));
        responses(&effects, &[latest]);
        assert_eq!(
            check(&manager, &effects).unwrap().len(),
            usize::from(expected)
        );
        assert_eq!(effects.calls.borrow().len(), 1);
    }
    let (manager, _, effects) = configured(&json!({}), &json!(["npm:example"]));
    effects
        .capture_outputs
        .borrow_mut()
        .push_back(Err(std::io::Error::other("lookup failed")));
    assert!(check(&manager, &effects).unwrap().is_empty());
    assert_eq!(effects.calls.borrow().len(), 1);
}

#[test]
fn npm_capture_uses_live_prefix_cwd_and_timeout() {
    let (manager, settings, effects) =
        configured(&json!({"packages":["npm:@scope/pkg"]}), &json!([]));
    let changed = settings;
    *effects.on_command.borrow_mut() = Some(Box::new(move || {
        changed.borrow_mut().set_npm_command(Some(
            ["custom", "--flag"]
                .map(|s| maestro_settings::SettingsListEntry::String(s.into()))
                .to_vec(),
        ));
    }));
    let result = check(&manager, &effects).unwrap();
    assert_eq!(result[0].display_name, "@scope/pkg");
    assert_eq!(
        *effects.calls.borrow(),
        [
            ("npm".into(), vec!["root".into(), "-g".into()]),
            (
                "custom".into(),
                ["--flag", "view", "@scope/pkg", "version", "--json"]
                    .map(str::to_owned)
                    .to_vec()
            )
        ]
    );
    assert_eq!(
        *effects.operands.borrow(),
        [(
            Some("/work/project".into()),
            Some(std::time::Duration::from_millis(10000)),
            vec![]
        )]
    );
}

#[test]
fn git_availability_uses_origin_upstream_then_head() {
    for upstream in ["origin/main", "", "other/main", "origin/"] {
        let (manager, _, effects) = configured(&json!({}), &json!(["git:github.com/org/repo"]));
        responses(
            &effects,
            &[
                &format!(" \u{feff}{}\n", "a".repeat(40)),
                upstream,
                &format!(
                    "{}\t{}",
                    "b".repeat(40),
                    if upstream == "origin/main" {
                        "refs/heads/main"
                    } else {
                        "HEAD"
                    }
                ),
            ],
        );
        let result = check(&manager, &effects).unwrap();
        assert_eq!(result[0].source, "git:github.com/org/repo");
        assert_eq!(result[0].display_name, "github.com/org/repo");
        let calls = effects.calls.borrow();
        assert_eq!(calls[0].1, ["rev-parse", "HEAD"]);
        assert_eq!(calls[1].1, ["rev-parse", "--abbrev-ref", "@{upstream}"]);
        assert_eq!(
            calls[2].1,
            [
                "ls-remote",
                "origin",
                if upstream == "origin/main" {
                    "refs/heads/main"
                } else {
                    "HEAD"
                }
            ]
        );
        let operands = effects.operands.borrow();
        assert!(operands[..2].iter().all(|o| o.2.is_empty()));
        assert_eq!(operands[2].2, [("GIT_TERMINAL_PROMPT".into(), "0".into())]);
        assert!(operands.iter().all(|o| o.0.as_deref()
            == Some("/work/project/.maestro/git/github.com/org/repo")
            && o.1 == Some(std::time::Duration::from_millis(10000))));
    }
}

#[test]
fn git_remote_failures_obey_fallback_boundaries() {
    let (manager, _, effects) = configured(&json!({}), &json!(["git:github.com/org/repo"]));
    responses(
        &effects,
        &[
            &"a".repeat(40),
            "origin/main",
            "no matching hash",
            &format!("{}\tHEAD", "b".repeat(40)),
        ],
    );
    assert_eq!(check(&manager, &effects).unwrap().len(), 1);
    assert_eq!(effects.calls.borrow().len(), 4);
    for phase in 0..4 {
        let (manager, _, effects) = configured(&json!({}), &json!(["git:github.com/org/repo"]));
        let upstream = if phase == 1 { "origin/main" } else { "" };
        *effects.capture_hook.borrow_mut() = Some(Rc::new(move |_, args, _| {
            let outcome = if (phase == 0 && args == ["rev-parse", "HEAD"])
                || (phase == 1 && args.get(2).is_some_and(|a| a == "refs/heads/main"))
                || (phase == 2 && args == ["ls-remote", "origin", "HEAD"])
            {
                Err(std::io::Error::other("probe failed"))
            } else if args == ["rev-parse", "HEAD"] {
                Ok("a".repeat(40))
            } else if args.get(1).is_some_and(|a| a == "--abbrev-ref") {
                Ok(upstream.into())
            } else {
                assert_eq!(args, ["ls-remote", "origin", "HEAD"]);
                Ok("invalid HEAD".into())
            };
            Box::pin(async move { outcome })
        }));
        assert!(check(&manager, &effects).unwrap().is_empty());
        assert_eq!(effects.calls.borrow().len(), if phase == 0 { 1 } else { 3 });
    }
}

/// Recorded remote-output inputs and their observable availability result.
const REMOTE_HEAD_CASES: &[(&[&str], bool)] = &[
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "origin/main",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\trefs/heads/main\n",
        ],
        false,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "origin/main",
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB\trefs/heads/main",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\tHEAD",
        ],
        true,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "origin/main",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\trefs/heads/main",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\tHEAD",
        ],
        true,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "origin/main",
            "xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\trefs/heads/main",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\tHEAD",
        ],
        true,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "origin/main",
            "noise\nbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\tanything",
        ],
        true,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "other/main",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\tHEAD extra",
        ],
        false,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "other/main",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\tHEAD\r\n",
        ],
        true,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "origin/main",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb﻿refs/heads/main",
        ],
        true,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "origin/main",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbrefs/heads/main",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\tHEAD",
        ],
        false,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "origin/main",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\trefs/heads/main",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\tHEAD",
        ],
        false,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "origin/main",
            "noise\rbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\tref",
        ],
        true,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "origin/main",
            "noise bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\tref",
        ],
        true,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "origin/main",
            "noise bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\tref",
        ],
        true,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "other/main",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n\tHEAD",
        ],
        true,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "other/main",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\tHEAD extra",
        ],
        true,
    ),
    (
        &[
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "other/main",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\t",
        ],
        false,
    ),
];

#[test]
fn git_head_matching_keeps_hash_line_and_whitespace_rules() {
    for (outputs, changed) in REMOTE_HEAD_CASES {
        let (manager, _, effects) = configured(&json!({}), &json!(["git:github.com/org/repo"]));
        responses(&effects, outputs);
        assert_eq!(
            check(&manager, &effects).unwrap().len(),
            usize::from(*changed)
        );
        assert_eq!(effects.calls.borrow().len(), outputs.len());
    }
}

#[test]
fn offline_changes_are_observed_at_per_source_checks() {
    let (manager, _, effects) = configured(
        &json!({}),
        &json!(["npm:example", "git:github.com/org/repo"]),
    );
    effects
        .offline_sequence
        .borrow_mut()
        .extend([None, Some("yes".into()), Some("1".into())]);
    assert!(check(&manager, &effects).unwrap().is_empty());
    assert!(effects.calls.borrow().is_empty());
    assert_eq!(effects.paths.borrow().len(), 2);
    let (manager, _, effects) = configured(&json!({}), &json!(["npm:example"]));
    let changed = effects.clone();
    *effects.capture_hook.borrow_mut() = Some(Rc::new(move |_, _, _| {
        *changed.offline.borrow_mut() = Some("true".into());
        Box::pin(async { Ok("\"2\"".into()) })
    }));
    assert_eq!(check(&manager, &effects).unwrap().len(), 1);
    effects.capture_hook.borrow_mut().take();
}

#[test]
fn invalid_settings_and_ambient_reads_follow_consumption_order() {
    for packages in [json!(null), json!([])] {
        let (manager, _, effects) = configured(&json!({"packages":packages}), &json!([]));
        assert!(check(&manager, &effects).unwrap().is_empty());
    }
    for packages in [
        json!({}),
        json!([null]),
        json!([{"skills":[]}]),
        json!([{"source":3}]),
        json!(["npm:good", null]),
    ] {
        let (manager, _, effects) = configured(&json!({"packages":packages}), &json!([]));
        assert!(check(&manager, &effects).is_err());
        assert_eq!(effects.spawns.get(), 0);
        assert!(effects.calls.borrow().is_empty());
    }
    for (user, project, expected) in [
        (
            json!({"packages":{}}),
            json!(["./local"]),
            "packages is not an array",
        ),
        (json!({}), json!(["./local", null]), "cwd unavailable"),
        (
            json!({}),
            json!(["/absolute", null]),
            "package source is not a string",
        ),
    ] {
        let (_, settings, effects) = configured(&user, &project);
        effects.fail_ambient.set(true);
        let manager = Rc::new(DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: "relative".into(),
                agent_dir: "relative-agent".into(),
                settings_manager: settings,
            },
            support::Controlled(effects.clone()),
        ));
        assert_eq!(check(&manager, &effects).unwrap_err().to_string(), expected);
        assert!(effects.calls.borrow().is_empty());
        assert_eq!(
            effects.reads.borrow().len(),
            usize::from(expected == "cwd unavailable")
        );
    }
}

#[test]
fn check_calls_do_not_hold_settings_borrows_across_effects() {
    let (manager, settings, effects) =
        configured(&json!({"packages":["npm:first","npm:second"]}), &json!([]));
    let state = settings.clone();
    *effects.capture_hook.borrow_mut() = Some(Rc::new(move |command, args, _| {
        if args[1] == "first" {
            assert_eq!(command, "npm");
            state.borrow_mut().set_packages(vec![]);
            state.borrow_mut().set_npm_command(Some(vec![
                maestro_settings::SettingsListEntry::String("changed".into()),
            ]));
        } else {
            assert_eq!(command, "changed");
        }
        Box::pin(async { Ok("\"2\"".into()) })
    }));
    let result = check(&manager, &effects).unwrap();
    assert_eq!(
        sources(&result),
        [
            ("npm:first", InstalledSourceScope::User),
            ("npm:second", InstalledSourceScope::User)
        ]
    );
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!([])
    );
}

/// Controls a capture at its first awaited probe for each source.
struct Gates {
    /// Completion permits indexed by selected source.
    release: RefCell<std::collections::BTreeMap<usize, tokio::sync::oneshot::Sender<()>>>,
    /// Admission witnesses from the producer.
    starts: tokio::sync::mpsc::UnboundedSender<usize>,
}
impl Gates {
    /// Creates one gate per source and observes admission at the capture boundary.
    fn install(
        effects: &support::Effects,
        count: usize,
    ) -> (Rc<Self>, tokio::sync::mpsc::UnboundedReceiver<usize>) {
        let (starts, receive) = tokio::sync::mpsc::unbounded_channel();
        let gates = Rc::new(Self {
            release: RefCell::default(),
            starts,
        });
        let (release, mut wait): (
            std::collections::BTreeMap<_, _>,
            std::collections::BTreeMap<_, _>,
        ) = (0..count)
            .map(|i| {
                let (tx, rx) = tokio::sync::oneshot::channel();
                ((i, tx), (i, rx))
            })
            .unzip();
        *gates.release.borrow_mut() = release;
        let wait = Rc::new(RefCell::new(std::mem::take(&mut wait)));
        let observed = gates.clone();
        *effects.capture_hook.borrow_mut() = Some(gated_capture(observed, wait));
        (gates, receive)
    }
    /// Releases exactly one admitted capture.
    fn finish(&self, index: usize) {
        let release = self.release.borrow_mut().remove(&index).unwrap();
        release.send(()).unwrap();
    }
}
/// Produces native-kind responses around one gated initial probe.
fn gated_capture(
    observed: Rc<Gates>,
    wait: Rc<RefCell<std::collections::BTreeMap<usize, tokio::sync::oneshot::Receiver<()>>>>,
) -> support::CaptureHook {
    Rc::new(move |command, args, cwd| {
        let initial = command != "git" || args == ["rev-parse", "HEAD"];
        if !initial {
            let text = if args[0] == "rev-parse" {
                String::new()
            } else {
                format!("{}\tHEAD", "b".repeat(40))
            };
            return Box::pin(async move { Ok(text) });
        }
        let label = if command == "git" {
            cwd.unwrap().rsplit('/').next().unwrap()
        } else {
            args[1].as_str()
        };
        let index: usize = label.strip_prefix('p').unwrap().parse().unwrap();
        let gate = wait.borrow_mut().remove(&index).unwrap();
        let starts = observed.starts.clone();
        let text = if command == "git" {
            "a".repeat(40)
        } else {
            "\"2\"".into()
        };
        Box::pin(async move {
            starts.send(index).unwrap();
            gate.await.unwrap();
            Ok(text)
        })
    })
}
/// Native local runtime used by concurrency witnesses.
fn driver(effects: &support::Effects) -> (tokio::runtime::Runtime, Rc<tokio::task::LocalSet>) {
    let local = Rc::new(tokio::task::LocalSet::new());
    *effects.local.borrow_mut() = Rc::downgrade(&local);
    (
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap(),
        local,
    )
}
/// Bounds a broken asynchronous witness without relying on a delay for correctness.
async fn bounded<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(std::time::Duration::from_secs(5), future)
        .await
        .expect("controlled completion witness did not fire")
}

#[test]
fn availability_returns_typed_records_in_source_order() {
    let (manager, _, effects) = configured(
        &json!({"packages":["npm:p2","git:github.com/org/p3"]}),
        &json!(["git:github.com/org/p0", "npm:p1"]),
    );
    let (runtime, local) = driver(&effects);
    let (gates, mut starts) = Gates::install(&effects, 4);
    let result = local.spawn_local(manager.check_for_available_updates());
    runtime.block_on(local.run_until(async {
        for index in 0..4 {
            assert_eq!(bounded(starts.recv()).await, Some(index));
        }
        for index in (0..4).rev() {
            gates.finish(index);
        }
        let result = bounded(result).await.unwrap().unwrap();
        assert_eq!(
            sources(&result),
            [
                ("git:github.com/org/p0", InstalledSourceScope::Project),
                ("npm:p1", InstalledSourceScope::Project),
                ("npm:p2", InstalledSourceScope::User),
                ("git:github.com/org/p3", InstalledSourceScope::User)
            ]
        );
        assert_eq!(
            result
                .iter()
                .map(|r| (r.display_name.as_str(), r.r#type))
                .collect::<Vec<_>>(),
            [
                ("github.com/org/p0", PackageUpdateType::Git),
                ("p1", PackageUpdateType::Npm),
                ("p2", PackageUpdateType::Npm),
                ("github.com/org/p3", PackageUpdateType::Git)
            ]
        );
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
}

#[test]
fn four_workers_claim_slots_without_global_serialization() {
    for count in [0, 1, 4, 7] {
        check_pool_size(count);
    }
    let (manager, _, effects) =
        configured(&json!({}), &json!(["npm:p0", "npm:p1", "npm:p2", "npm:p3"]));
    let (runtime, local) = driver(&effects);
    let (send, mut started) = tokio::sync::mpsc::unbounded_channel();
    let (release, wait) = tokio::sync::watch::channel(false);
    *effects.capture_hook.borrow_mut() = Some(Rc::new(move |_, _, _| {
        let send = send.clone();
        let mut wait = wait.clone();
        Box::pin(async move {
            send.send(()).unwrap();
            wait.wait_for(|ready| *ready).await.unwrap();
            Ok("\"2\"".into())
        })
    }));
    let first = local.spawn_local(manager.check_for_available_updates());
    let second = local.spawn_local(manager.check_for_available_updates());
    runtime.block_on(local.run_until(async {
        for _ in 0..8 {
            bounded(started.recv()).await.unwrap();
        }
        assert_eq!(effects.spawns.get(), 8);
        release.send(true).unwrap();
        assert_eq!(bounded(first).await.unwrap().unwrap().len(), 4);
        assert_eq!(bounded(second).await.unwrap().unwrap().len(), 4);
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
}

#[test]
fn first_failure_keeps_other_workers_claiming_sources() {
    let (manager, _, effects) = configured(
        &json!({"packages":["npm:fail","git:github.com/org/p3","git:github.com/org/p4","git:github.com/org/p5"]}),
        &json!([
            "git:github.com/org/p0",
            "git:github.com/org/p1",
            "git:github.com/org/p2"
        ]),
    );
    effects
        .outputs
        .borrow_mut()
        .push_back(Err(std::io::Error::other("first failure")));
    let (runtime, local) = driver(&effects);
    let (gates, mut starts) = Gates::install(&effects, 6);
    let result = local.spawn_local(manager.check_for_available_updates());
    runtime.block_on(local.run_until(async {
        for i in 0..3 {
            assert_eq!(bounded(starts.recv()).await, Some(i));
        }
        assert!(
            bounded(result)
                .await
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("first failure")
        );
        gates.finish(1);
        assert_eq!(bounded(starts.recv()).await, Some(3));
        gates.finish(3);
        assert_eq!(bounded(starts.recv()).await, Some(4));
        gates.finish(4);
        assert_eq!(bounded(starts.recv()).await, Some(5));
        for i in [0, 2, 5] {
            gates.finish(i);
        }
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    assert!(gates.release.borrow().is_empty());
}

#[test]
fn availability_releases_last_owner_after_remaining_work() {
    let (manager, _, effects) = configured(
        &json!({"packages":["npm:fail","git:github.com/org/p3"]}),
        &json!([
            "git:github.com/org/p0",
            "git:github.com/org/p1",
            "git:github.com/org/p2"
        ]),
    );
    effects
        .outputs
        .borrow_mut()
        .push_back(Err(std::io::Error::other("first failure")));
    let weak = Rc::downgrade(&manager);
    let (runtime, local) = driver(&effects);
    let (gates, mut starts) = Gates::install(&effects, 4);
    let result = local.spawn_local(manager.check_for_available_updates());
    drop(manager);
    runtime.block_on(local.run_until(async {
        for i in 0..3 {
            assert_eq!(bounded(starts.recv()).await, Some(i));
        }
        assert!(bounded(result).await.unwrap().is_err());
        assert!(weak.upgrade().is_some());
        gates.finish(0);
        assert_eq!(bounded(starts.recv()).await, Some(3));
        for i in [1, 2, 3] {
            gates.finish(i);
        }
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    assert!(weak.upgrade().is_none());
}

/// Runs Git only in the test's disposable tree.
fn git_at(path: &std::path::Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(path)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(output.status.success(), "sandbox Git command failed");
    String::from_utf8(output.stdout).unwrap().trim().into()
}
#[test]
fn native_git_availability_leaves_head_and_contents_unchanged() {
    let scratch = native_support::Scratch::new().unwrap();
    let root = &scratch.0;
    let (author, install) = seed_git(root);
    let original = git_at(&install, &["rev-parse", "HEAD"]);
    let settings = Rc::new(RefCell::new(
        SettingsManager::in_memory(Settings::default()),
    ));
    settings
        .borrow_mut()
        .set_project_packages(vec![maestro_settings::PackageSource::Source(
            "git:github.com/org/repo".into(),
        )]);
    let before = settings.borrow().get_project_settings();
    let local = Rc::new(tokio::task::LocalSet::new());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let manager = native_manager(root.to_str().unwrap(), settings.clone(), &local);
    assert!(
        runtime
            .block_on(local.run_until(manager.check_for_available_updates()))
            .unwrap()
            .is_empty()
    );
    std::fs::write(author.join("contents"), "two").unwrap();
    git_at(&author, &["commit", "-am", "two"]);
    git_at(&author, &["push", "origin", "HEAD"]);
    let changed = runtime
        .block_on(local.run_until(manager.check_for_available_updates()))
        .unwrap();
    assert_eq!(
        sources(&changed),
        [("git:github.com/org/repo", InstalledSourceScope::Project)]
    );
    git_at(&install, &["checkout", "--detach"]);
    assert_eq!(
        runtime
            .block_on(local.run_until(manager.check_for_available_updates()))
            .unwrap(),
        changed
    );
    assert_eq!(git_at(&install, &["rev-parse", "HEAD"]), original);
    assert_eq!(
        git_at(&install, &["rev-parse", "refs/remotes/origin/HEAD"]),
        original
    );
    assert_eq!(
        std::fs::read_to_string(install.join("contents")).unwrap(),
        "one"
    );
    assert_eq!(settings.borrow().get_project_settings(), before);
}

/// Proves admission and replacement at a chosen pool size.
fn check_pool_size(count: usize) {
    let list: Vec<_> = (0..count)
        .map(|i| {
            if i % 2 == 0 {
                format!("npm:p{i}")
            } else {
                format!("git:github.com/org/p{i}")
            }
        })
        .collect();
    let (manager, _, effects) = configured(&json!({}), &json!(list));
    let (runtime, local) = driver(&effects);
    let (gates, mut starts) = Gates::install(&effects, count);
    let result = local.spawn_local(manager.check_for_available_updates());
    runtime.block_on(local.run_until(async {
        for index in 0..count.min(4) {
            assert_eq!(bounded(starts.recv()).await, Some(index));
        }
        if count > 0 {
            assert_eq!(effects.spawns.get(), count.min(4));
        }
        let completion: Vec<_> = if count == 7 {
            vec![3, 2, 1, 6, 5, 4, 0]
        } else {
            (0..count).collect()
        };
        for (completed, index) in completion.into_iter().enumerate() {
            gates.finish(index);
            if completed + 4 < count {
                assert_eq!(bounded(starts.recv()).await, Some(completed + 4));
            }
        }
        let result = bounded(result).await.unwrap().unwrap();
        let ordered: Vec<_> = result.iter().map(|r| r.display_name.as_str()).collect();
        let expected = &[
            "p0",
            "github.com/org/p1",
            "p2",
            "github.com/org/p3",
            "p4",
            "github.com/org/p5",
            "p6",
        ][..count];
        assert_eq!(ordered, expected);
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
}

/// Seeds a local remote, its author and an installed checkout.
fn seed_git(root: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    git_at(root, &["init", "--bare", "origin"]);
    git_at(root, &["clone", "origin", "author"]);
    let author = root.join("author");
    git_at(&author, &["config", "user.name", "Fixture"]);
    git_at(
        &author,
        &["config", "user.email", "fixture@example.invalid"],
    );
    std::fs::write(author.join("contents"), "one").unwrap();
    git_at(&author, &["add", "contents"]);
    git_at(&author, &["commit", "-m", "one"]);
    git_at(&author, &["push", "origin", "HEAD"]);
    let install = root.join(".maestro/git/github.com/org/repo");
    std::fs::create_dir_all(install.parent().unwrap()).unwrap();
    git_at(
        root,
        &[
            "clone",
            root.join("origin").to_str().unwrap(),
            install.to_str().unwrap(),
        ],
    );
    (author, install)
}

/// Proves replacement decoding at native file I/O before manifest selection.
fn replacement_manifest(
    path: &str,
    local: &Rc<tokio::task::LocalSet>,
    runtime: &tokio::runtime::Runtime,
    manager: &Rc<DefaultPackageManager<NativePackageOperations>>,
    expected: &[maestro_packages::PackageUpdate],
) {
    use maestro_packages::PackageOperations;
    std::fs::write(path, b"{\"version\":\"\xff\"}").unwrap();
    let operations = NativePackageOperations::new(|_| false, Rc::new(|| false), local);
    assert_eq!(operations.read_file(path).unwrap(), "{\"version\":\"�\"}");
    assert_eq!(
        runtime
            .block_on(local.run_until(manager.check_for_available_updates()))
            .unwrap(),
        expected
    );
}

/// Wires native effects to the test's surviving settings and local runtime.
fn native_manager(
    cwd: &str,
    settings: Rc<RefCell<SettingsManager>>,
    local: &Rc<tokio::task::LocalSet>,
) -> Rc<DefaultPackageManager<NativePackageOperations>> {
    Rc::new(DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: cwd.into(),
            agent_dir: format!("{cwd}/agent"),
            settings_manager: settings,
        },
        NativePackageOperations::new(|_| false, Rc::new(|| false), local),
    ))
}
