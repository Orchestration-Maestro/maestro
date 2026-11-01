//! Startup offline selections reaching the configured package owner.
#![cfg(test)]
#![cfg(unix)]
use super::support;
use maestro_packages::{PackageManager, PackageUpdateType};
use serde_json::json;
use std::rc::Rc;

/// A configured manager holding an installed npm package and Git checkout at version one.
fn installed_selection(selection: Option<bool>) -> support::Configured {
    support::installed_with(
        &json!({"packages": ["npm:missing", "https://github.com/team/missing", "./local"]}),
        &json!(["npm:a", "https://github.com/team/repo"]),
        selection,
    )
}

#[test]
fn startup_selection_skips_missing_remote_sources() {
    super::configured_resolution_support::check(&[
        "selection-_true_",
        "selection-_false_",
        "selection-_absent_",
        "selection-ambient-truthy",
    ]);
}

#[test]
fn startup_selection_keeps_cached_sources_and_pins() {
    super::configured_resolution_support::check(&[
        "selection-pin-exact",
        "selection-pin-mismatch",
        "selection-unpinned",
    ]);
}

#[test]
fn startup_selection_blocks_temporary_git_refresh() {
    crate::runtime::run(async {
        super::cache_support::cases(&["git_selection_offline", "git_selection_online"]).await;
    });
}

#[test]
fn startup_selection_preserves_update_selection_order() {
    let (manager, _, effects, _tree) = installed_selection(Some(true));
    let error = support::drive(&effects, manager.update(Some("npm:absent")))
        .unwrap_err()
        .to_string();
    assert_eq!(error, "No matching package found for npm:absent");
    assert!(effects.calls.borrow().is_empty());
    assert_eq!(effects.spawns.get(), 0);

    let (manager, _, effects, _tree) = installed_selection(Some(true));
    support::drive(&effects, manager.update(Some("npm:a"))).unwrap();
    assert!(effects.calls.borrow().is_empty());
    assert_eq!(effects.spawns.get(), 0);
}

/// A configured manager over installed contents for the npm and Git probes.
fn probes(selection: Option<bool>) -> support::Configured {
    support::installed_with(
        &json!({"packages": ["npm:a"]}),
        &json!(["git:github.com/team/repo"]),
        selection,
    )
}

#[test]
fn startup_selection_reaches_both_update_probes() {
    let (manager, settings, effects, _tree) = probes(Some(true));
    let borrowed = settings.borrow();
    assert!(
        support::drive(&effects, manager.check_for_available_updates())
            .unwrap()
            .is_empty()
    );
    drop(borrowed);
    assert!(effects.calls.borrow().is_empty());
    assert_eq!(effects.spawns.get(), 0);

    assert_both_probes_run();
}

/// Both probes report a changed source under an admitted selection.
fn assert_both_probes_run() {
    let (manager, _, effects, _tree) = probes(Some(false));
    *effects.capture_hook.borrow_mut() = Some(Rc::new(|command, args, _| {
        let text: String = if command == "npm" {
            "\"2\"".into()
        } else if args == ["rev-parse", "HEAD"] {
            "a".repeat(40)
        } else if args == ["rev-parse", "--abbrev-ref", "@{upstream}"] {
            "origin/main".into()
        } else {
            format!("{}\trefs/heads/main", "b".repeat(40))
        };
        Box::pin(async move { Ok(text) })
    }));
    let updates = support::drive(&effects, manager.check_for_available_updates()).unwrap();
    let observed: Vec<_> = updates
        .iter()
        .map(|update| (update.source.as_str(), update.r#type))
        .collect();
    assert_eq!(
        observed,
        [
            ("git:github.com/team/repo", PackageUpdateType::Git),
            ("npm:a", PackageUpdateType::Npm)
        ]
    );
    let calls: Vec<_> = effects
        .calls
        .borrow()
        .iter()
        .map(|(command, args)| (command.clone(), args.clone()))
        .collect();
    assert!(
        calls.contains(&(
            "npm".to_owned(),
            ["view", "a", "version", "--json"]
                .map(str::to_owned)
                .to_vec()
        ))
    );
    assert!(
        calls
            .iter()
            .any(|(command, args)| command == "git" && args == &["rev-parse", "HEAD"])
    );
}
