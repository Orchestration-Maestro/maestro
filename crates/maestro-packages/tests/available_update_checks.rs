//! Available changes through the configured-source manager.
#![cfg(test)]
#![cfg(unix)]
use crate::native_support;
use crate::resolution_support::{case_in_child, case_in_removed_cwd};
use crate::scratch;
use crate::support;
use crate::worker_support;
use maestro_packages::{
    CommandCaptureOptions, CommandOutput, DefaultPackageManager, InstalledSourceScope,
    NativePackageOperations, PackageFuture, PackageManagerOptions, PackageOperations,
    PackageUpdateType,
};
use maestro_settings::{Settings, SettingsManager};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};
use worker_support::{Gates, driver, gated_npm_checks};

#[test]
fn native_and_controlled_checks_match_for_pinned_and_unpinned_sources() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (controlled, settings, effects, _tree) =
        support::installed(&json!({}), &json!(["npm:example", "npm:pinned@1"]));
    let (runtime, local) = driver(&effects);
    let scratch = scratch::Scratch::new().unwrap();
    let cwd = scratch.path().to_string_lossy().into_owned();
    native_support::write(
        format!("{cwd}/.maestro/npm/node_modules/example/package.json"),
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

/// Drives the owned workers to completion, then closes their runtime.
fn check(
    manager: &Rc<DefaultPackageManager<support::Controlled>>,
    effects: &Rc<support::Effects>,
) -> std::io::Result<Vec<maestro_packages::PackageUpdate>> {
    support::drive(effects, manager.check_for_available_updates())
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
    for (index, (value, offline)) in [
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
    ]
    .into_iter()
    .enumerate()
    {
        let environment = |_: &str| vec![("MAESTRO_OFFLINE", value.map(str::to_owned))];
        if case_in_child(&index.to_string(), environment).is_none() {
            continue;
        }
        let (manager, _, effects, _tree) = support::installed(&json!({"packages":{}}), &json!([]));
        let result = check(&manager, &effects);
        assert_eq!(result.is_ok(), offline, "case {value:?}");
        assert!(effects.calls.borrow().is_empty());
        assert_eq!(effects.spawns.get(), 0);
    }
}

#[test]
fn availability_reads_snapshots_then_project_first_identities() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, settings, effects, _tree) = support::installed(
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
}

#[test]
fn availability_preserves_first_same_scope_source_and_pin_precedence() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, settings, effects, tree) = support::installed(
        &json!({"packages":["npm:shadow","npm:missing"]}),
        &json!([{"source":"npm:first","skills":["x"]}, "npm:first@1", "npm:shadow@latest", "npm:missing"]),
    );
    std::fs::remove_dir_all(format!("{}/missing", support::npm_root(tree.path(), false))).unwrap();
    *effects.capture_hook.borrow_mut() = Some(Rc::new(|_, args, _| {
        assert_eq!(args[1], "first");
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
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let pins = json!([
        "npm:fixed@1.0.0",
        "npm:tag@latest",
        "npm:range@^1",
        "npm:@scope/pkg@~2",
        "git:github.com/org/repo@main",
        "./local",
        "npm:control"
    ]);
    let (manager, _, effects, _tree) = support::installed(&json!({}), &pins);
    let result = check(&manager, &effects).unwrap();
    assert_eq!(
        sources(&result),
        [("npm:control", InstalledSourceScope::Project)]
    );
    assert_eq!(effects.calls.borrow().len(), 1);
}

#[test]
fn availability_ignores_missing_contents_without_installing() {
    let (manager, settings, effects, tree) = support::uninstalled(
        &json!({}),
        &json!(["npm:missing", "git:github.com/org/repo"]),
    );
    let initial = settings.borrow().get_project_settings();
    assert!(check(&manager, &effects).unwrap().is_empty());
    assert_eq!(effects.calls.borrow().len(), 0);
    assert_eq!(std::fs::read_dir(tree.path()).unwrap().count(), 0);
    assert_eq!(settings.borrow().get_project_settings(), initial);
    let (empty, _, empty_effects, _tree) = support::uninstalled(&json!({}), &json!([]));
    assert!(check(&empty, &empty_effects).unwrap().is_empty());
    assert_eq!(empty_effects.spawns.get(), 0);
}

#[test]
fn npm_availability_compares_exact_version_strings() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    for (installed, latest, expected) in [
        ("1.0", "\"1.0\"", false),
        ("1.0", "\"2.0\"", true),
        ("1.0.0", "\"1.0\"", true),
        ("1", "\"\"", true),
        ("1", "\"\\u0031\"", false),
        (" 1 ", "\"1\"", true),
    ] {
        let (manager, _, effects, tree) = support::installed(&json!({}), &json!(["npm:example"]));
        let text = json!({"version": installed}).to_string();
        support::project_manifest(&tree, "example", Some(&text));
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
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
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
        let (manager, _, effects, tree) = support::installed(&json!({}), &json!(["npm:example"]));
        support::project_manifest(&tree, "example", manifest);
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
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
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
        let (manager, _, effects, _tree) = support::installed(&json!({}), &json!(["npm:example"]));
        responses(&effects, &[latest]);
        assert_eq!(
            check(&manager, &effects).unwrap().len(),
            usize::from(expected)
        );
        assert_eq!(effects.calls.borrow().len(), 1);
    }
    let (manager, _, effects, _tree) = support::installed(&json!({}), &json!(["npm:example"]));
    effects
        .capture_outputs
        .borrow_mut()
        .push_back(Err(std::io::Error::other("lookup failed")));
    assert!(check(&manager, &effects).unwrap().is_empty());
    assert_eq!(effects.calls.borrow().len(), 1);
}

#[test]
fn npm_capture_uses_live_prefix_cwd_and_timeout() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, settings, effects, tree) =
        support::installed(&json!({"packages":["npm:@scope/pkg"]}), &json!([]));
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
            Some(support::below(tree.path(), "/work/project")),
            Some(std::time::Duration::from_millis(10000)),
            vec![]
        )]
    );
}

#[test]
fn git_availability_uses_origin_upstream_then_head() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    for upstream in ["origin/main", "", "other/main", "origin/"] {
        let (manager, _, effects, tree) =
            support::installed(&json!({}), &json!(["git:github.com/org/repo"]));
        let install = support::below(
            tree.path(),
            "/work/project/.maestro/git/github.com/org/repo",
        );
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
        assert!(
            operands
                .iter()
                .all(|o| o.0.as_deref() == Some(install.as_str())
                    && o.1 == Some(std::time::Duration::from_millis(10000)))
        );
    }
}

#[test]
fn git_remote_failures_obey_fallback_boundaries() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, _, effects, _tree) =
        support::installed(&json!({}), &json!(["git:github.com/org/repo"]));
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
        let (manager, _, effects, _tree) =
            support::installed(&json!({}), &json!(["git:github.com/org/repo"]));
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
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    for (outputs, changed) in REMOTE_HEAD_CASES {
        let (manager, _, effects, _tree) =
            support::installed(&json!({}), &json!(["git:github.com/org/repo"]));
        responses(&effects, outputs);
        assert_eq!(
            check(&manager, &effects).unwrap().len(),
            usize::from(*changed)
        );
        assert_eq!(effects.calls.borrow().len(), outputs.len());
    }
}

#[test]
fn invalid_settings_and_ambient_reads_follow_consumption_order() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_some() {
        for packages in [json!(null), json!([])] {
            let (manager, _, effects, _tree) =
                support::installed(&json!({"packages":packages}), &json!([]));
            assert!(check(&manager, &effects).unwrap().is_empty());
        }
        for packages in [
            json!({}),
            json!([null]),
            json!([{"skills":[]}]),
            json!([{"source":3}]),
            json!(["npm:good", null]),
        ] {
            let (manager, _, effects, _tree) =
                support::installed(&json!({"packages":packages}), &json!([]));
            assert!(check(&manager, &effects).is_err());
            assert_eq!(effects.spawns.get(), 0);
            assert!(effects.calls.borrow().is_empty());
        }
    }
    for (index, (user, project, expected)) in [
        (
            json!({"packages":{}}),
            json!(["./local"]),
            Ok("packages is not an array"),
        ),
        (
            json!({}),
            json!(["./local", null]),
            Err(std::io::ErrorKind::NotFound),
        ),
        (
            json!({}),
            json!(["/absolute", null]),
            Ok("package source is not a string"),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if case_in_removed_cwd(&index.to_string(), |_| vec![]).is_none() {
            continue;
        }
        let (_, settings, effects, _tree) = support::uninstalled(&user, &project);
        let manager = Rc::new(DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: "relative".into(),
                agent_dir: "relative-agent".into(),
                settings_manager: settings,
            },
            support::Controlled(effects.clone()),
        ));
        let error = check(&manager, &effects).unwrap_err();
        match expected {
            Ok(message) => assert_eq!(error.to_string(), message),
            Err(kind) => assert_eq!(error.kind(), kind),
        }
        assert!(effects.calls.borrow().is_empty());
    }
}

#[test]
fn check_calls_do_not_hold_settings_borrows_across_effects() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, settings, effects, tree) =
        support::installed(&json!({"packages":["npm:first","npm:second"]}), &json!([]));
    let global = support::npm_root(tree.path(), true);
    // The changed command looks the global root up again.
    let lookup = Ok(support::output(&global, "", Some(0)));
    effects.outputs.borrow_mut().push_back(lookup);
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

#[test]
fn availability_returns_typed_records_in_source_order() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, _, effects, _tree) = support::installed(
        &json!({"packages":["npm:p2","git:github.com/org/p3"]}),
        &json!(["git:github.com/org/p0", "npm:p1"]),
    );
    let (runtime, local) = driver(&effects);
    let (gates, mut starts) = Gates::install(&effects, 4);
    let result = local.spawn_local(manager.check_for_available_updates());
    runtime.block_on(local.run_until(async {
        for index in 0..4 {
            assert_eq!(starts.recv().await, Some(index));
        }
        for index in (0..4).rev() {
            gates.finish(index);
        }
        let result = result.await.unwrap().unwrap();
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
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    for count in [0, 1, 4, 7] {
        check_pool_size(count);
    }
    let (manager, _, effects, _tree) =
        support::installed(&json!({}), &json!(["npm:p0", "npm:p1", "npm:p2", "npm:p3"]));
    let (runtime, local) = driver(&effects);
    let (release, mut started) = gated_npm_checks(&effects);
    let first = local.spawn_local(manager.check_for_available_updates());
    let second = local.spawn_local(manager.check_for_available_updates());
    runtime.block_on(local.run_until(async {
        for _ in 0..8 {
            started.recv().await.unwrap();
        }
        assert_eq!(effects.spawns.get(), 8);
        release.send(true).unwrap();
        assert_eq!(first.await.unwrap().unwrap().len(), 4);
        assert_eq!(second.await.unwrap().unwrap().len(), 4);
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
}

#[test]
fn first_failure_keeps_other_workers_claiming_sources() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, _, effects, _tree) = support::installed(
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
        .push_front(Err(std::io::Error::other("first failure")));
    let (runtime, local) = driver(&effects);
    let (gates, mut starts) = Gates::install(&effects, 6);
    let result = local.spawn_local(manager.check_for_available_updates());
    runtime.block_on(local.run_until(async {
        for i in 0..3 {
            assert_eq!(starts.recv().await, Some(i));
        }
        assert!(
            result
                .await
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("first failure")
        );
        gates.finish(1);
        assert_eq!(starts.recv().await, Some(3));
        gates.finish(3);
        assert_eq!(starts.recv().await, Some(4));
        gates.finish(4);
        assert_eq!(starts.recv().await, Some(5));
        for i in [0, 2, 5] {
            gates.finish(i);
        }
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    assert!(gates.release.borrow().is_empty());
}

#[test]
fn availability_releases_last_owner_after_remaining_work() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, _, effects, _tree) = support::installed(
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
        .push_front(Err(std::io::Error::other("first failure")));
    let weak = Rc::downgrade(&manager);
    let (runtime, local) = driver(&effects);
    let (gates, mut starts) = Gates::install(&effects, 4);
    let result = local.spawn_local(manager.check_for_available_updates());
    drop(manager);
    runtime.block_on(local.run_until(async {
        for i in 0..3 {
            assert_eq!(starts.recv().await, Some(i));
        }
        assert!(result.await.unwrap().is_err());
        assert!(weak.upgrade().is_some());
        gates.finish(0);
        assert_eq!(starts.recv().await, Some(3));
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
/// Native effects that record every captured command's arguments.
struct RecordingNative {
    /// The real adapter every call is delegated to.
    inner: NativePackageOperations,
    /// Arguments of each captured command, in call order.
    captured: Rc<RefCell<Vec<Vec<String>>>>,
}
impl PackageOperations for RecordingNative {
    fn run_command_capture<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        options: CommandCaptureOptions<'a>,
    ) -> PackageFuture<'a, String> {
        let mut call = vec![command.to_owned()];
        call.extend_from_slice(args);
        self.captured.borrow_mut().push(call);
        self.inner.run_command_capture(command, args, options)
    }
    fn spawn(
        &self,
        operation: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'static>>,
    ) -> std::io::Result<()> {
        self.inner.spawn(operation)
    }
    fn run_command_sync(&self, command: &str, args: &[String]) -> std::io::Result<CommandOutput> {
        self.inner.run_command_sync(command, args)
    }
    fn run_command<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        cwd: Option<&'a str>,
    ) -> PackageFuture<'a, Option<i32>> {
        self.inner.run_command(command, args, cwd)
    }
}

/// Wires recording native effects to the test's surviving settings and local runtime.
fn recording_manager(
    cwd: &str,
    settings: &Rc<RefCell<SettingsManager>>,
    local: &Rc<tokio::task::LocalSet>,
    captured: &Rc<RefCell<Vec<Vec<String>>>>,
) -> Rc<DefaultPackageManager<RecordingNative>> {
    Rc::new(DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: cwd.into(),
            agent_dir: format!("{cwd}/agent"),
            settings_manager: settings.clone(),
        },
        RecordingNative {
            inner: NativePackageOperations::new(Rc::new(|| false), local),
            captured: captured.clone(),
        },
    ))
}

/// Asserts the availability check ran Git, and only its two read-only queries.
fn assert_only_read_commands(calls: &[Vec<String>]) {
    assert!(!calls.is_empty());
    assert!(
        calls
            .iter()
            .all(|call| call[0] == "git" && matches!(call[1].as_str(), "rev-parse" | "ls-remote")),
        "{calls:?}"
    );
}

#[test]
fn native_git_availability_leaves_head_and_contents_unchanged() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let scratch = scratch::Scratch::new().unwrap();
    let root = scratch.path();
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
    let runtime = crate::runtime::runtime();
    let captured = Rc::new(RefCell::new(Vec::new()));
    let manager = recording_manager(root.to_str().unwrap(), &settings, &local, &captured);
    std::fs::write(install.join("untracked"), "mine").unwrap();
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
    assert_eq!(
        std::fs::read_to_string(install.join("untracked")).unwrap(),
        "mine"
    );
    assert_only_read_commands(&captured.borrow());
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
    let (manager, _, effects, _tree) = support::installed(&json!({}), &json!(list));
    let (runtime, local) = driver(&effects);
    let (gates, mut starts) = Gates::install(&effects, count);
    let result = local.spawn_local(manager.check_for_available_updates());
    runtime.block_on(local.run_until(async {
        for index in 0..count.min(4) {
            assert_eq!(starts.recv().await, Some(index));
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
                assert_eq!(starts.recv().await, Some(completed + 4));
            }
        }
        let result = result.await.unwrap().unwrap();
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
    std::fs::write(path, b"{\"version\":\"\xff\"}").unwrap();
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
        NativePackageOperations::new(Rc::new(|| false), local),
    ))
}
