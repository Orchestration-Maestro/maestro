//! Public replay of strict recorded package-update observations.
mod native;
mod parallel;
mod types;
use super::support;
use maestro_packages::{
    DefaultPackageManager, PackageManager, PackageManagerOptions, ProgressEventType,
};
use maestro_settings::{PackageSource, SettingsListEntry, SettingsManager};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
use types::{Case, Command, Event, Expected, Files, GitInput, Input, Response};

/// Runs each committed case once, comparing every retained observation.
pub fn replay() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/configured-update.json")).unwrap();
    let mut ids = std::collections::HashSet::new();
    assert_eq!(cases.len(), 81);
    for case in cases {
        assert!(ids.insert(case.id.clone()), "duplicate corpus case");
        assert_eq!(run(case.input), case.expected, "corpus case {}", case.id);
    }
}
/// The manager and the caller's retained settings and process observations.
type Manager = Rc<DefaultPackageManager<support::Controlled>>;
/// Creates authored fixture bases, without depending on ambient directories.
fn manager(
    user: &Value,
    project: &Value,
) -> (Manager, Rc<RefCell<SettingsManager>>, Rc<support::Effects>) {
    let (_, settings, effects) = support::manager(&json!({"packages":user}));
    settings.borrow_mut().set_project_packages(
        project
            .as_array()
            .unwrap()
            .iter()
            .cloned()
            .map(PackageSource::Unknown)
            .collect(),
    );
    let manager = Rc::new(DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: "/fixture/cwd".into(),
            agent_dir: "/fixture/agent".into(),
            settings_manager: settings.clone(),
        },
        support::Controlled(effects.clone()),
    ));
    (manager, settings, effects)
}
/// Replays one operation with controlled process responses or native repository effects.
fn run(input: Input) -> Expected {
    match input {
        Input::Native { scenario } => Expected {
            native: Some(native::run(&scenario)),
            ..Expected::default()
        },
        Input::Parallel {
            user_jobs,
            project_jobs,
            git_jobs,
        } => Expected {
            groups: Some(parallel::run(user_jobs, project_jobs, git_jobs)),
            ..Expected::default()
        },
        other => controlled(other),
    }
}
/// Selects settings and an optional named argument for each public input class.
fn settings_input(input: &Input) -> (Value, Value, Option<String>) {
    match input {
        Input::Named {
            user,
            project,
            source,
        } => (user.clone(), project.clone(), Some(source.clone())),
        Input::Offline { user, source, .. } => (user.clone(), json!([]), source.clone()),
        Input::Skip { source } => (json!([source]), json!([]), None),
        Input::Npm { .. } => (json!([]), json!(["npm:a"]), None),
        Input::Batch { user, project, .. } => (user.clone(), project.clone(), None),
        Input::Callback { .. } => (json!(["npm:a"]), json!([]), None),
        Input::Git(_) => (json!(["git:github.com/acme/tool"]), json!([]), None),
        Input::Native { .. } | Input::Parallel { .. } => unreachable!(),
    }
}
/// Runs controlled work to completion, including workers surviving a returned error.
fn controlled(input: Input) -> Expected {
    let (user, project, source) = settings_input(&input);
    let (manager, settings, effects) = manager(&user, &project);
    let observed = Rc::new(RefCell::new(Expected::default()));
    observe_commands(&effects, &observed);
    if !matches!(input, Input::Named { .. } | Input::Offline { .. }) {
        observe_events(&manager, &observed);
    }
    configure(&input, &manager, &settings, &effects, &observed);
    let local = Rc::new(tokio::task::LocalSet::new());
    *effects.local.borrow_mut() = Rc::downgrade(&local);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = runtime.block_on(local.run_until(manager.update(source.as_deref())));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    let mut actual = std::mem::take(&mut *observed.borrow_mut());
    actual.error = result.err().map(|error| error.to_string());
    if let Input::Batch { command, .. } = input {
        actual.settings = Some(json!(settings.borrow().get_global_settings().0));
        actual.project = Some(json!(settings.borrow().get_project_settings().0));
        if command.is_some() {
            actual.settings =
                Some(json!({"packages":settings.borrow().get_global_settings().0["packages"]}));
            actual.project =
                Some(json!({"packages":settings.borrow().get_project_settings().0["packages"]}));
        }
        actual.files = project_files(&effects);
    }
    actual
}
/// Records the ordered literal command operands through the existing replaceable seam.
fn observe_commands(effects: &Rc<support::Effects>, observed: &Rc<RefCell<Expected>>) {
    let output = observed.clone();
    *effects.run_hook.borrow_mut() = Some(Rc::new(move |program, args, cwd| {
        output
            .borrow_mut()
            .commands
            .push(command(program, args, cwd));
        Box::pin(async { Ok(Some(0)) })
    }));
    let output = observed.clone();
    let weak = Rc::downgrade(effects);
    *effects.capture_hook.borrow_mut() = Some(Rc::new(move |program, args, cwd| {
        output.borrow_mut().commands.push(captured_command(
            &weak.upgrade().unwrap(),
            program,
            args,
            cwd,
        ));
        Box::pin(async { Ok("\"2\"".into()) })
    }));
}
/// Constructs an observation, not an expected command.
fn command(program: &str, args: &[String], cwd: Option<&str>) -> Command {
    Command {
        program: program.into(),
        args: args.to_vec(),
        cwd: cwd.map(str::to_owned),
        timeout_ms: None,
    }
}
/// Records the live callback's event fields without changing them.
fn observe_events(manager: &Manager, observed: &Rc<RefCell<Expected>>) {
    let output = observed.clone();
    manager.set_progress_callback(Some(Rc::new(move |event| {
        output.borrow_mut().events.push(event_record(event, None));
        Ok(())
    })));
}
/// Preserves each observable event field and the optional replacement observer.
fn event_record(event: &maestro_packages::ProgressEvent, observer: Option<String>) -> Event {
    Event {
        r#type: match event.r#type {
            ProgressEventType::Start => "start",
            ProgressEventType::Complete => "complete",
            ProgressEventType::Error => "error",
            ProgressEventType::Progress => "progress",
        }
        .into(),
        action: "update".into(),
        source: event.source.clone(),
        message: event.message.clone(),
        observer,
    }
}
/// Applies the input's process/filesystem responses before invoking the public operation.
fn configure(
    input: &Input,
    manager: &Manager,
    settings: &Rc<RefCell<SettingsManager>>,
    effects: &Rc<support::Effects>,
    observed: &Rc<RefCell<Expected>>,
) {
    *effects.manifest.borrow_mut() = None;
    match input {
        Input::Offline { offline, .. } => *effects.offline.borrow_mut() = Some(offline.clone()),
        Input::Npm { installed, latest } => {
            configure_npm(installed.as_deref(), latest.clone(), effects, observed);
        }
        Input::Batch {
            command,
            existing,
            user,
            project,
        } => {
            set_command(settings, command.as_ref());
            if !user.as_array().unwrap().is_empty() && !project.as_array().unwrap().is_empty() {
                parallel_batches(effects, observed);
            }
            if *existing {
                effects.exists.set(true);
                effects.writes.borrow_mut().extend([
                    (
                        "/fixture/cwd/.maestro/npm/package.json".into(),
                        "existing manifest".into(),
                    ),
                    (
                        "/fixture/cwd/.maestro/npm/.gitignore".into(),
                        "existing ignore".into(),
                    ),
                ]);
            }
        }
        Input::Callback { phase } => configure_callback(phase, manager, effects, observed),
        Input::Git(git) => configure_git(git, settings, effects, observed),
        _ => {}
    }
}
/// Retains live command configuration in its existing settings owner.
fn set_command(settings: &Rc<RefCell<SettingsManager>>, command: Option<&Vec<String>>) {
    settings.borrow_mut().set_npm_command(command.map(|args| {
        args.iter()
            .map(|arg| SettingsListEntry::String(arg.clone()))
            .collect()
    }));
}
/// Installed-version and latest-process responses for an npm candidate.
fn configure_npm(
    installed: Option<&str>,
    latest: Response,
    effects: &Rc<support::Effects>,
    observed: &Rc<RefCell<Expected>>,
) {
    effects.exists.set(installed.is_some());
    *effects.manifest.borrow_mut() = installed
        .filter(|text| !["NO_FILE", "DIRECTORY"].contains(text))
        .map(str::to_owned);
    let output = observed.clone();
    let weak = Rc::downgrade(effects);
    *effects.capture_hook.borrow_mut() = Some(Rc::new(move |program, args, cwd| {
        output.borrow_mut().commands.push(captured_command(
            &weak.upgrade().unwrap(),
            program,
            args,
            cwd,
        ));
        let response = response(&latest);
        Box::pin(async move { response })
    }));
}
/// Converts one recorded process response at its declared adapter boundary.
fn response(response: &Response) -> std::io::Result<String> {
    match response {
        Response::Text(text) => Ok(text.clone()),
        Response::Failure(failure) => Err(std::io::Error::other(failure.error.clone())),
    }
}
/// Reports actual prepared-file records from the controlled filesystem seam.
fn project_files(effects: &support::Effects) -> Option<Files> {
    let files = effects.writes.borrow();
    let manifest = files
        .iter()
        .rev()
        .find(|(path, _)| path == "/fixture/cwd/.maestro/npm/package.json")?;
    let ignore = files
        .iter()
        .rev()
        .find(|(path, _)| path == "/fixture/cwd/.maestro/npm/.gitignore")?;
    Some(Files {
        manifest: manifest.1.clone(),
        ignore: ignore.1.clone(),
    })
}
/// Installs stateful callback failures and replacement without retaining its owner.
fn configure_callback(
    phase: &str,
    manager: &Manager,
    effects: &Rc<support::Effects>,
    observed: &Rc<RefCell<Expected>>,
) {
    let phase = phase.to_owned();
    if phase == "error" {
        let output = observed.clone();
        *effects.run_hook.borrow_mut() = Some(Rc::new(move |program, args, cwd| {
            output
                .borrow_mut()
                .commands
                .push(command(program, args, cwd));
            Box::pin(async { Err(std::io::Error::other("command failed")) })
        }));
    }
    let weak = Rc::downgrade(manager);
    let output = observed.clone();
    manager.set_progress_callback(Some(Rc::new(move |event| {
        let record = event_record(event, None);
        let fails = record.r#type == phase;
        output.borrow_mut().events.push(record);
        if phase == "replace" && event.r#type == ProgressEventType::Start {
            let next = output.clone();
            weak.upgrade()
                .unwrap()
                .set_progress_callback(Some(Rc::new(move |event| {
                    next.borrow_mut()
                        .events
                        .push(event_record(event, Some("new".into())));
                    Ok(())
                })));
        }
        if fails {
            Err(std::io::Error::other(format!("callback {phase}")))
        } else {
            Ok(())
        }
    })));
}
/// Installs controlled Git probes while retaining command phase boundaries.
fn configure_git(
    input: &GitInput,
    settings: &Rc<RefCell<SettingsManager>>,
    effects: &Rc<support::Effects>,
    observed: &Rc<RefCell<Expected>>,
) {
    set_command(settings, input.command.as_ref());
    effects.exists.set(!input.missing);
    if !input.manifest {
        effects
            .absent_under
            .borrow_mut()
            .push("/fixture/agent/git/github.com/acme/tool/package.json".into());
    }
    let fetched = Rc::new(std::cell::Cell::new(false));
    let output = observed.clone();
    let after_fetch = fetched.clone();
    let failed = input.fail.clone();
    let set_head_error = input.set_head_error;
    *effects.run_hook.borrow_mut() = Some(Rc::new(move |program, args, cwd| {
        output
            .borrow_mut()
            .commands
            .push(command(program, args, cwd));
        after_fetch.set(after_fetch.get() || args[0] == "fetch");
        let fails = failed.as_deref() == Some(&args[0]) || (args[0] == "remote" && set_head_error);
        let message = format!("failed {}", args[0]);
        Box::pin(async move {
            if fails {
                Err(std::io::Error::other(message))
            } else {
                Ok(Some(0))
            }
        })
    }));
    git_probes(input, effects, observed, fetched);
}
/// Captures the source-defined upstream/fallback and refreshed-head responses.
fn git_probes(
    input: &GitInput,
    effects: &Rc<support::Effects>,
    observed: &Rc<RefCell<Expected>>,
    fetched: Rc<std::cell::Cell<bool>>,
) {
    let upstream = input.upstream.clone();
    let upstream_head = input.upstream_head.clone();
    let origin_head = input.origin_head.clone();
    let symbolic = input.symbolic.clone();
    let current = input.current.unwrap_or(false);
    let fail_capture = input.fail_capture.clone();
    let output = observed.clone();
    let weak = Rc::downgrade(effects);
    *effects.capture_hook.borrow_mut() = Some(Rc::new(move |program, args, cwd| {
        output.borrow_mut().commands.push(captured_command(
            &weak.upgrade().unwrap(),
            program,
            args,
            cwd,
        ));
        let selected = if args[0] == "symbolic-ref" {
            symbolic.as_ref()
        } else if args[1] == "--abbrev-ref" {
            upstream.as_ref()
        } else if args[1] == "origin/HEAD" {
            origin_head.as_ref()
        } else {
            upstream_head.as_ref()
        };
        let value = if args[1] == "HEAD" {
            Ok(if current { "new" } else { "old" }.into())
        } else if let Some(value) = selected {
            response(value)
        } else {
            Ok(if args[0] == "symbolic-ref" {
                "refs/remotes/origin/main"
            } else {
                "new"
            }
            .into())
        };
        let fails = fail_capture.as_deref() == Some(&args[1])
            || (fail_capture.as_deref() == Some("REFRESH")
                && args[1] == "@{upstream}"
                && fetched.get());
        let result = if fails {
            Err(std::io::Error::other(format!("capture failed {}", args[1])))
        } else {
            value
        };
        Box::pin(async move { result })
    }));
}

/// Seals both batch command admissions before either completion callback can run.
fn parallel_batches(effects: &support::Effects, observed: &Rc<RefCell<Expected>>) {
    let barrier = Rc::new(tokio::sync::Barrier::new(2));
    let output = observed.clone();
    *effects.run_hook.borrow_mut() = Some(Rc::new(move |program, args, cwd| {
        output
            .borrow_mut()
            .commands
            .push(command(program, args, cwd));
        let barrier = barrier.clone();
        Box::pin(async move {
            barrier.wait().await;
            Ok(Some(0))
        })
    }));
}

/// Reads the actual deadline recorded by the captured-command operations adapter.
fn captured_command(
    effects: &support::Effects,
    program: &str,
    args: &[String],
    cwd: Option<&str>,
) -> Command {
    let timeout_ms = effects
        .operands
        .borrow()
        .last()
        .unwrap()
        .1
        .map(|timeout| u64::try_from(timeout.as_millis()).unwrap());
    Command {
        timeout_ms,
        ..command(program, args, cwd)
    }
}
