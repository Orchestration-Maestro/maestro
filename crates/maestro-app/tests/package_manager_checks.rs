//! Selected configuration roots feed existing package operations end to end.
#![cfg(test)]
#![cfg(unix)]
#[path = "../../maestro-packages/tests/component_support/mod.rs"]
mod component_support;
use crate::child as probe;
use crate::package_fixtures as fixtures;
#[path = "../../maestro-packages/tests/scratch/mod.rs"]
mod scratch;
use maestro_app::configuration::config::{CONFIG_DIR_NAME, get_agent_dir};
use maestro_app::output_guard::is_stdout_taken_over;
use maestro_app::{
    DefaultPackageManager, PackageManager, PathMetadata, ProgressCallback, ProgressEvent,
    ResolvedPaths, ResolvedResource, SettingsManager,
};
use maestro_packages::{NativePackageOperations, PackageManagerOptions};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, fs, path::Path, rc::Rc};
/// Root-selection operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RootInput {
    branch: String,
    r#override: Option<String>,
}
/// One persisted registration.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Listed {
    source: String,
    scope: maestro_resources::SourceScope,
    filtered: bool,
    installed_path: Option<String>,
}
/// Complete progress notification.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    r#type: String,
    action: String,
    source: String,
    message: Option<String>,
}
/// Complete selected-root workflow output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Roots {
    agent: String,
    resolved: component_support::ExpectedPaths,
    listed: Vec<Listed>,
    events: Vec<Event>,
    removed: bool,
    global: Value,
    project: Value,
}
/// Write a file below the exclusively owned scratch root.
fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
/// Compare every retained field, including absent messages, in event order.
fn check_events(expected: &[Event], actual: &[ProgressEvent], root: &str) {
    assert_eq!(expected.len(), actual.len());
    for (expected, actual) in expected.iter().zip(actual) {
        assert_eq!(
            format!("{:?}", actual.r#type).to_lowercase(),
            expected.r#type
        );
        assert_eq!(
            format!("{:?}", actual.action).to_lowercase(),
            expected.action
        );
        assert_eq!(actual.source, expected.source.replace("$ROOT", root));
        assert_eq!(
            actual.message,
            expected.message.as_ref().map(|s| s.replace("$ROOT", root))
        );
    }
}
/// Register a callback through the embedding SDK, retaining only the observation sink.
fn progress(manager: &impl PackageManager) -> Rc<RefCell<Vec<ProgressEvent>>> {
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::clone(&events);
    let callback: ProgressCallback = Rc::new(move |event: &ProgressEvent| {
        observed.borrow_mut().push(event.clone());
        Ok(())
    });
    manager.set_progress_callback(Some(callback));
    events
}
/// Compare the file-backed settings with the original recorded objects.
fn check_settings(expected: (&Value, &Value), root: &str, cwd: &str, agent: &str) {
    for (path, expected) in [
        (format!("{agent}/settings.json"), expected.0),
        (format!("{cwd}/{CONFIG_DIR_NAME}/settings.json"), expected.1),
    ] {
        let actual: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        let bound: Value =
            serde_json::from_str(&expected.to_string().replace("$ROOT", root)).unwrap();
        assert_eq!(actual, bound);
    }
    let reloaded = SettingsManager::create(cwd, agent, CONFIG_DIR_NAME);
    assert_eq!(
        reloaded.get_global_settings().0,
        serde_json::from_str::<Value>(&expected.0.to_string().replace("$ROOT", root))
            .unwrap()
            .as_object()
            .unwrap()
            .clone()
    );
    assert_eq!(
        reloaded.get_project_settings().0,
        serde_json::from_str::<Value>(&expected.1.to_string().replace("$ROOT", root))
            .unwrap()
            .as_object()
            .unwrap()
            .clone()
    );
}
/// The getter-selected roots reach resolution, registration and persistence without setters.
#[test]
fn selected_configuration_roots_feed_package_resolution_and_persistence() {
    let cases = fixtures::cases::<RootInput, Roots>("root-");
    if let Ok(id) = std::env::var("MAESTRO_PACKAGE_CASE") {
        let case = cases.iter().find(|c| c.id == id).unwrap();
        root_case(case);
        return;
    }
    for case in cases {
        assert_eq!(case.id, format!("root-{}", case.input.branch));
        let dir = scratch::Scratch::new().unwrap();
        let root = dir.0.to_str().unwrap();
        fs::create_dir_all(dir.0.join("work")).unwrap();
        let mut envs = vec![
            ("MAESTRO_PACKAGE_CASE", case.id.as_str()),
            ("MAESTRO_CASE_ROOT", root),
        ];
        let selected = case
            .input
            .r#override
            .as_ref()
            .map(|s| s.replace("$ROOT", root));
        if let Some(selected) = &selected {
            envs.push(("MAESTRO_CODING_AGENT_DIR", selected));
        }
        let home = format!("{root}/home");
        envs.push(("HOME", &home));
        assert!(
            probe::probe(
                maestro_test_process::TestProcess::current(
                    "selected_configuration_roots_feed_package_resolution_and_persistence"
                )
                .unwrap()
                .name(),
                &dir.0.join("work"),
                &envs,
                &["MAESTRO_CODING_AGENT_DIR", "MAESTRO_OFFLINE"]
            )
            .is_empty()
        );
    }
}
/// Execute the complete workflow in an environment-isolated process.
fn root_case(case: &fixtures::Case<RootInput, Roots>) {
    let root = std::env::var("MAESTRO_CASE_ROOT").unwrap();
    let cwd = format!("{root}/work");
    let agent = get_agent_dir().unwrap();
    assert_eq!(agent, case.expected.agent.replace("$ROOT", &root));
    setup_root(case, &root, &cwd, &agent);
    let settings = Rc::new(RefCell::new(SettingsManager::create(
        &cwd,
        &agent,
        CONFIG_DIR_NAME,
    )));
    let local = Rc::new(tokio::task::LocalSet::new());
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: cwd.clone(),
            agent_dir: agent.clone(),
            settings_manager: Rc::clone(&settings),
        },
        NativePackageOperations::new(|_| false, Rc::new(is_stdout_taken_over), &local),
    );
    let events = progress(&manager);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(local.run_until(async {
        let resolved: ResolvedPaths = manager.resolve(None).await.unwrap();
        let resource: &ResolvedResource = &resolved.extensions[0];
        let metadata: &PathMetadata = &resource.metadata;
        assert_eq!(metadata.scope, maestro_resources::SourceScope::Project);
        case.expected.resolved.check(resolved, &root);
        let extra = format!("{cwd}/extra");
        manager.install_and_persist(&extra, None).await.unwrap();
        let flushed = settings.borrow().flush();
        flushed.await;
        let listed = manager.list_configured_packages().unwrap();
        check_listed(&case.expected.listed, &listed, &root);
        assert_eq!(
            manager.remove_and_persist(&extra, None).await.unwrap(),
            case.expected.removed
        );
        let flushed = settings.borrow().flush();
        flushed.await;
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    check_events(&case.expected.events, &events.borrow(), &root);
    check_settings(
        (&case.expected.global, &case.expected.project),
        &root,
        &cwd,
        &agent,
    );
}
/// Independently selected outdated scope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateInput {
    selected: String,
}
/// One complete native command observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Call {
    phase: String,
    command: String,
    args: Vec<String>,
    cwd: String,
}
/// Complete update result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Updated {
    calls: Vec<Call>,
    events: Vec<Event>,
    global: Value,
    project: Value,
}
/// Unequal versions in both directions select only the outdated scope.
#[test]
fn selected_package_roots_keep_update_pins_and_progress() {
    let cases = fixtures::cases::<UpdateInput, Updated>("update-");
    if let Ok(id) = std::env::var("MAESTRO_PACKAGE_CASE") {
        let case = cases.iter().find(|c| c.id == id).unwrap();
        update_case(case);
        return;
    }
    for case in cases {
        assert_eq!(case.id, format!("update-{}", case.input.selected));
        let dir = scratch::Scratch::new().unwrap();
        let root = dir.0.to_str().unwrap();
        fs::create_dir_all(dir.0.join("work")).unwrap();
        let script = dir.0.join("controlled-npm");
        assert!(
            std::process::Command::new("install")
                .args([
                    "-m",
                    "0700",
                    concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/tests/fixtures/controlled-npm.sh"
                    )
                ])
                .arg(&script)
                .status()
                .unwrap()
                .success()
        );
        let agent = format!("{root}/selected");
        let log = format!("{root}/calls");
        let npm = format!("{root}/global/node_modules");
        assert!(
            probe::probe(
                maestro_test_process::TestProcess::current(
                    "selected_package_roots_keep_update_pins_and_progress"
                )
                .unwrap()
                .name(),
                &dir.0.join("work"),
                &[
                    ("MAESTRO_PACKAGE_CASE", &case.id),
                    ("MAESTRO_CASE_ROOT", root),
                    ("MAESTRO_CODING_AGENT_DIR", &agent),
                    ("MAESTRO_COMMAND_LOG", &log),
                    ("MAESTRO_NPM_ROOT", &npm)
                ],
                &["MAESTRO_OFFLINE"]
            )
            .is_empty()
        );
    }
}
/// Seed unequal versions and drive the file-backed configured updater.
fn update_case(case: &fixtures::Case<UpdateInput, Updated>) {
    let root = std::env::var("MAESTRO_CASE_ROOT").unwrap();
    let cwd = format!("{root}/work");
    let agent = get_agent_dir().unwrap();
    assert_eq!(agent, format!("{root}/selected"));
    let script = format!("{root}/controlled-npm");
    let global = case
        .expected
        .global
        .to_string()
        .replace("controlled-npm", &script);
    write(Path::new(&format!("{agent}/settings.json")), &global);
    write(
        Path::new(&format!("{cwd}/{CONFIG_DIR_NAME}/settings.json")),
        &case.expected.project.to_string(),
    );
    setup_versions(case, &root, &cwd);
    let settings = Rc::new(RefCell::new(SettingsManager::create(
        &cwd,
        &agent,
        CONFIG_DIR_NAME,
    )));
    let local = Rc::new(tokio::task::LocalSet::new());
    let manager = Rc::new(DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: cwd.clone(),
            agent_dir: agent.clone(),
            settings_manager: Rc::clone(&settings),
        },
        NativePackageOperations::new(|_| false, Rc::new(is_stdout_taken_over), &local),
    ));
    let events = progress(manager.as_ref());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(local.run_until(async {
        manager.update(None).await.unwrap();
        let flushed = settings.borrow().flush();
        flushed.await;
    }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    check_events(&case.expected.events, &events.borrow(), &root);
    check_settings(
        (
            &serde_json::from_str(&global).unwrap(),
            &case.expected.project,
        ),
        &root,
        &cwd,
        &agent,
    );
    check_calls(&case.expected.calls, &root);
}

/// Populate the real author packages and unequal scoped settings.
fn setup_root(case: &fixtures::Case<RootInput, Roots>, root: &str, cwd: &str, agent: &str) {
    fs::create_dir_all(format!("{cwd}/extra")).unwrap();
    write(Path::new(&format!("{cwd}/.git")), "gitdir: unused");
    for (dir, names) in [
        ("user-pkg", vec!["visible.wasm", "hidden.wasm"]),
        ("project-pkg", vec!["project.wasm"]),
    ] {
        let base = Path::new(root).join(dir);
        write(
            &base.join("package.json"),
            &json!({"maestro":{"extensions":names}}).to_string(),
        );
        for name in names {
            fs::copy(
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../maestro-packages/tests/fixtures/author_component.wasm"
                ),
                base.join(name),
            )
            .unwrap();
        }
    }
    write(
        Path::new(&format!("{agent}/settings.json")),
        &case.expected.global.to_string().replace("$ROOT", root),
    );
    write(
        Path::new(&format!("{cwd}/{CONFIG_DIR_NAME}/settings.json")),
        &case.expected.project.to_string().replace("$ROOT", root),
    );
    if ["absolute", "tilde"].contains(&case.input.branch.as_str()) {
        write(
            Path::new(&format!("{root}/home/.maestro/agent/settings.json")),
            "{\"packages\":[\"/decoy-do-not-read\"]}",
        );
    }
}

/// Give only the selected scope an outdated installed version.
fn setup_versions(case: &fixtures::Case<UpdateInput, Updated>, root: &str, cwd: &str) {
    for (scope, base) in [
        ("user", format!("{root}/global/node_modules")),
        (
            "project",
            format!("{cwd}/{CONFIG_DIR_NAME}/npm/node_modules"),
        ),
    ] {
        let version = if case.input.selected == scope {
            "1.0.0"
        } else {
            "2.0.0"
        };
        write(
            Path::new(&format!("{base}/outdated/package.json")),
            &json!({"version":version}).to_string(),
        );
    }
}

/// Compare every argv, working directory and operation phase in order.
fn check_calls(expected: &[Call], root: &str) {
    let log = fs::read_to_string(format!("{root}/calls")).unwrap();
    let calls: Vec<_> = log
        .lines()
        .map(|s| s.split('\t').collect::<Vec<_>>())
        .collect();
    assert_eq!(calls.len(), expected.len());
    for (actual, expected) in calls.iter().zip(expected) {
        assert_eq!(actual[0], expected.cwd.replace("$ROOT", root));
        assert_eq!(actual[1], format!("{root}/{}", expected.command));
        assert_eq!(actual[3], expected.phase);
        assert_eq!(
            actual[2..],
            expected
                .args
                .iter()
                .map(|s| s.replace("$ROOT", root))
                .collect::<Vec<_>>()
        );
    }
}

/// Compare ordered registrations after persistence and before removal.
fn check_listed(expected: &[Listed], actual: &[maestro_packages::ConfiguredPackage], root: &str) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.source, expected.source.replace("$ROOT", root));
        assert_eq!(
            format!("{:?}", actual.scope).to_lowercase(),
            format!("{:?}", expected.scope).to_lowercase()
        );
        assert_eq!(actual.filtered, expected.filtered);
        assert_eq!(
            actual.installed_path,
            expected
                .installed_path
                .as_ref()
                .map(|s| s.replace("$ROOT", root))
        );
    }
}
