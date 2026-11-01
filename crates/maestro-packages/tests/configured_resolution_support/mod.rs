//! Configured native recipes observed through the public operation.
#![cfg(test)]
use super::{resolution_support, scratch, support};
use maestro_packages::{DefaultPackageManager, PackageManager, PackageManagerOptions};
use maestro_settings::SettingsManager;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path, rc::Rc};
/// One complete configured operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    /// Unique input label.
    case: String,
    /// Native input.
    recipe: Recipe,
    /// Completed observations.
    expected: Value,
}
/// Native tree and selected settings.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Recipe {
    /// User preferences.
    global: Value,
    /// Project preferences.
    project: Value,
    /// Package manifest inputs.
    packages: BTreeMap<String, Value>,
    /// Directory inputs.
    dirs: Vec<String>,
    /// Native symbolic links.
    links: BTreeMap<String, String>,
    /// Failing native directory reads.
    denied: Vec<String>,
    /// Authored user base.
    #[serde(rename = "agentDir")]
    agent_dir: Option<String>,
    /// Authored project base.
    cwd: Option<String>,
    /// Authored manager cwd.
    #[serde(rename = "managerCwd")]
    manager_cwd: Option<String>,
    /// Home base.
    home: Option<String>,
    /// Missing-source choice.
    action: Option<String>,
    /// Offline environment value.
    offline: Option<String>,
    /// Startup offline selection supplied to the manager.
    selection: Option<bool>,
    /// Failed acquisition.
    #[serde(rename = "failInstall")]
    fail_install: Option<bool>,
    /// Failed ambient cwd read.
    #[serde(rename = "cwdFailure")]
    cwd_failure: Option<bool>,
    /// Omit the default git marker.
    #[serde(rename = "noGit")]
    no_git: Option<bool>,
    /// Repeat with live settings.
    twice: Option<bool>,
    /// Settings changed from the callback.
    #[serde(rename = "editDuringCallback")]
    edit_during_callback: Option<Value>,
    /// Admission gate.
    #[serde(rename = "callbackGate")]
    callback_gate: Option<bool>,
    /// Files to create.
    files: BTreeMap<String, String>,
}
/// One native tree with surviving caller aliases.
struct Fixture {
    /// The tree the calling process owns.
    base: std::path::PathBuf,
    /// Public operation owner.
    manager: DefaultPackageManager<support::Controlled>,
    /// Surviving settings alias.
    settings: Rc<std::cell::RefCell<SettingsManager>>,
    /// Guards that give denied entries their permissions back before the tree goes.
    _denied: Vec<maestro_test_directory::Restore>,
    /// Completed effect observations.
    effects: Rc<support::Effects>,
}
/// Runs complete selected recipes.
pub fn check(names: &[&str]) {
    let cases: Vec<Record> =
        serde_json::from_str(include_str!("../fixtures/configured_resolution.json")).unwrap();
    let mut identities = std::collections::HashSet::new();
    for case in &cases {
        assert!(identities.insert(&case.case), "duplicate recipe");
    }
    let cases: Vec<_> = cases
        .into_iter()
        .filter(|case| names.contains(&case.case.as_str()))
        .collect();
    assert_eq!(cases.len(), names.len());
    for case in cases {
        run_case(&case);
    }
}
/// Runs one recipe in a child process with the recipe's home and offline value.
fn run_case(case: &Record) {
    let Some(base) = resolution_support::case_in_child(&case.case, |base| {
        let home = case
            .recipe
            .home
            .as_deref()
            .map_or_else(|| format!("{base}/home"), |s| s.replace("$BASE", base));
        vec![
            ("HOME", Some(home)),
            ("MAESTRO_OFFLINE", case.recipe.offline.clone()),
        ]
    }) else {
        return;
    };
    // A privileged run reads denied entries anyway, so such a recipe has nothing to check.
    let Some(fixture) = setup(&case.recipe, base) else {
        return;
    };
    let _removed = case.recipe.cwd_failure.unwrap_or_default().then(|| {
        let owner = scratch::Scratch::new_in(&fixture.base).unwrap();
        owner.enter_removed("removed").unwrap();
        owner
    });
    resolution_support::run(observe(&fixture, case)).unwrap();
}
/// Constructs the configured manager from real scoped settings files; `None` when this
/// process still reads the recipe's denied entries.
fn setup(recipe: &Recipe, root: std::path::PathBuf) -> Option<Fixture> {
    let base = root.to_str().unwrap();
    let cwd = recipe
        .cwd
        .as_deref()
        .map_or_else(|| format!("{base}/project"), |s| s.replace("$BASE", base));
    let user = recipe
        .agent_dir
        .as_deref()
        .map_or_else(|| format!("{base}/user"), |s| s.replace("$BASE", base));
    fs::create_dir_all(&cwd).unwrap();
    fs::create_dir_all(&user).unwrap();
    if !recipe.no_git.unwrap_or_default() {
        fs::write(Path::new(&cwd).join(".git"), "gitdir: unused").unwrap();
    }
    let denied = native_tree(&root, recipe)?;
    let mut global = substitute(&recipe.global, base);
    global["npmCommand"] = json!(["npm"]);
    fs::write(Path::new(&user).join("settings.json"), global.to_string()).unwrap();
    fs::create_dir_all(Path::new(&cwd).join(".maestro")).unwrap();
    fs::write(
        Path::new(&cwd).join(".maestro/settings.json"),
        substitute(&recipe.project, base).to_string(),
    )
    .unwrap();
    let settings = Rc::new(std::cell::RefCell::new(SettingsManager::create(
        &cwd, &user, ".maestro",
    )));
    let effects = effects(recipe, base);
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: recipe.manager_cwd.clone().unwrap_or(cwd),
            agent_dir: user,
            settings_manager: Rc::clone(&settings),
        },
        support::Controlled(Rc::clone(&effects)),
    )
    .with_offline(recipe.selection);
    Some(Fixture {
        _denied: denied,
        base: root,
        manager,
        settings,
        effects,
    })
}
/// Creates each supplied file, directory, package and link, then denies access to the
/// recipe's denied entries; `None` when this process is not refused.
fn native_tree(root: &Path, recipe: &Recipe) -> Option<Vec<maestro_test_directory::Restore>> {
    let base = root.to_str().unwrap();
    for (name, text) in &recipe.files {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text.replace("$BASE", base)).unwrap();
    }
    for name in &recipe.dirs {
        fs::create_dir_all(root.join(name)).unwrap();
    }
    for (name, manifest) in &recipe.packages {
        package(&root.join(name), &substitute(manifest, base));
    }
    for (name, target) in &recipe.links {
        let dest = root.join(name);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        if dest.exists() {
            fs::remove_file(&dest).unwrap();
        }
        std::os::unix::fs::symlink(root.join(target), dest).unwrap();
    }
    recipe
        .denied
        .iter()
        .map(|name| maestro_test_directory::deny(root.join(name), 0o000).unwrap())
        .collect()
}
/// Supplies controlled acquisition failures below their owning wrappers.
fn effects(recipe: &Recipe, base: &str) -> Rc<support::Effects> {
    let effects = Rc::new(support::Effects::default());
    effects.outputs.borrow_mut().push_back(Ok(support::output(
        &format!("{base}/global_node_modules"),
        "",
        Some(0),
    )));
    acquisition(&effects, base, recipe.fail_install.unwrap_or_default());
    effects
}
/// Observes both the initial resolution and any explicitly requested subsequent call.
async fn observe(fixture: &Fixture, case: &Record) -> std::io::Result<()> {
    let manager = &fixture.manager;
    let settings = &fixture.settings;
    let effects = &fixture.effects;
    let base = fixture.base.to_str().unwrap();
    let events = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let sink = Rc::clone(&events);
    manager.set_progress_callback(Some(Rc::new(move |event| {
        sink.borrow_mut().push(format!("{:?}", event.r#type));
        Ok(())
    })));
    let callbacks = std::cell::RefCell::new(Vec::new());
    let action = case.recipe.action.as_deref();
    let callback = missing_callback(
        &callbacks,
        settings,
        action,
        case.recipe.edit_during_callback.as_ref(),
    );
    let cb = action.map(|_| &callback);
    let (completed, pending) = if case.recipe.callback_gate.unwrap_or_default() {
        gated_resolution(manager, effects, &callbacks).await
    } else {
        (manager.resolve(cb.map(|cb| cb as &_)).await, None)
    };
    let (result, error) = match completed {
        Ok(result) => (output(result, base), Value::Null),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
            (Value::Null, json!({"kind":"InvalidData"}))
        }
        Err(error) => (Value::Null, json!({"message":error.to_string()})),
    };
    let second = if case.recipe.twice.unwrap_or_default() {
        output(manager.resolve(cb.map(|cb| cb as &_)).await?, base)
    } else {
        Value::Null
    };
    let calls: Vec<_> = effects
        .transcript
        .borrow()
        .iter()
        .map(|call| command_output(call, base))
        .collect();
    let callback_output = callbacks
        .borrow()
        .iter()
        .map(|s| s.replace(base, "$BASE"))
        .collect::<Vec<_>>();
    let mut observed = json!({"result":result,"error":error,"calls":calls,"events":*events.borrow(),"callbacks":callback_output,"second":second});
    if let Some(pending) = pending {
        observed["pending"] = pending;
    }
    assert_eq!(observed, case.expected, "case {}", case.case);
    let flush = settings.borrow().flush();
    flush.await;
    Ok(())
}
/// Suspends the same operation at the first callback and releases its completion signal.
async fn gated_resolution(
    manager: &DefaultPackageManager<support::Controlled>,
    effects: &support::Effects,
    callbacks: &std::cell::RefCell<Vec<String>>,
) -> (
    std::io::Result<maestro_packages::ResolvedPaths>,
    Option<Value>,
) {
    let (release, receiver) = tokio::sync::oneshot::channel();
    let receiver = std::cell::RefCell::new(Some(receiver));
    let gated = gated_callback(callbacks, &receiver);
    let mut work = manager.resolve(Some(&gated));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(work.as_mut().poll(&mut context).is_pending());
    let pending = json!({"calls":*effects.transcript.borrow(),"callbacks":*callbacks.borrow()});
    release.send(()).unwrap();
    (work.await, Some(pending))
}
/// Preserves command order, repetitions and complete operands while changing only framing.
fn command_output(call: &Value, base: &str) -> Value {
    let mut call: Value = serde_json::from_str(&support::recorded_stages(
        &call.to_string().replace(base, "$BASE"),
    ))
    .unwrap();
    let object = call.as_object_mut().unwrap();
    object.shift_remove("mode");
    if let Some(cwd) = object.shift_remove("cwd") {
        object.insert("options".into(), json!({"cwd":cwd}));
    }
    call
}
/// Reuses the existing complete projection, omitting an absent optional field.
fn output(result: maestro_packages::ResolvedPaths, base: &str) -> Value {
    let mut result = resolution_support::output(result, base);
    for items in result.as_object_mut().unwrap().values_mut() {
        for item in items.as_array_mut().unwrap() {
            if item["metadata"]["baseDir"].is_null() {
                item["metadata"]
                    .as_object_mut()
                    .unwrap()
                    .shift_remove("baseDir");
            }
        }
    }
    result
}
/// Substitutes only the scratch root in recorded inputs.
fn substitute(value: &Value, base: &str) -> Value {
    serde_json::from_str(&value.to_string().replace("$BASE", base)).unwrap()
}
/// Writes known acquired resources.
fn package(root: &Path, manifest: &Value) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("package.json"),
        json!({"version":"1.0.0","maestro":manifest}).to_string(),
    )
    .unwrap();
    for name in [
        "entry.ts",
        "other.ts",
        "skill.md",
        "prompt.md",
        "theme.json",
    ] {
        fs::write(root.join(name), name).unwrap();
    }
}
/// Captures effects below the acquisition owner's wrappers.
fn acquisition(effects: &Rc<support::Effects>, base: &str, fail: bool) {
    let base = base.to_owned();
    *effects.run_hook.borrow_mut() = Some(Rc::new(move |command, args, _| {
        if fail {
            return Box::pin(async { Err(std::io::Error::other("acquire failed")) });
        }
        if command == "npm" && args.first().is_some_and(|s| s == "install") && args.len() > 1 {
            let global = args[1] == "-g";
            let spec = &args[if global { 2 } else { 1 }];
            let name = spec.trim().split('@').next().unwrap();
            let root = if global {
                format!("{base}/global_node_modules")
            } else {
                format!("{base}/project/.maestro/npm/node_modules")
            };
            package(
                &Path::new(&root).join(name),
                &json!({"extensions":["entry.ts"]}),
            );
        } else if command == "git" && args.first().is_some_and(|s| s == "clone") {
            package(
                Path::new(args.last().unwrap()),
                &json!({"extensions":["entry.ts"]}),
            );
        }
        Box::pin(async { Ok(Some(0)) })
    }));
}

/// Builds an operation-local callback whose future shares the caller lifetime.
fn missing_callback<'a>(
    callbacks: &'a std::cell::RefCell<Vec<String>>,
    settings: &'a Rc<std::cell::RefCell<SettingsManager>>,
    action: Option<&'a str>,
    edit: Option<&'a Value>,
) -> impl Fn(&str) -> maestro_packages::PackageFuture<'a, maestro_packages::MissingSourceAction>
+ Copy
+ 'a {
    move |source| {
        callbacks.borrow_mut().push(source.to_owned());
        if let Some(edit) = edit {
            settings.borrow_mut().set_project_packages(Vec::new());
            settings.borrow_mut().set_project_extension_paths(
                edit["project"]["extensions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .cloned()
                    .map(maestro_settings::SettingsListEntry::Unknown)
                    .collect(),
            );
        }
        Box::pin(async move {
            match action {
                Some("skip") => Ok(maestro_packages::MissingSourceAction::Skip),
                Some("error") => Ok(maestro_packages::MissingSourceAction::Error),
                Some("throw") => Err(std::io::Error::other("callback failed")),
                _ => Ok(maestro_packages::MissingSourceAction::Install),
            }
        })
    }
}

/// Blocks only the first admitted source on an explicit completion signal.
fn gated_callback<'a>(
    callbacks: &'a std::cell::RefCell<Vec<String>>,
    receiver: &'a std::cell::RefCell<Option<tokio::sync::oneshot::Receiver<()>>>,
) -> impl Fn(&str) -> maestro_packages::PackageFuture<'a, maestro_packages::MissingSourceAction>
+ Copy
+ 'a {
    move |source| {
        callbacks.borrow_mut().push(source.to_owned());
        let receiver = receiver.borrow_mut().take();
        Box::pin(async move {
            if let Some(receiver) = receiver {
                receiver.await.unwrap();
            }
            Ok(maestro_packages::MissingSourceAction::Install)
        })
    }
}
/// Demonstrates a non-static callback capture through a completed configured operation.
pub fn borrowed_callback() {
    let tree = scratch::Scratch::new().unwrap();
    let fixture = setup(
        &Recipe {
            global: json!({"packages":["npm:missing"]}),
            ..Recipe::default()
        },
        tree.path().to_path_buf(),
    )
    .unwrap();
    let manager = &fixture.manager;
    let settings = &fixture.settings;
    let effects = &fixture.effects;
    let callbacks = std::cell::RefCell::new(Vec::new());
    {
        let callback = missing_callback(&callbacks, settings, Some("skip"), None);
        resolution_support::run(async { manager.resolve(Some(&callback)).await.map(drop) })
            .unwrap();
    }
    assert_eq!(*callbacks.borrow(), vec!["npm:missing"]);
    callbacks.borrow_mut().clear();
    resolution_support::run(async { manager.resolve(None).await.map(drop) }).unwrap();
    assert!(callbacks.borrow().is_empty());
    assert!(
        effects
            .calls
            .borrow()
            .iter()
            .any(|(_, args)| args.first().is_some_and(|arg| arg == "install"))
    );
}
