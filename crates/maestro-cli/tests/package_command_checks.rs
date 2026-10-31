//! Package command grammar, presentation and application calls.
#![cfg(test)]
#![cfg(unix)]

use crate::support::package_commands::{CONFIG_DIR, Failing, Failure, Harness, Trace};
use maestro_app::configuration::config::VERSION;
use maestro_app::{SettingsListEntry, SettingsManager};
use serde_json::json;
use std::sync::Arc;

/// The stored entries of a package-manager command.
fn entries(argv: &[&str]) -> Vec<SettingsListEntry> {
    argv.iter()
        .map(|entry| SettingsListEntry::String((*entry).to_owned()))
        .collect()
}

/// The usage line of `command`.
fn usage(command: &str) -> String {
    match command {
        "install" | "remove" => format!("maestro {command} <source> [-l]"),
        "update" => "maestro update [source|self|maestro] [--self] [--extensions] \
                     [--extension <source>] [--force]"
            .into(),
        _ => "maestro list".into(),
    }
}

/// The two diagnostic lines of a primary line and its secondary line.
fn diagnostic(primary: &str, secondary: &str) -> String {
    format!("{primary}\n{secondary}\n")
}

#[test]
fn package_verbs_are_only_recognized_first() {
    let harness = Harness::new(None, None);
    for args in [&[][..], &["start"][..], &["npm:x", "install"][..]] {
        assert!(!harness.run(args), "{args:?}");
    }
    assert!(harness.state.loaded.borrow().is_empty());
    assert!(harness.calls().is_empty());
    assert_eq!(harness.stdout.text(), "");
    assert_eq!(harness.stderr.text(), "");
}

#[test]
fn package_help_is_effect_free() {
    let harness = Harness::new(Some(json!({})), Some(json!({})));
    harness.fail_loading("help never loads settings");
    harness.fail_cwd("help never observes the directory");
    help_document(&harness, &["install", "--help"], install_document);
    help_document(&harness, &["remove", "-h"], remove_document);
    help_document(&harness, &["update", "--help"], update_document);
    help_document(&harness, &["list", "--help"], list_document);
    help_document(&harness, &["update", "--bogus", "--help"], update_help);
    assert_eq!(harness.stderr.text(), "");
    assert_eq!(harness.exit_code.get(), None);
    assert!(harness.state.loaded.borrow().is_empty());
    assert!(harness.calls().is_empty());
}

/// Run one help request and check the only document it printed.
fn help_document(harness: &Harness, args: &[&str], check: impl Fn(&str)) {
    harness.stdout.clear();
    assert!(harness.run(args), "{args:?}");
    check(&harness.stdout.text());
}

/// Check the complete install document.
fn install_document(text: &str) {
    assert!(text.starts_with("Usage:\n  maestro install <source> [-l]\n\nInstall a package"));
    assert!(text.contains("  -l, --local    Install project-locally (.maestro/settings.json)"));
    assert!(text.contains("  maestro install ssh://git@github.com/user/repo\n"));
    assert!(text.ends_with("  maestro install ./local/path\n\n"));
}

/// Check the complete remove document.
fn remove_document(text: &str) {
    assert!(text.contains(
        "Remove a package and its source from settings.\nAlias: maestro uninstall <source> [-l]\n"
    ));
    assert!(text.contains("  maestro uninstall npm:@foo/bar\n"));
}

/// Check the complete update document.
fn update_document(text: &str) {
    assert!(text.contains("Update maestro and installed packages."));
    assert!(text.contains(
        "  --force                 Reinstall maestro even if the current version is latest"
    ));
    assert!(text.contains(
        "  maestro update maestro        Update maestro only (self works as alias to maestro)\n"
    ));
}

/// Check the complete list document.
fn list_document(text: &str) {
    assert!(text.ends_with(
        "Usage:\n  maestro list\n\nList installed packages from user and project settings.\n\n"
    ));
}

/// Check that help wins over an unknown option.
fn update_help(text: &str) {
    assert!(text.starts_with("Usage:\n  maestro update"));
}

#[test]
fn package_options_keep_first_diagnostic() {
    let harness = Harness::new(None, None);
    assert!(harness.run(&["update", "--first", "--second"]));
    assert_eq!(
        harness.stderr.text(),
        diagnostic(
            "Unknown option --first for \"update\".",
            &format!("Use \"maestro --help\" or \"{}\".", usage("update"))
        )
    );
    harness.stderr.clear();
    assert!(harness.run(&["update", "--extension", "--extension"]));
    assert_eq!(
        harness.stderr.text(),
        diagnostic(
            "Missing value for --extension.",
            &format!("Usage: {}", usage("update"))
        )
    );
    harness.stderr.clear();
    assert!(harness.run(&["install", "npm:a", ""]));
    assert_eq!(
        harness.stderr.text(),
        diagnostic(
            "Unexpected argument .",
            &format!("Usage: {}", usage("install"))
        )
    );
    assert_eq!(harness.exit_code.get(), Some(1));
}

#[test]
fn package_diagnostics_keep_category_precedence() {
    let harness = Harness::new(None, None);
    for (args, expected) in [
        (
            &["update", "--bogus", "--extension"][..],
            diagnostic(
                "Unknown option --bogus for \"update\".",
                &format!("Use \"maestro --help\" or \"{}\".", usage("update")),
            ),
        ),
        (
            &["update", "x", "y", "--extension"][..],
            diagnostic(
                "Missing value for --extension.",
                &format!("Usage: {}", usage("update")),
            ),
        ),
        (
            &["update", "x", "y", "--self"][..],
            diagnostic(
                "Unexpected argument y.",
                &format!("Usage: {}", usage("update")),
            ),
        ),
        (
            &["update", "x", "--self"][..],
            diagnostic(
                "positional update targets cannot be combined with --self or --extensions",
                &format!("Usage: {}", usage("update")),
            ),
        ),
        (
            &["install"][..],
            diagnostic(
                "Missing install source.",
                &format!("Usage: {}", usage("install")),
            ),
        ),
    ] {
        harness.stderr.clear();
        harness.exit_code.set(None);
        assert!(harness.run(args));
        assert_eq!(harness.stderr.text(), expected, "{args:?}");
        assert_eq!(harness.exit_code.get(), Some(1), "{args:?}");
        assert_eq!(harness.stdout.text(), "", "{args:?}");
        assert!(harness.state.loaded.borrow().is_empty(), "{args:?}");
    }
}

#[test]
fn package_extension_values_keep_missing_and_dash_boundaries() {
    let harness = Harness::new(None, None);
    for args in [
        &["update", "--extension"][..],
        &["update", "--extension", ""][..],
        &["update", "--extension", "--self"][..],
    ] {
        harness.stderr.clear();
        harness.exit_code.set(None);
        assert!(harness.run(args));
        assert_eq!(
            harness.stderr.text(),
            diagnostic(
                "Missing value for --extension.",
                &format!("Usage: {}", usage("update"))
            ),
            "{args:?}"
        );
    }
    harness.stderr.clear();
    assert!(harness.run(&["update", "--extension", "-l"]));
    assert_eq!(
        harness.stderr.text(),
        diagnostic(
            "Unknown option -l for \"update\".",
            &format!("Use \"maestro --help\" or \"{}\".", usage("update"))
        ),
        "the dash operand stays an option"
    );
    harness.stderr.clear();
    harness.stdout.clear();
    assert!(harness.run(&["update", "--extension", " 123 "]));
    assert_eq!(
        harness.stderr.text(),
        "Error: No matching package found for  123 \n",
        "the authored spelling reaches the manager"
    );
    assert!(harness.runs().is_empty(), "no own update after the failure");
}

#[test]
fn package_extension_repetition_keeps_first_conflict() {
    let harness = Harness::new(None, None);
    assert!(harness.run(&["update", "--extension", "a", "--extension", "b"]));
    assert_eq!(
        harness.stderr.text(),
        diagnostic(
            "--extension can only be provided once",
            &format!("Usage: {}", usage("update"))
        )
    );
    assert_eq!(harness.exit_code.get(), Some(1));
    assert!(harness.runs().is_empty());
}

#[test]
fn package_update_targets_keep_each_independent_selection() {
    for (args, extensions, own) in [
        (&["update"][..], Some("Updated packages\n"), true),
        (&["update", "--self"][..], None, true),
        (
            &["update", "--extensions"][..],
            Some("Updated packages\n"),
            false,
        ),
        (&["update", "self"][..], None, true),
        (&["update", "maestro"][..], None, true),
        (&["update", "npm:x"][..], Some("Updated npm:x\n"), false),
        (
            &["update", "self", "--extensions"][..],
            Some("Updated packages\n"),
            true,
        ),
        (
            &["update", "--self", "--extensions"][..],
            Some("Updated packages\n"),
            true,
        ),
        (
            &["update", "--extension", "npm:x"][..],
            Some("Updated npm:x\n"),
            false,
        ),
    ] {
        let harness = Harness::new(Some(json!({ "packages": ["npm:x"] })), None);
        assert!(harness.run(args), "{args:?}");
        let stdout = harness.stdout.text();
        for line in ["Updated packages\n", "Updated npm:x\n"] {
            assert_eq!(
                stdout.contains(line),
                extensions == Some(line),
                "{args:?} in {stdout}"
            );
        }
        assert_eq!(!harness.runs().is_empty(), own, "{args:?} in {stdout}");
    }
}

#[test]
fn package_update_flags_reject_positional_conflicts() {
    let harness = Harness::new(None, None);
    for args in [
        &["update", "npm:x", "--self"][..],
        &["update", "--self", "npm:x"][..],
        &["update", "npm:x", "--extensions"][..],
        &["update", "--extensions", "npm:x"][..],
    ] {
        harness.stderr.clear();
        assert!(harness.run(args));
        assert_eq!(
            harness.stderr.text(),
            diagnostic(
                "positional update targets cannot be combined with --self or --extensions",
                &format!("Usage: {}", usage("update"))
            ),
            "{args:?}"
        );
    }
    for args in [
        &["update", "--extension", "a", "npm:x"][..],
        &["update", "npm:x", "--extension", "a"][..],
    ] {
        harness.stderr.clear();
        assert!(harness.run(args));
        assert_eq!(
            harness.stderr.text(),
            diagnostic(
                "--extension cannot be combined with a positional source",
                &format!("Usage: {}", usage("update"))
            ),
            "{args:?}"
        );
    }
    harness.stderr.clear();
    assert!(harness.run(&["update", "--extension", "a", "--self"]));
    assert_eq!(
        harness.stderr.text(),
        diagnostic(
            "--extension cannot be combined with --self or --extensions",
            &format!("Usage: {}", usage("update"))
        )
    );
}

#[test]
fn package_sources_are_required() {
    let harness = Harness::new(Some(json!({})), None);
    for command in ["install", "remove"] {
        for args in [&[command][..], &[command, ""][..]] {
            harness.stdout.clear();
            harness.stderr.clear();
            harness.exit_code.set(None);
            assert!(harness.run(args));
            assert_eq!(
                harness.stderr.text(),
                diagnostic(
                    &format!("Missing {command} source."),
                    &format!("Usage: {}", usage(command))
                )
            );
            assert_eq!(harness.exit_code.get(), Some(1));
        }
    }
    assert!(harness.state.loaded.borrow().is_empty());
    assert!(harness.calls().is_empty());
}

#[test]
fn package_empty_source_is_replaceable() {
    let harness = Harness::new(Some(json!({})), None);
    let source = scratch_source(&harness, "replaceable");
    assert!(harness.run(&["install", "", &source]));
    assert!(
        harness
            .stdout
            .text()
            .contains(&format!("Installed {source}\n")),
        "{}",
        harness.stdout.text()
    );
    assert!(
        harness.stderr.text().is_empty(),
        "{}",
        harness.stderr.text()
    );
}

/// A source directory inside the harness scratch root.
fn scratch_source(harness: &Harness, name: &str) -> String {
    let path = harness.scratch.path().join(name);
    std::fs::create_dir_all(&path).expect("source directory");
    path.to_string_lossy().into_owned()
}

#[test]
fn package_empty_extra_is_diagnosed_before_setup() {
    let harness = Harness::new(Some(json!({})), None);
    for args in [
        &["install", "npm:x", ""][..],
        &["install", "npm:x", "", "later"][..],
    ] {
        harness.stdout.clear();
        harness.stderr.clear();
        harness.exit_code.set(None);
        assert!(harness.run(args));
        assert_eq!(
            harness.stderr.text(),
            diagnostic(
                "Unexpected argument .",
                &format!("Usage: {}", usage("install"))
            ),
            "{args:?}"
        );
        assert_eq!(harness.exit_code.get(), Some(1));
        assert!(harness.stdout.text().is_empty());
        assert!(harness.state.loaded.borrow().is_empty());
        assert!(harness.calls().is_empty());
    }
}

#[test]
fn package_source_spelling_is_not_trimmed() {
    let harness = Harness::new(None, None);
    for (args, expected) in [
        (
            &["install", "x", "  y  "][..],
            diagnostic(
                "Unexpected argument   y  .",
                &format!("Usage: {}", usage("install")),
            ),
        ),
        (
            &["install", "x", "\u{feff}y"][..],
            diagnostic(
                "Unexpected argument \u{feff}y.",
                &format!("Usage: {}", usage("install")),
            ),
        ),
        (
            &["install", "x", "a\u{85}b"][..],
            diagnostic(
                "Unexpected argument a\u{85}b.",
                &format!("Usage: {}", usage("install")),
            ),
        ),
        (
            &["install", "--"][..],
            diagnostic(
                "Unknown option -- for \"install\".",
                &format!("Use \"maestro --help\" or \"{}\".", usage("install")),
            ),
        ),
        (
            &["install", "--local=true"][..],
            diagnostic(
                "Unknown option --local=true for \"install\".",
                &format!("Use \"maestro --help\" or \"{}\".", usage("install")),
            ),
        ),
        (
            &["install", "--LOCAL"][..],
            diagnostic(
                "Unknown option --LOCAL for \"install\".",
                &format!("Use \"maestro --help\" or \"{}\".", usage("install")),
            ),
        ),
    ] {
        harness.stderr.clear();
        assert!(harness.run(args));
        assert_eq!(harness.stderr.text(), expected, "{args:?}");
    }
}

#[test]
fn package_ambient_failures_keep_their_catch_boundary() {
    let harness = Harness::new(None, None);
    harness.fail_cwd("directory observation refused");
    assert!(harness.run(&["install"]));
    assert_eq!(harness.exit_code.get(), Some(1));
    assert!(
        harness.stderr.text().starts_with("Missing install source."),
        "{}",
        harness.stderr.text()
    );
    let error = harness
        .try_run(&["install", "npm:x"])
        .expect_err("the directory failure is required");
    assert_eq!(error.to_string(), "directory observation refused");
    let harness = Harness::new(Some(json!({})), None);
    harness.fail_loading("settings load refused");
    assert_eq!(
        harness
            .try_run(&["list"])
            .expect_err("the settings failure is required")
            .to_string(),
        "settings load refused"
    );
}

#[test]
fn package_warning_drain_keeps_scope_message_and_supplied_stack() {
    let harness = Harness::new(None, None);
    harness.use_storage(Arc::new(Failing {
        global: Failure {
            message: "",
            trace: Some(Trace("inner\ntrace")),
        },
        project: Failure {
            message: "project settings broke",
            trace: None,
        },
    }));
    assert!(harness.run(&["list"]));
    assert_eq!(
        harness.stderr.text(),
        "Warning (package command, global settings): \ninner\ntrace\n\
         Warning (package command, project settings): project settings broke\n"
    );
    assert!(
        harness.supplied().borrow_mut().drain_errors().is_empty(),
        "the same owner drains once"
    );
}

#[test]
fn package_progress_prints_only_start_events() {
    let harness = Harness::new(Some(json!({})), Some(json!({})));
    let source = scratch_source(&harness, "progress");
    assert!(harness.run(&["install", &source]));
    let stdout = harness.stdout.text();
    assert!(
        stdout.ends_with(&format!("Installed {source}\n")),
        "{stdout}"
    );
    assert!(!stdout.contains("complete"), "{stdout}");
    assert!(!stdout.contains("failed"), "{stdout}");
}

#[test]
fn package_commands_reuse_supplied_owners() {
    let harness = Harness::new(None, None);
    harness.use_storage(Arc::new(Failing {
        global: Failure {
            message: "global broke",
            trace: None,
        },
        project: Failure {
            message: "project broke",
            trace: None,
        },
    }));
    let supplied = std::rc::Rc::clone(&harness.state);
    harness.stderr.set_hook(std::rc::Rc::new(move || {
        if let Some(manager) = supplied.supplied.borrow().last() {
            manager
                .borrow_mut()
                .set_npm_command(Some(entries(&["npm", "--from-warning"])));
        }
    }));
    let set = harness.state.edit_in_factory.clone();
    *set.borrow_mut() = Some(Arc::new(|manager: &mut maestro_app::SettingsManager| {
        manager.set_npm_command(Some(entries(&["npm", "--from-factory"])));
    }));
    assert!(harness.run(&["update"]));
    let loaded = harness.state.loaded.borrow();
    let factory = harness.state.factory.borrow();
    assert_eq!(loaded.len(), 1);
    assert_eq!(
        factory.first().copied(),
        Some(std::rc::Rc::as_ptr(&loaded[0])),
        "the factory received the loaded owner"
    );
    assert!(
        harness
            .runs()
            .iter()
            .all(|run| run.contains("--from-warning")),
        "{:?}",
        harness.runs()
    );
}

#[test]
fn relative_local_install_keeps_settings_base() {
    let harness = Harness::new(Some(json!({})), None);
    std::fs::create_dir_all(harness.scratch.path().join("root/pkg")).expect("local source");
    assert!(harness.run(&["install", "./pkg"]));
    assert!(
        harness.stdout.text().contains("Installed ./pkg\n"),
        "out={:?} err={:?}",
        harness.stdout.text(),
        harness.stderr.text()
    );
    let stored = harness.supplied().borrow().get_global_settings();
    assert_eq!(
        stored.0["packages"],
        json!(["../root/pkg"]),
        "the source keeps its settings-relative identity"
    );
}

/// The readable text of one settings file.
fn settings_text(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn trailing_slash_removal_keeps_local_identity() {
    let harness = Harness::new(None, None);
    let package = scratch_source(&harness, "identity");
    let slashed = format!("{package}/");
    harness.write_settings(
        Some(json!({ "packages": [slashed] })),
        Some(json!({ "packages": [package] })),
    );
    assert!(harness.run(&["remove", &package]));
    assert!(
        harness
            .stdout
            .text()
            .contains(&format!("Removed {package}\n")),
        "{}",
        harness.stdout.text()
    );
    let user = harness.supplied().borrow().get_global_settings();
    assert!(
        user.0["packages"].as_array().is_none_or(Vec::is_empty),
        "the user registration is gone: {user:?}"
    );
    let project = settings_text(&format!("{}/{CONFIG_DIR}/settings.json", harness.cwd));
    assert!(
        project.contains(&package),
        "the project registration remains"
    );
}

#[test]
fn package_removal_absence_sets_status_without_exit() {
    let harness = Harness::new(None, None);
    assert!(harness.run(&["remove", "npm:@absent/pkg"]));
    assert_eq!(
        harness.stderr.text(),
        "No matching package found for npm:@absent/pkg\n"
    );
    assert_eq!(harness.exit_code.get(), Some(1));
    assert!(
        !harness.stdout.text().contains("Removed "),
        "{}",
        harness.stdout.text()
    );
}

#[test]
fn package_listing_keeps_scope_order_and_optional_rows() {
    let harness = Harness::new(
        Some(json!({ "packages": ["npm:a", { "source": "npm:b" }] })),
        Some(json!({ "packages": ["npm:c"] })),
    );
    assert!(harness.run(&["list"]));
    assert_eq!(
        harness.stdout.text(),
        "User packages:\n  npm:a\n  npm:b (filtered)\n\nProject packages:\n  npm:c\n"
    );
    let empty = Harness::new(None, None);
    assert!(empty.run(&["list"]));
    assert_eq!(empty.stdout.text(), "No packages installed.\n");
    let project = Harness::new(None, Some(json!({ "packages": ["npm:c"] })));
    assert!(project.run(&["list"]));
    assert_eq!(project.stdout.text(), "Project packages:\n  npm:c\n");
    harness.stdout.clear();
    assert!(harness.run(&["list", "npm:a"]));
    assert_eq!(
        harness.stdout.text(),
        "User packages:\n  npm:a\n  npm:b (filtered)\n\nProject packages:\n  npm:c\n",
        "a list source token is ignored"
    );
}

#[test]
fn package_extension_update_precedes_self() {
    let harness = Harness::new(Some(json!({ "packages": ["npm:a"] })), None);
    assert!(harness.run(&["update"]));
    let stdout = harness.stdout.text();
    let extensions = stdout.find("Updated packages").expect("the extension line");
    let own = stdout.find("Updating maestro with").expect("the own line");
    assert!(extensions < own, "{stdout}");
    let failing = Harness::new(Some(json!({ "packages": "not-an-array" })), None);
    assert!(failing.run(&["update"]));
    assert_eq!(
        failing.stderr.text(),
        "Error: packages is not an array\n",
        "the operation failure is reported, not propagated"
    );
    assert_eq!(failing.exit_code.get(), Some(1));
    assert!(failing.runs().is_empty(), "no own update after a failure");
    assert!(!failing.stdout.text().contains("Updated packages"));
}

#[test]
fn self_update_uses_global_command_without_lookup() {
    let harness = Harness::new(
        Some(json!({ "npmCommand": ["npm", "--global-only"] })),
        Some(json!({ "npmCommand": ["npm", "--project"] })),
    );
    assert!(harness.run(&["update", "--self"]));
    assert_eq!(harness.runs().len(), 1);
    assert!(
        harness.runs()[0].contains("--global-only"),
        "{:?}",
        harness.runs()
    );
    assert!(
        !harness.runs()[0].contains("--project"),
        "{:?}",
        harness.runs()
    );
}

#[test]
fn self_update_defaults_missing_release_identity() {
    let mut harness = Harness::new(Some(json!({})), None);
    harness.origin = "https://controlled.invalid".into();
    harness.release(200, &json!({"version":"999.0.0"}).to_string());
    assert!(harness.run(&["update", "--self"]));
    assert_eq!(harness.runs().len(), 1);
    assert!(
        harness.runs()[0].contains(" maestro") || harness.runs()[0].ends_with("maestro"),
        "{:?}",
        harness.runs()
    );
    assert!(harness.stdout.text().contains("Updated maestro\n"));
    assert_eq!(
        harness
            .release_requests
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert!(harness.stderr.text().is_empty());
    assert_eq!(harness.exit_code.get(), None);
}

#[test]
fn self_update_skips_current_and_older_releases() {
    for body in [json!({ "version": VERSION }), json!({ "version": "0.0.1" })] {
        let mut harness = Harness::new(Some(json!({})), None);
        harness.origin = "https://controlled.invalid".into();
        harness.release(200, &body.to_string());
        assert!(harness.run(&["update", "--self"]));
        assert_eq!(
            harness.stdout.text(),
            format!("maestro is already up to date (v{VERSION})\n")
        );
        assert!(harness.runs().is_empty());
        assert_eq!(harness.exit_code.get(), None);
    }
}

#[test]
fn renamed_release_runs_ordered_replacement() {
    let mut harness = Harness::new(Some(json!({ "npmCommand": ["npm", "--global"] })), None);
    harness.origin = "https://controlled.invalid".into();
    harness.release(
        200,
        &json!({ "version": "0.0.1", "packageName": "maestro-next" }).to_string(),
    );
    assert!(harness.run(&["update", "--self"]));
    assert_eq!(
        harness.runs(),
        [
            "npm --global uninstall -g --ignore-scripts maestro".to_owned(),
            "npm --global install -g --ignore-scripts maestro-next".to_owned()
        ]
    );
    assert!(
        harness.stdout.text().contains(
            "Updating maestro with npm --global uninstall -g --ignore-scripts maestro && npm --global \
             install -g --ignore-scripts maestro-next...\n"
        ),
        "{}",
        harness.stdout.text()
    );
    assert!(harness.stdout.text().ends_with("Updated maestro\n"));
    assert_eq!(
        harness
            .release_requests
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert!(harness.stderr.text().is_empty());
    assert_eq!(harness.exit_code.get(), None);
}

#[test]
fn renamed_install_failure_has_manual_fallback() {
    let mut harness = Harness::new(Some(json!({ "npmCommand": ["npm", "--global"] })), None);
    harness.origin = "https://controlled.invalid".into();
    harness.release(
        200,
        &json!({ "version": "0.0.1", "packageName": "maestro-next" }).to_string(),
    );
    harness
        .state
        .updates
        .steps
        .borrow_mut()
        .push_back(maestro_cli_step(23));
    assert!(harness.run(&["update", "--self"]));
    assert_eq!(harness.runs().len(), 1, "the second step does not run");
    assert!(
        harness.stderr.text().starts_with(
            "Error: npm --global uninstall -g --ignore-scripts maestro exited with code 23\n"
        ),
        "{}",
        harness.stderr.text()
    );
    assert!(
        harness.stderr.text().contains(
            "If this keeps failing, run this command yourself: npm --global uninstall -g --ignore-scripts maestro && \
             npm --global install -g --ignore-scripts maestro-next\n"
        ),
        "{}",
        harness.stderr.text()
    );
    assert!(!harness.stdout.text().contains("Updated maestro"));
    assert_eq!(harness.exit_code.get(), Some(1));
}

/// A status with the supplied exit code.
fn maestro_cli_step(code: i32) -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code << 8)
}

#[test]
fn self_update_unavailable_keeps_instruction_and_location() {
    let harness = Harness::new(None, None);
    harness.state.absent_package.set(true);
    assert!(harness.run(&["update", "--self"]));
    let stderr = harness.stderr.text();
    assert!(
        stderr.starts_with("error: maestro cannot self-update this installation.\n"),
        "out={:?} err={stderr:?}",
        harness.stdout.text()
    );
    assert!(
        stderr.contains("\n\nLocation of maestro executable: /opt/maestro/bin/maestro\n"),
        "{stderr}"
    );
    assert_eq!(harness.exit_code.get(), Some(1));
    assert!(harness.runs().is_empty());
    assert!(!harness.stdout.text().contains("Updating maestro with"));
}

#[test]
fn update_source_suggestion_preserves_settings() {
    let harness = Harness::new(Some(json!({ "npmCommand": ["npm", "--global"] })), None);
    let before = settings_text(&format!("{}/settings.json", harness.agent_dir));
    *harness.state.updates.sync_failure.borrow_mut() = Some("query refused".into());
    assert!(harness.run(&["update", "--self"]));
    assert_eq!(
        harness.stderr.text(),
        "Error: Failed to run npm --global root -g: query refused\n"
    );
    assert_eq!(harness.exit_code.get(), Some(1));
    assert!(!harness.stdout.text().contains("Updated maestro"));
    assert!(harness.runs().is_empty());
    assert_eq!(
        settings_text(&format!("{}/settings.json", harness.agent_dir)),
        before
    );
}

#[test]
fn package_walks_remain_linear() {
    let mut previous = 0;
    for count in [16, 32, 64] {
        let packages: Vec<serde_json::Value> = (0..count)
            .map(|index| json!(format!("npm:pkg{index}")))
            .collect();
        let harness = Harness::new(Some(json!({ "packages": packages })), None);
        assert!(harness.run(&["list"]));
        let visited = harness.state.effects.calls.borrow().len();
        assert!(visited >= count, "{visited} < {count}");
        assert!(visited >= previous, "work shrinks with more rows");
        previous = visited;
        let stdout = harness.stdout.text();
        assert_eq!(stdout.matches("  npm:pkg").count(), count);
    }
}

#[test]
fn self_update_command_keeps_preoperation_global_snapshot() {
    let harness = Harness::new(Some(json!({ "npmCommand": ["npm", "--original"] })), None);
    let live: std::rc::Rc<
        std::cell::RefCell<Option<std::rc::Rc<std::cell::RefCell<SettingsManager>>>>,
    > = std::rc::Rc::new(std::cell::RefCell::new(None));
    let supplied = std::rc::Rc::clone(&harness.state);
    let watch = std::rc::Rc::clone(&live);
    harness.stdout.set_hook(std::rc::Rc::new(move || {
        if let Some(manager) = supplied.supplied.borrow().last() {
            *watch.borrow_mut() = Some(std::rc::Rc::clone(manager));
        }
        if let Some(manager) = &*watch.borrow() {
            manager
                .borrow_mut()
                .set_npm_command(Some(entries(&["npm", "--late"])));
        }
    }));
    assert!(harness.run(&["update", "--self"]));
    assert!(
        harness.runs()[0].contains("--original"),
        "{:?}",
        harness.runs()
    );
    assert!(
        !harness.runs()[0].contains("--late"),
        "{:?}",
        harness.runs()
    );
}

#[test]
fn package_output_keeps_visible_styles_and_lines() {
    let harness = Harness::new(Some(json!({ "packages": ["npm:a"] })), None);
    assert!(harness.run(&["install"]));
    assert_eq!(
        harness.stderr.text(),
        "Missing install source.\nUsage: maestro install <source> [-l]\n"
    );
    harness.stderr.clear();
    assert!(harness.run(&["remove", "npm:absent"]));
    assert_eq!(
        harness.stderr.text(),
        "No matching package found for npm:absent\n"
    );
    harness.stderr.clear();
    harness.stdout.clear();
    assert!(harness.run(&["list"]));
    assert_eq!(harness.stdout.text(), "User packages:\n  npm:a\n");
    let styled = Harness::new(Some(json!({ "packages": ["npm:a"] })), None)
        .stdout
        .text();
    assert_eq!(styled, "", "no styles are written without output");
}

#[test]
fn package_runtime_types_preserve_settings_domains() {
    let harness = Harness::new(Some(json!({ "npmCommand": "npm --scalar" })), None);
    harness.stdout.clear();
    assert!(harness.run(&["list"]));
    assert_eq!(harness.stdout.text(), "No packages installed.\n");
    let malformed = Harness::new(Some(json!({ "npmCommand": 7 })), None);
    assert!(malformed.run(&["update", "--extensions"]));
    assert!(
        malformed.run(&["update", "--self"]),
        "the consuming branch reports the malformed value"
    );
    assert!(
        malformed.stderr.text().contains("Error: "),
        "{}",
        malformed.stderr.text()
    );
    let absent = Harness::new(Some(json!({ "npmCommand": [] })), None);
    assert!(absent.run(&["update", "--self"]));
    assert_eq!(absent.runs().len(), 1);
}

#[test]
fn self_update_install_failure_keeps_nonzero_status() {
    let mut harness = Harness::new(Some(json!({"npmCommand":["npm","--global"]})), None);
    harness.origin = "https://controlled.invalid".into();
    harness.release(
        200,
        &json!({"version":"999.0.0", "packageName":"maestro-next"}).to_string(),
    );
    harness
        .state
        .updates
        .steps
        .borrow_mut()
        .extend([maestro_cli_step(0), maestro_cli_step(23)]);
    assert!(harness.run(&["update", "--self"]));
    assert_eq!(
        harness.runs(),
        [
            "npm --global uninstall -g --ignore-scripts maestro",
            "npm --global install -g --ignore-scripts maestro-next"
        ]
    );
    assert!(harness.stderr.text().contains("exited with code 23"));
    assert_eq!(harness.exit_code.get(), Some(1));
    assert!(!harness.stdout.text().contains("Updated maestro"));
    assert_eq!(
        harness
            .release_requests
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}
