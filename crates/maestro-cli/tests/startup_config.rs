//! The configuration command's place in the startup command sequence.
#![cfg(test)]
#![cfg(unix)]

use crate::support::package_commands::{Harness, State};
use crate::support::presenters::{Harness as Presenters, NoWatch, theme, view};
use maestro_app::presentation_data::input_selectors::{
    ConfigSelectorFactory, ConfigSelectorOptions,
};
use maestro_cli::input::config_selector::ConfigPresentation;
use maestro_test_directory::Scratch;
use maestro_theme::ThemeState;
use serde_json::json;
use std::cell::RefCell;
use std::io;
use std::rc::Rc;

/// The effects one configuration presentation records on the command's log.
struct Recording {
    /// The command state whose step log the presentation extends.
    state: Rc<State>,
    /// The options the presenter received.
    received: Rc<RefCell<Option<ConfigSelectorOptions>>>,
    /// Exit statuses the command requested.
    exits: Rc<RefCell<Vec<i32>>>,
    /// A presentation failure the presenter reports instead.
    failure: Rc<RefCell<Option<&'static str>>>,
}

impl Recording {
    /// The recording of `harness` with no presentation failure.
    fn new(harness: &Harness) -> Self {
        Self {
            state: Rc::clone(&harness.state),
            received: Rc::default(),
            exits: Rc::default(),
            failure: Rc::default(),
        }
    }

    /// A presentation over `theme` and the supplied frame-writer harness.
    ///
    /// The presenter closes the selector as soon as it is built, so the command
    /// completes without an interactive terminal.
    fn presentation(&self, theme: Rc<ThemeState>, writer: &Presenters) -> ConfigPresentation {
        let steps = Rc::clone(&self.state);
        let received = Rc::clone(&self.received);
        let failure = Rc::clone(&self.failure);
        let presenter: ConfigSelectorFactory = Rc::new(move |options, callbacks| {
            steps.steps.borrow_mut().push("present");
            *received.borrow_mut() = Some(options);
            if let Some(reason) = *failure.borrow() {
                return Err(io::Error::other(reason));
            }
            (callbacks.on_close)();
            Ok(view(&Rc::new(RefCell::new(Vec::new()))))
        });
        let exit_steps = Rc::clone(&self.state);
        let exits = Rc::clone(&self.exits);
        let exit = Rc::new(move |code: i32| {
            exits.borrow_mut().push(code);
            exit_steps.steps.borrow_mut().push("exit");
        });
        ConfigPresentation {
            host: writer.host(exit),
            theme,
            watcher: Rc::new(NoWatch),
            presenter,
        }
    }

    /// The options the presenter received.
    fn options(&self) -> ConfigSelectorOptions {
        self.received
            .borrow_mut()
            .take()
            .expect("the presenter runs once")
    }
}

/// The recorded steps of the command.
fn steps(harness: &Harness) -> Vec<&'static str> {
    harness.state.steps.borrow().clone()
}

#[test]
fn config_command_ignores_other_verbs_before_observations() {
    let harness = Harness::new(None, None);
    harness.fail_cwd("a refused verb never observes the directory");
    harness.fail_loading("a refused verb never loads settings");
    let recording = Recording::new(&harness);
    let writer = Presenters::new();
    let scratch = Scratch::new().unwrap();
    let verbs: [&[&str]; 6] = [
        &[],
        &["help"],
        &["help", "config"],
        &["Config"],
        &["--config"],
        &["install", "config"],
    ];
    for args in verbs {
        let presentation = recording.presentation(theme(scratch.path()), &writer);
        assert!(!harness.run_config(args, presentation), "{args:?}");
    }
    assert_eq!(steps(&harness), Vec::<&str>::new(), "no observation runs");
    assert!(harness.state.loaded.borrow().is_empty());
    assert_eq!(harness.stdout.text(), "");
    assert_eq!(harness.stderr.text(), "");
    assert_eq!(harness.exit_code.get(), None);
    assert!(recording.received.borrow().is_none());
    assert!(recording.exits.borrow().is_empty());
}

#[test]
fn config_command_preserves_resolution_presentation_and_exit_order() {
    let harness = Harness::new(None, None);
    let configured = harness.scratch.path().join("configured");
    std::fs::create_dir_all(&configured).unwrap();
    std::fs::write(configured.join("SKILL.md"), "# Configured\n").unwrap();
    harness.write_settings(
        Some(json!({ "skills": [configured.to_string_lossy()] })),
        None,
    );
    let recording = Recording::new(&harness);
    let writer = Presenters::new();
    let scratch = Scratch::new().unwrap();
    let presentation = recording.presentation(theme(scratch.path()), &writer);
    assert!(harness.run_config(&["config", "later", "tokens"], presentation));
    assert_eq!(
        steps(&harness),
        ["cwd", "agent", "settings", "manager", "present", "exit"]
    );
    assert_eq!(
        harness.exit_code.get(),
        None,
        "the presentation exits instead"
    );
    assert!(harness.stdout.text().is_empty());
    assert!(harness.stderr.text().is_empty());

    let options = recording.options();
    assert_eq!(options.cwd, harness.cwd);
    assert_eq!(options.agent_dir, harness.agent_dir);
    assert!(
        Rc::ptr_eq(&options.settings_manager, &harness.supplied()),
        "the presenter keeps the loaded settings owner"
    );
    assert_eq!(options.resolved_paths.skills.len(), 1);
    assert_eq!(
        options.resolved_paths.skills[0].path,
        format!("{}/SKILL.md", configured.to_string_lossy())
    );
    assert_eq!(recording.exits.borrow().as_slice(), [0]);
}

#[test]
fn config_command_reports_loaded_warnings_with_its_own_context() {
    let harness = Harness::new(None, None);
    std::fs::write(
        format!("{}/settings.json", harness.agent_dir),
        "{ not a settings document",
    )
    .unwrap();
    let recording = Recording::new(&harness);
    let writer = Presenters::new();
    let scratch = Scratch::new().unwrap();
    let presentation = recording.presentation(theme(scratch.path()), &writer);
    assert!(harness.run_config(&["config"], presentation));
    assert_eq!(
        steps(&harness),
        [
            "cwd", "agent", "settings", "warning", "warning", "manager", "present", "exit",
        ],
        "a warning and its supplied stack reach standard error before the manager"
    );
    assert!(
        harness
            .stderr
            .text()
            .starts_with("Warning (config command, global settings): "),
        "{}",
        harness.stderr.text()
    );
    assert!(
        harness.supplied().borrow_mut().drain_errors().is_empty(),
        "the same owner drains once"
    );
}

#[test]
fn config_command_propagates_agent_directory_and_manager_failures() {
    let writer = Presenters::new();
    let scratch = Scratch::new().unwrap();
    for (reason, seam) in [
        ("agent directory refused", "agent"),
        ("manager construction refused", "manager"),
    ] {
        let harness = Harness::new(None, None);
        if seam == "agent" {
            harness.fail_agent_dir(reason);
        } else {
            harness.fail_manager(reason);
        }
        let recording = Recording::new(&harness);
        let presentation = recording.presentation(theme(scratch.path()), &writer);
        assert_eq!(
            harness
                .try_run_config(&["config"], presentation)
                .expect_err("the failure is required")
                .to_string(),
            reason,
            "{seam}"
        );
        assert!(
            recording.received.borrow().is_none(),
            "{seam}: no presentation"
        );
        assert!(
            recording.exits.borrow().is_empty(),
            "{seam}: no exit effect"
        );
        assert!(
            harness.stderr.text().is_empty(),
            "{seam}: no package command catch"
        );
        assert_eq!(
            steps(&harness),
            if seam == "agent" {
                vec!["cwd", "agent"]
            } else {
                vec!["cwd", "agent", "settings"]
            },
            "{seam}: the sequence stops at the failing seam"
        );
    }
}

#[test]
fn config_command_propagates_failures_without_exit_or_catch() {
    let writer = Presenters::new();
    let refused = Harness::new(None, None);
    refused.fail_cwd("directory observation refused");
    let recording = Recording::new(&refused);
    let scratch = Scratch::new().unwrap();
    let presentation = recording.presentation(theme(scratch.path()), &writer);
    assert_eq!(
        refused
            .try_run_config(&["config"], presentation)
            .expect_err("the directory failure is required")
            .to_string(),
        "directory observation refused"
    );
    assert!(recording.exits.borrow().is_empty());
    assert!(refused.stderr.text().is_empty());

    let unloaded = Harness::new(None, None);
    unloaded.fail_loading("settings load refused");
    let recording = Recording::new(&unloaded);
    let presentation = recording.presentation(theme(scratch.path()), &writer);
    assert_eq!(
        unloaded
            .try_run_config(&["config"], presentation)
            .expect_err("the settings failure is required")
            .to_string(),
        "settings load refused"
    );
    assert!(recording.exits.borrow().is_empty());

    let unresolved = Harness::new(Some(json!({ "skills": [5] })), None);
    let recording = Recording::new(&unresolved);
    let presentation = recording.presentation(theme(scratch.path()), &writer);
    assert_eq!(
        unresolved
            .try_run_config(&["config"], presentation)
            .expect_err("the resolution failure is required")
            .to_string(),
        "resource entry is not a string"
    );
    assert!(recording.exits.borrow().is_empty());
    assert!(recording.received.borrow().is_none());
    assert!(unresolved.stderr.text().is_empty());

    let unshown = Harness::new(None, None);
    let recording = Recording::new(&unshown);
    *recording.failure.borrow_mut() = Some("presentation refused");
    let presentation = recording.presentation(theme(scratch.path()), &writer);
    assert_eq!(
        unshown
            .try_run_config(&["config"], presentation)
            .expect_err("the presentation failure is required")
            .to_string(),
        "presentation refused"
    );
    assert!(recording.exits.borrow().is_empty());
    assert!(unshown.stderr.text().is_empty(), "no package command catch");
}
