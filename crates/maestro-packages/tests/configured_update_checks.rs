//! Configured updates through the public manager and replaceable effects.
#![cfg(test)]
#![cfg(unix)]
use crate::native_support;
use crate::resolution_support::{case_in_child, case_in_removed_cwd};
use crate::scratch;
use crate::support;
use crate::worker_support;
use maestro_packages::{
    DefaultPackageManager, NativePackageOperations, PackageManager, PackageManagerOptions,
};
use maestro_settings::{PackageSource, Settings, SettingsListEntry, SettingsManager};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
use worker_support::{Gates, driver, gated_npm_checks};

#[test]
fn updates_selected_unpinned_source_beside_pin() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, settings, effects, tree) =
        installed(&json!([]), &json!(["npm:example@1", "npm:example"]));
    let entries = settings.borrow().get_project_settings().0["packages"].clone();
    let scratch = scratch::Scratch::new().unwrap();
    let cwd = scratch.path().to_string_lossy().into_owned();
    let manifest = format!("{cwd}/.maestro/npm/node_modules/example/package.json");
    native_support::write(&manifest, "{\"version\":\"1\"}");
    let native_settings = Rc::new(RefCell::new(
        SettingsManager::in_memory(Settings::default()),
    ));
    let entries = entries.as_array().unwrap().iter().cloned();
    native_settings
        .borrow_mut()
        .set_project_packages(entries.map(PackageSource::Unknown).collect());
    native_settings.borrow_mut().set_npm_command(Some([
        "/bin/sh", "-c", "if [ \"$1\" = view ]; then printf '\"2\"'; else test \"$1\" = install && test \"$2\" = example@latest && printf '{\"version\":\"2\"}' > \"$0/.maestro/npm/node_modules/example/package.json\"; fi", &cwd
    ].map(|s| SettingsListEntry::String(s.into())).to_vec()));
    let local = Rc::new(tokio::task::LocalSet::new());
    *effects.local.borrow_mut() = Rc::downgrade(&local);
    let native: Rc<dyn PackageManager> = Rc::new(DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: cwd.clone(),
            agent_dir: format!("{cwd}/agent"),
            settings_manager: native_settings.clone(),
        },
        NativePackageOperations::new(Rc::new(|| false), &local),
    ));
    let runtime = crate::runtime::runtime();
    runtime.block_on(local.run_until(async {
        manager.update(Some("npm:example")).await.unwrap();
        native.update(Some("npm:example")).await.unwrap();
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    assert_eq!(
        std::fs::read_to_string(manifest).unwrap(),
        "{\"version\":\"2\"}"
    );
    assert_eq!(
        effects.calls.borrow().last().unwrap().1,
        [
            "install",
            "example@latest",
            "--prefix",
            support::below(tree.path(), "/work/project/.maestro/npm").as_str()
        ]
    );
    for owner in [settings, native_settings] {
        assert_eq!(
            owner.borrow().get_project_settings().0["packages"],
            json!(["npm:example@1", "npm:example"])
        );
    }
}

/// Configures both scopes over an empty tree without validating filter members.
fn configured(user: &Value, project: &Value) -> support::Configured {
    support::uninstalled(&json!({ "packages": user }), project)
}
/// Configures both scopes over a tree where every listed source is installed.
fn installed(user: &Value, project: &Value) -> support::Configured {
    support::installed(&json!({ "packages": user }), project)
}
/// Joins admitted workers before returning the operation's outcome.
fn update(
    manager: Rc<DefaultPackageManager<support::Controlled>>,
    effects: &Rc<support::Effects>,
    source: Option<&str>,
) -> std::io::Result<()> {
    support::drive(effects, manager.update(source))
}

#[test]
fn empty_input_updates_all_configured_scopes_in_order() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    for source in [None, Some("")] {
        let (manager, _, effects, tree) =
            configured(&json!(["npm:b", "npm:a", "npm:b"]), &json!(["npm:c"]));
        update(manager, &effects, source).unwrap();
        let installs: Vec<_> = effects
            .calls
            .borrow()
            .iter()
            .filter(|(_, args)| args[0] == "install")
            .cloned()
            .collect();
        assert_eq!(
            installs,
            [
                (
                    "npm".into(),
                    ["install", "-g", "b@latest", "a@latest", "b@latest"]
                        .map(str::to_owned)
                        .to_vec()
                ),
                (
                    "npm".into(),
                    [
                        "install",
                        "c@latest",
                        "--prefix",
                        support::below(tree.path(), "/work/project/.maestro/npm").as_str()
                    ]
                    .map(str::to_owned)
                    .to_vec()
                )
            ]
        );
    }
    let (manager, _, effects, _tree) = configured(&json!([]), &json!([]));
    update(manager, &effects, None).unwrap();
    assert_eq!(effects.spawns.get(), 0);
}

#[test]
fn named_updates_preserve_both_scopes_and_duplicate_registrations() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, settings, effects, tree) = configured(
        &json!(["npm:a@1", "npm:a", "npm:a"]),
        &json!(["npm:a@next", "npm:a"]),
    );
    update(manager, &effects, Some("npm:a@2")).unwrap();
    assert_eq!(
        effects
            .calls
            .borrow()
            .iter()
            .filter(|(_, args)| args[0] == "install")
            .map(|(_, args)| args.clone())
            .collect::<Vec<_>>(),
        [
            vec!["install", "-g", "a@latest", "a@latest"],
            vec![
                "install",
                "a@latest",
                "--prefix",
                support::below(tree.path(), "/work/project/.maestro/npm").as_str()
            ]
        ]
    );
    assert_eq!(
        settings.borrow().get_global_settings().0["packages"],
        json!(["npm:a@1", "npm:a", "npm:a"])
    );
    assert_eq!(
        settings.borrow().get_project_settings().0["packages"],
        json!(["npm:a@next", "npm:a"])
    );
}

#[test]
fn clone_transport_aliases_select_one_git_identity() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, _, effects, _tree) = installed(
        &json!(["git:github.com/acme/tool@main"]),
        &json!([
            "https://github.com/acme/tool.git",
            "git:github.com/acme/tool",
            "git:github.com/acme/other"
        ]),
    );
    immediate_probes(&effects);
    update(manager, &effects, Some("git:git@github.com:acme/tool.git")).unwrap();
    let fetched: Vec<_> = effects
        .calls
        .borrow()
        .iter()
        .filter(|(program, args)| program == "git" && args[0] == "fetch")
        .cloned()
        .collect();
    assert_eq!(fetched.len(), 2, "{fetched:?}");
}

#[test]
fn cross_scope_suggestions_prefer_the_user_registration() {
    for (user, project, expected) in [
        ("npm:a@1", "npm:a", "npm:a@1"),
        ("npm:a", "npm:a@1", "npm:a"),
    ] {
        let (manager, _, effects, _tree) = configured(&json!([user]), &json!([project]));
        assert_eq!(
            update(manager, &effects, Some("a"))
                .unwrap_err()
                .to_string(),
            format!("No matching package found for a. Did you mean {expected}?")
        );
    }
}

#[test]
fn local_identity_uses_stored_scope_and_input_cwd() {
    for (user, project, source) in [
        (
            json!(["./tool"]),
            json!(["./tool"]),
            "/home/reader/agent/tool",
        ),
        (
            json!(["./tool"]),
            json!(["./tool"]),
            "/work/project/.maestro/tool",
        ),
        (
            json!(["../../../work/project/tool"]),
            json!(["../tool"]),
            "./tool",
        ),
    ] {
        let (manager, _, effects, tree) = configured(&user, &project);
        let source = support::below(tree.path(), source);
        update(manager, &effects, Some(&source)).unwrap();
        assert_eq!(effects.spawns.get(), 0);
    }
}

#[test]
fn no_match_precedes_offline_with_first_shorthand_suggestion() {
    let offline = |_: &str| vec![("MAESTRO_OFFLINE", Some("1".into()))];
    if case_in_child("offline", offline).is_none() {
        return;
    }
    for &(input, rows, expected) in SUGGESTIONS {
        let (manager, _, effects, _tree) = configured(&json!(rows), &json!([]));
        assert_eq!(
            update(manager, &effects, Some(input))
                .unwrap_err()
                .to_string(),
            expected
        );
        assert!(effects.calls.borrow().is_empty());
    }
}

#[test]
fn pins_and_local_sources_skip_missing_content_repair() {
    let (manager, _, effects, tree) = configured(
        &json!([
            "npm:a@1.0.0",
            "npm:a@next",
            "npm:a@^1",
            "npm:@org/a@latest",
            "git:github.com/acme/tool@main",
            "./absent"
        ]),
        &json!([]),
    );
    update(manager, &effects, None).unwrap();
    assert_eq!(std::fs::read_dir(tree.path()).unwrap().count(), 0);
    assert!(effects.calls.borrow().is_empty());
    assert_eq!(effects.spawns.get(), 0);
}

#[test]
fn offline_update_still_selects_sources_before_skipping_work() {
    for (value, offline) in [
        ("1", true),
        ("YES", true),
        ("true", true),
        (" true ", false),
    ] {
        let environment = |_: &str| vec![("MAESTRO_OFFLINE", Some(value.into()))];
        if case_in_child(value, environment).is_none() {
            continue;
        }
        let (manager, _, effects, _tree) = configured(&json!(["npm:a"]), &json!([]));
        update(manager, &effects, Some("npm:a")).unwrap();
        assert_eq!(
            effects
                .calls
                .borrow()
                .iter()
                .any(|(_, args)| args[0] == "install"),
            !offline
        );
    }
}

/// No-match and suggestion expectations from the configured-source cases.
const SUGGESTIONS: &[(&str, &[&str], &str)] = &[
    (
        "a",
        &["npm:a@1", "npm:a"],
        "No matching package found for a. Did you mean npm:a@1?",
    ),
    (
        "a@next",
        &["npm:a@1", "npm:a@next"],
        "No matching package found for a@next. Did you mean npm:a@next?",
    ),
    (
        "@org/a",
        &["npm:@org/a@next"],
        "No matching package found for @org/a. Did you mean npm:@org/a@next?",
    ),
    (
        "github.com/acme/tool",
        &["git:github.com/acme/tool@v1"],
        "No matching package found for github.com/acme/tool. Did you mean git:github.com/acme/tool@v1?",
    ),
    (
        "github.com/acme/tool@v1",
        &["git:github.com/acme/tool@v2", "git:github.com/acme/tool@v1"],
        "No matching package found for github.com/acme/tool@v1. Did you mean git:github.com/acme/tool@v1?",
    ),
    (
        "\u{feff}a\u{feff}",
        &["npm:a"],
        "No matching package found for \u{feff}a\u{feff}. Did you mean npm:a?",
    ),
    (
        "\u{85}a\u{85}",
        &["npm:a"],
        "No matching package found for \u{85}a\u{85}",
    ),
    ("A", &["npm:a"], "No matching package found for A"),
    (
        "folder",
        &["./folder"],
        "No matching package found for folder",
    ),
    (
        "npm:missing",
        &[],
        "No matching package found for npm:missing",
    ),
    (
        "git@github.com:acme/tool.git",
        &["git:github.com/acme/tool"],
        "No matching package found for git@github.com:acme/tool.git",
    ),
];

#[test]
fn unknown_versions_and_failed_lookups_trigger_updates() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    for (manifest, latest) in [
        (None, Ok("\"2\"")),
        (Some("{"), Ok("\"2\"")),
        (Some("{}"), Ok("\"2\"")),
        (Some("{\"version\":null}"), Ok("\"2\"")),
        (Some("{\"version\":\"\"}"), Ok("\"2\"")),
        (Some("{\"version\":\"1\"}"), Err("lookup failed")),
        (Some("{\"version\":\"1\"}"), Ok("\u{feff}")),
        (Some("{\"version\":\"1\"}"), Ok("{")),
        (Some("{\"version\":\"1\"}"), Ok("null")),
    ] {
        let (manager, _, effects, tree) = installed(&json!([]), &json!(["npm:a"]));
        support::project_manifest(&tree, "a", manifest);
        effects
            .capture_outputs
            .borrow_mut()
            .push_back(latest.map(str::to_owned).map_err(std::io::Error::other));
        update(manager, &effects, None).unwrap();
        assert_eq!(
            effects.calls.borrow().last().unwrap().1,
            [
                "install",
                "a@latest",
                "--prefix",
                support::below(tree.path(), "/work/project/.maestro/npm").as_str()
            ]
        );
    }
}

#[test]
fn matching_version_strings_skip_only_current_candidates() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    for (manifest, latest, changed) in [
        ("{\"version\":\"1\"}", "\"1\"", false),
        ("{\"version\":\" \"}", "\" \"", false),
        ("{\"version\":\"1\"}", "\"2\"", true),
        ("{\"version\":\"2\"}", "\"1\"", true),
        ("{\"version\":\"1.0\"}", "\"1.0.0\"", true),
        (
            "{\"version\":\"2\",\"version\":\"1\",\"unused\":{\"x\":4}}",
            "\"2\"",
            true,
        ),
    ] {
        let (manager, _, effects, tree) = installed(&json!([]), &json!(["npm:a"]));
        support::project_manifest(&tree, "a", Some(manifest));
        effects
            .capture_outputs
            .borrow_mut()
            .push_back(Ok(latest.into()));
        update(manager, &effects, None).unwrap();
        assert_eq!(
            effects.calls.borrow()[0].1,
            ["view", "a", "version", "--json"]
        );
        assert_eq!(effects.calls.borrow().len(), if changed { 2 } else { 1 });
    }
}

#[test]
fn scoped_batches_preserve_spec_order_and_grouped_progress() {
    use maestro_packages::{ProgressAction, ProgressEventType};
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, _, effects, tree) = configured(
        &json!(["npm:z", "npm:a", "npm:z", "npm:b"]),
        &json!(["npm:z"]),
    );
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed = events.clone();
    manager.set_progress_callback(Some(Rc::new(move |event| {
        observed.borrow_mut().push(event.clone());
        Ok(())
    })));
    update(manager, &effects, None).unwrap();
    assert_eq!(
        effects
            .calls
            .borrow()
            .iter()
            .filter(|(_, args)| args[0] == "install")
            .map(|(_, args)| args.clone())
            .collect::<Vec<_>>(),
        [
            vec![
                "install", "-g", "z@latest", "a@latest", "z@latest", "b@latest"
            ],
            vec![
                "install",
                "z@latest",
                "--prefix",
                support::below(tree.path(), "/work/project/.maestro/npm").as_str()
            ]
        ]
    );
    for (label, message) in [
        ("user npm packages", "Updating user npm packages..."),
        ("npm:z", "Updating npm:z..."),
    ] {
        let actual: Vec<_> = events
            .borrow()
            .iter()
            .filter(|event| event.source == label)
            .cloned()
            .collect();
        assert_eq!(
            actual
                .iter()
                .map(|event| (event.r#type, event.action, event.message.as_deref()))
                .collect::<Vec<_>>(),
            [
                (
                    ProgressEventType::Start,
                    ProgressAction::Update,
                    Some(message)
                ),
                (ProgressEventType::Complete, ProgressAction::Update, None)
            ]
        );
    }
}

#[test]
fn update_preserves_filtered_settings_and_live_command_selection() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let entry = json!({"source":"npm:a", "extensions":null, "unknown":{"retained":9}});
    let (manager, settings, effects, tree) = configured(&json!([]), &json!([entry]));
    let weak = Rc::downgrade(&settings);
    manager.set_progress_callback(Some(Rc::new(move |_| {
        weak.upgrade().unwrap().borrow_mut().set_npm_command(Some(
            ["runner", "fixed"]
                .map(|s| SettingsListEntry::String(s.into()))
                .to_vec(),
        ));
        Ok(())
    })));
    update(manager, &effects, None).unwrap();
    assert_eq!(
        effects.calls.borrow().last().unwrap(),
        &(
            "runner".into(),
            [
                "fixed",
                "install",
                "a@latest",
                "--prefix",
                support::below(tree.path(), "/work/project/.maestro/npm").as_str()
            ]
            .map(str::to_owned)
            .to_vec()
        )
    );
    assert_eq!(
        settings.borrow().get_project_settings().0["packages"],
        json!([entry])
    );
    captured_rows_keep_live_command_edits();
}

#[test]
fn update_reads_ambient_state_only_when_selection_requires_it() {
    let offline = |_: &str| vec![("MAESTRO_OFFLINE", Some("1".into()))];
    if case_in_removed_cwd("offline", offline).is_none() {
        return;
    }
    for source in ["npm:a", "git:github.com/acme/tool", "/absolute"] {
        let (manager, effects) = relative(&json!([source]), &json!([]));
        update(manager, &effects, Some(source)).unwrap();
    }
    let (manager, effects) = relative(&json!(["./tool"]), &json!([]));
    let error = update(manager, &effects, Some("./tool")).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn source_consumption_precedes_later_settings_errors() {
    if case_in_removed_cwd("removed", |_| vec![]).is_none() {
        return;
    }
    let (manager, effects) = relative(&json!([{"source":null}]), &json!([0]));
    let error = update(manager.clone(), &effects, Some("./input")).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(
        update(manager, &effects, None).unwrap_err().to_string(),
        "package source is not a string"
    );
    let (manager, effects) = relative(&json!(["./tool"]), &json!([0]));
    let error = update(manager, &effects, Some("/unrelated")).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}
/// A configured manager whose working and agent directories are relative.
fn relative(
    user: &Value,
    project: &Value,
) -> (
    Rc<DefaultPackageManager<support::Controlled>>,
    Rc<support::Effects>,
) {
    let (_, settings, effects, _tree) = configured(user, project);
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: "relative".into(),
            agent_dir: "agent".into(),
            settings_manager: settings,
        },
        support::Controlled(effects.clone()),
    );
    (Rc::new(manager), effects)
}

#[test]
fn four_update_workers_preserve_order_across_independent_calls() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    for count in [0, 1, 4, 7] {
        let sources: Vec<_> = (0..count).map(|i| format!("npm:p{i}")).collect();
        let (manager, _, effects, _tree) = installed(&json!([]), &json!(sources));
        let (runtime, local) = driver(&effects);
        let (gates, mut starts) = Gates::install(&effects, count);
        let result = local.spawn_local(manager.update(None));
        runtime.block_on(local.run_until(async {
            for index in 0..count.min(4) {
                assert_eq!(starts.recv().await, Some(index));
            }
            if count > 0 {
                assert_eq!(effects.spawns.get(), count.min(4));
            }
            for index in 4..count {
                gates.finish(index - 4);
                assert_eq!(starts.recv().await, Some(index));
            }
            for index in (count.saturating_sub(4)..count).rev() {
                gates.finish(index);
            }
            result.await.unwrap().unwrap();
            if count == 0 {
                assert_eq!(effects.spawns.get(), 0);
            } else {
                let args = effects.calls.borrow().last().unwrap().1.clone();
                assert_eq!(
                    &args[1..=count],
                    &(0..count)
                        .map(|i| format!("p{i}@latest"))
                        .collect::<Vec<_>>()
                );
            }
        }));
        runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    }
    independent_calls();
}
/// One update and one availability call each admit four independent capture slots.
fn independent_calls() {
    let (manager, _, effects, _tree) =
        installed(&json!([]), &json!(["npm:p0", "npm:p1", "npm:p2", "npm:p3"]));
    let (runtime, local) = driver(&effects);
    let (release, mut started) = gated_npm_checks(&effects);
    let availability = local.spawn_local(manager.check_for_available_updates());
    let update = local.spawn_local(manager.update(None));
    runtime.block_on(local.run_until(async {
        for _ in 0..8 {
            started.recv().await.unwrap();
        }
        assert_eq!(effects.spawns.get(), 8);
        release.send(true).unwrap();
        assert_eq!(availability.await.unwrap().unwrap().len(), 4);
        update.await.unwrap().unwrap();
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
}

#[test]
fn update_progress_uses_live_callback_and_preserves_error_precedence() {
    use maestro_packages::ProgressEventType as Phase;
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    for phase in [Phase::Start, Phase::Complete, Phase::Error] {
        let (manager, _, effects, _tree) = configured(&json!([]), &json!(["npm:a"]));
        let events = Rc::new(RefCell::new(Vec::new()));
        let observed = events.clone();
        if phase == Phase::Error {
            *effects.run_hook.borrow_mut() = Some(failed_command());
        }
        manager.set_progress_callback(Some(Rc::new(move |event| {
            observed.borrow_mut().push(event.r#type);
            if event.r#type == phase {
                return Err(std::io::Error::other("callback failed"));
            }
            Ok(())
        })));
        assert_eq!(
            update(manager, &effects, None).unwrap_err().to_string(),
            "callback failed"
        );
        assert_eq!(
            *events.borrow(),
            if phase == Phase::Start {
                vec![Phase::Start]
            } else if phase == Phase::Complete {
                vec![Phase::Start, Phase::Complete, Phase::Error]
            } else {
                vec![Phase::Start, Phase::Error]
            }
        );
    }
    callback_replacement(true);
    callback_replacement(false);
}
/// An inherited command whose failure exercises error callback precedence.
fn failed_command() -> support::RunHook {
    Rc::new(|_, _, _| Box::pin(async { Err(std::io::Error::other("command failed")) }))
}
/// Replaces or clears the slot during a start callback.
fn callback_replacement(clear: bool) {
    use maestro_packages::ProgressEventType as Phase;
    let (manager, _, effects, _tree) = configured(&json!([]), &json!(["npm:a"]));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let weak = Rc::downgrade(&manager);
    let events = observed.clone();
    manager.set_progress_callback(Some(Rc::new(move |event| {
        events.borrow_mut().push(("old", event.r#type));
        let callback = if clear {
            None
        } else {
            Some(replacement_callback(events.clone()))
        };
        weak.upgrade().unwrap().set_progress_callback(callback);
        Ok(())
    })));
    update(manager.clone(), &effects, None).unwrap();
    assert_eq!(
        *observed.borrow(),
        if clear {
            vec![("old", Phase::Start)]
        } else {
            vec![("old", Phase::Start), ("new", Phase::Complete)]
        }
    );
    manager.set_progress_callback(None);
}
/// Records events delivered to the replacement slot.
fn replacement_callback(
    events: Rc<RefCell<Vec<(&'static str, maestro_packages::ProgressEventType)>>>,
) -> maestro_packages::ProgressCallback {
    Rc::new(move |event| {
        events.borrow_mut().push(("new", event.r#type));
        Ok(())
    })
}

#[test]
fn project_batch_preparation_keeps_existing_files() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    for existing in [false, true] {
        let scratch = scratch::Scratch::new().unwrap();
        let cwd = scratch.path().to_string_lossy().into_owned();
        let root = format!("{cwd}/.maestro/npm");
        if existing {
            native_support::write(format!("{root}/package.json"), "existing manifest");
            native_support::write(format!("{root}/.gitignore"), "existing ignore");
        }
        let settings = Rc::new(RefCell::new(
            SettingsManager::in_memory(Settings::default()),
        ));
        settings
            .borrow_mut()
            .set_project_packages(vec![PackageSource::Source("npm:a".into())]);
        settings
            .borrow_mut()
            .set_npm_command(Some(vec![SettingsListEntry::String(
                "/usr/bin/true".into(),
            )]));
        let local = Rc::new(tokio::task::LocalSet::new());
        let manager = Rc::new(DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: cwd.clone(),
                agent_dir: format!("{cwd}/agent"),
                settings_manager: settings,
            },
            NativePackageOperations::new(Rc::new(|| false), &local),
        ));
        let runtime = crate::runtime::runtime();
        runtime
            .block_on(local.run_until(manager.update(None)))
            .unwrap();
        runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
        assert_eq!(
            std::fs::read_to_string(format!("{root}/package.json")).unwrap(),
            if existing {
                "existing manifest"
            } else {
                "{\n  \"name\": \"maestro-extensions\",\n  \"private\": true\n}"
            }
        );
        assert_eq!(
            std::fs::read_to_string(format!("{root}/.gitignore")).unwrap(),
            if existing {
                "existing ignore"
            } else {
                "*\n!.gitignore\n"
            }
        );
    }
    project_preparation_failure();
}

#[test]
fn runtime_admission_errors_keep_accepted_work() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, _, effects, _tree) = installed(&json!([]), &json!(["npm:a", "npm:b"]));
    effects.fail_spawn_at.set(Some(1));
    let (runtime, local) = driver(&effects);
    let (send, mut finished) = tokio::sync::mpsc::unbounded_channel();
    *effects.capture_hook.borrow_mut() = Some(Rc::new(move |_, args, _| {
        let label = args[1].clone();
        let send = send.clone();
        Box::pin(async move {
            send.send(label).unwrap();
            Ok("\"2\"".into())
        })
    }));
    runtime.block_on(local.run_until(async {
        assert_eq!(
            manager.clone().update(None).await.unwrap_err().to_string(),
            "admission failed"
        );
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    let mut finished_labels = Vec::new();
    while let Ok(label) = finished.try_recv() {
        finished_labels.push(label);
    }
    assert_eq!(finished_labels, ["a", "b"]);
    assert!(
        effects
            .calls
            .borrow()
            .iter()
            .all(|(_, args)| args[0] != "install")
    );
    let (manager, _, effects, _tree) = configured(&json!([]), &json!(["npm:a"]));
    assert_eq!(
        crate::runtime::run(manager.update(None))
            .unwrap_err()
            .to_string(),
        "stopped runtime"
    );
    assert_eq!(effects.spawns.get(), 0);
}

#[test]
fn npm_checks_finish_before_parallel_npm_and_git_groups() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, _, effects, _tree) = installed(
        &json!(["npm:p0", "npm:p1"]),
        &json!([
            "npm:p2",
            "npm:p3",
            "git:github.com/acme/g0",
            "git:github.com/acme/g1",
            "git:github.com/acme/g2",
            "git:github.com/acme/g3",
            "git:github.com/acme/g4"
        ]),
    );
    let (runtime, local) = driver(&effects);
    let (release_checks, mut checking) = gated_npm_checks(&effects);
    let (running, mut started) = tokio::sync::mpsc::unbounded_channel();
    let (release_work, wait_work) = tokio::sync::watch::channel(false);
    *effects.run_hook.borrow_mut() = Some(Rc::new(move |command, args, cwd| {
        let running = running.clone();
        let mut wait = wait_work.clone();
        let label = if command == "git" {
            assert_eq!(args[0], "fetch");
            cwd.unwrap().rsplit('/').next().unwrap().to_owned()
        } else if args.iter().any(|arg| arg == "-g") {
            "user".into()
        } else {
            "project".into()
        };
        Box::pin(async move {
            running.send(label).unwrap();
            wait.wait_for(|ready| *ready).await.unwrap();
            Ok(Some(0))
        })
    }));
    let operation = local.spawn_local(manager.update(None));
    runtime.block_on(local.run_until(async {
        for _ in 0..4 {
            checking.recv().await.unwrap();
        }
        // All check workers are blocked at their producer gates, so execution cannot advance.
        assert!(matches!(
            started.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));
        release_checks.send(true).unwrap();
        let mut admitted = Vec::new();
        for _ in 0..6 {
            admitted.push(started.recv().await.unwrap());
        }
        admitted.sort();
        assert_eq!(admitted, ["g0", "g1", "g2", "g3", "project", "user"]);
        release_work.send(true).unwrap();
        assert_eq!(started.recv().await.as_deref(), Some("g4"));
        operation.await.unwrap().unwrap();
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
}

/// Supplies successful version and unchanged Git-head probes.
fn immediate_probes(effects: &support::Effects) {
    *effects.capture_hook.borrow_mut() = Some(Rc::new(|command, _, _| {
        let text = if command == "git" {
            "origin/main"
        } else {
            "\"2\""
        };
        Box::pin(async move { Ok(text.into()) })
    }));
}

/// Runs a failing execution group with sealed sibling gates, then joins the retained work.
fn failed_execution(failure: &'static str) -> (Vec<String>, bool, bool) {
    let (manager, _, effects, _tree) = installed(
        &json!(["npm:user"]),
        &json!([
            "npm:project",
            "git:github.com/acme/g0",
            "git:github.com/acme/g1",
            "git:github.com/acme/g2",
            "git:github.com/acme/g3",
            "git:github.com/acme/g4"
        ]),
    );
    let weak = Rc::downgrade(&manager);
    let weak_effects = Rc::downgrade(&effects);
    let (runtime, local) = driver(&effects);
    immediate_probes(&effects);
    let (started, mut starts) = tokio::sync::mpsc::unbounded_channel();
    let (completed, mut completions) = tokio::sync::mpsc::unbounded_channel();
    let (release, wait) = tokio::sync::watch::channel(false);
    *effects.run_hook.borrow_mut() = Some(failing_group_hook(failure, started, completed, wait));
    let operation = local.spawn_local(manager.update(None));
    let (finished, retained) = runtime.block_on(local.run_until(async {
        assert_eq!(
            operation.await.unwrap().unwrap_err().to_string(),
            "first execution failure"
        );
        let retained = weak.upgrade().is_some();
        for _ in 0..5 {
            starts.recv().await.unwrap();
        }
        release.send(true).unwrap();
        let mut finished = Vec::new();
        for _ in 0..6 {
            finished.push(completions.recv().await.unwrap());
        }
        (finished, retained)
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    drop(effects);
    (
        finished,
        retained,
        weak.upgrade().is_none() && weak_effects.upgrade().is_none(),
    )
}

#[test]
fn first_update_error_keeps_siblings_claiming_work() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, _, effects, _tree) = installed(
        &json!([
            "npm:p0", "npm:p1", "npm:p2", "npm:p3", "npm:p4", "npm:p5", "npm:p6"
        ]),
        &json!([]),
    );
    effects
        .outputs
        .borrow_mut()
        .push_front(Err(std::io::Error::other("first root failure")));
    let (runtime, local) = driver(&effects);
    let (gates, mut starts) = Gates::install(&effects, 7);
    let operation = local.spawn_local(manager.update(None));
    runtime.block_on(local.run_until(async {
        assert_eq!(
            operation.await.unwrap().unwrap_err().to_string(),
            "Failed to run npm root -g: first root failure"
        );
        for index in 1..7 {
            gates.finish(index);
        }
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    let mut claimed = Vec::new();
    while let Ok(index) = starts.try_recv() {
        claimed.push(index);
    }
    assert_eq!(claimed, [1, 2, 3, 4, 5, 6]);
    assert!(
        effects
            .calls
            .borrow()
            .iter()
            .all(|(_, args)| args[0] != "install")
    );
    let (mut finished, _, _) = failed_execution("user");
    finished.sort();
    assert_eq!(finished, ["g0", "g1", "g2", "g3", "g4", "project"]);
}

#[test]
fn updates_release_manager_after_surviving_work_finishes() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (_, retained, released) = failed_execution("g0");
    assert!(
        retained,
        "accepted work must retain its manager after error delivery"
    );
    assert!(
        released,
        "joined work must release its final manager and operations aliases"
    );
}

/// Preparation errors emit grouped failure without executing the npm child.
fn project_preparation_failure() {
    let (manager, _, effects, tree) = configured(&json!([]), &json!(["npm:z", "npm:a"]));
    let root = support::below(tree.path(), "/work/project/.maestro/npm");
    std::fs::create_dir_all(&root).unwrap();
    let Some(restore) = maestro_test_directory::deny(&root, 0o555).unwrap() else {
        return;
    };
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed = events.clone();
    manager.set_progress_callback(Some(Rc::new(move |event| {
        observed.borrow_mut().push(event.clone());
        Ok(())
    })));
    let error = update(manager, &effects, None).unwrap_err();
    drop(restore);
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(effects.calls.borrow().is_empty());
    let events = events.borrow();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].source, "project npm packages");
    assert_eq!(events[1].r#type, maestro_packages::ProgressEventType::Error);
    assert_eq!(events[1].message, Some(error.to_string()));
}
/// Caller edits replace settings entries but cannot replace already selected rows.
fn captured_rows_keep_live_command_edits() {
    let (manager, settings, effects, tree) = configured(&json!([]), &json!(["npm:a"]));
    let weak = Rc::downgrade(&settings);
    manager.set_progress_callback(Some(Rc::new(move |event| {
        if event.r#type == maestro_packages::ProgressEventType::Start {
            let owner = weak.upgrade().unwrap();
            owner
                .borrow_mut()
                .set_project_packages(vec![PackageSource::Source("npm:replacement".into())]);
            owner
                .borrow_mut()
                .set_npm_command(Some(vec![SettingsListEntry::String("live-runner".into())]));
        }
        Ok(())
    })));
    update(manager, &effects, None).unwrap();
    assert_eq!(
        effects.calls.borrow().last().unwrap(),
        &(
            "live-runner".into(),
            [
                "install",
                "a@latest",
                "--prefix",
                support::below(tree.path(), "/work/project/.maestro/npm").as_str()
            ]
            .map(str::to_owned)
            .to_vec()
        )
    );
    assert_eq!(
        settings.borrow().get_project_settings().0["packages"],
        json!(["npm:replacement"])
    );
}

/// Gates surviving groups while failing exactly the selected execution job.
fn failing_group_hook(
    failure: &'static str,
    started: tokio::sync::mpsc::UnboundedSender<String>,
    completed: tokio::sync::mpsc::UnboundedSender<String>,
    wait: tokio::sync::watch::Receiver<bool>,
) -> support::RunHook {
    Rc::new(move |command, args, cwd| {
        let label = if command == "git" {
            cwd.unwrap().rsplit('/').next().unwrap().to_owned()
        } else if args.iter().any(|arg| arg == "-g") {
            "user".into()
        } else {
            "project".into()
        };
        let started = started.clone();
        let completed = completed.clone();
        let mut wait = wait.clone();
        Box::pin(async move {
            if label == failure {
                return Err(std::io::Error::other("first execution failure"));
            }
            started.send(label.clone()).unwrap();
            wait.wait_for(|ready| *ready).await.unwrap();
            completed.send(label).unwrap();
            Ok(Some(0))
        })
    })
}

#[test]
fn mixed_pinned_current_and_outdated_candidates_install_only_outdated() {
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let (manager, _, effects, tree) = installed(
        &json!([]),
        &json!(["npm:pinned@1", "npm:current", "npm:outdated"]),
    );
    effects
        .capture_outputs
        .borrow_mut()
        .extend([Ok("\"1\"".into()), Ok("\"2\"".into())]);
    update(manager, &effects, None).unwrap();
    assert_eq!(
        *effects.calls.borrow(),
        [
            (
                "npm".into(),
                vec![
                    "view".into(),
                    "current".into(),
                    "version".into(),
                    "--json".into()
                ]
            ),
            (
                "npm".into(),
                vec![
                    "view".into(),
                    "outdated".into(),
                    "version".into(),
                    "--json".into()
                ]
            ),
            (
                "npm".into(),
                vec![
                    "install".into(),
                    "outdated@latest".into(),
                    "--prefix".into(),
                    support::below(tree.path(), "/work/project/.maestro/npm")
                ]
            ),
        ]
    );
}
