//! Built components load through the extension domain's path, discovery and registration rules.
#![cfg(test)]
use super::components::{TracedBus, hooks, loader, local, place};
use maestro_extensions::{
    ExtensionActions, ExtensionRunner, LoadExtensionsResult, Local, ModuleLoader,
    discover_and_load_extensions, load_extensions,
    types::{ExtensionFactory, ExtensionHandle, FlagValue},
};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{cell::RefCell, path::Path, rc::Rc, sync::RwLock};

/// Setup variable naming the component whose `location` flag defaults to its own path.
const LOCATION: &str = "MAESTRO_LOCATION_COMPONENT";

/// Setup variable naming the component that installs described hooks and registrations.
const HOOKS: &str = "MAESTRO_HOOK_COMPONENT";

/// The host spelling of a path.
fn spelled(path: &Path) -> String {
    path.to_str().unwrap().to_owned()
}

/// The default of an extension's `location` flag, which names the artifact it was loaded from.
fn location(owner: &ExtensionHandle<Local>) -> Option<FlagValue> {
    let flag = owner.read().unwrap().flags["location"].clone();
    flag.read().unwrap().default.clone()
}

/// A runner over the loaded extensions and their shared runtime.
fn runner(loaded: &LoadExtensionsResult<Local>, cwd: &str) -> ExtensionRunner<Local> {
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(loaded.extensions.clone())),
        loaded.runtime.clone(),
        super::session::runner_session(cwd).0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    super::support::bind_prompt(&runner, "loading prompt");
    runner
}

/// A module loader that records each module path and when its initialization finished.
fn recording(steps: &Rc<RefCell<Vec<String>>>) -> ModuleLoader<Local> {
    let steps = steps.clone();
    Rc::new(move |path: &str| {
        steps.borrow_mut().push(format!("load {path}"));
        let (factory, steps, path) = (
            crate::artifact(path.to_owned()),
            steps.clone(),
            path.to_owned(),
        );
        let observed: ExtensionFactory<Local> = Rc::new(move |api| {
            let (factory, steps, path) = (factory.clone(), steps.clone(), path.clone());
            Box::pin(async move {
                factory(api).await?;
                steps.borrow_mut().push(format!("ready {path}"));
                Ok(())
            })
        });
        Box::pin(std::future::ready(Ok(Some(observed))))
    })
}

/// Records keep the supplied spelling and share one runtime; registrations name their owner.
fn records(loaded: &LoadExtensionsResult<Local>, supplied: [&str; 3], resolved: [&String; 3]) {
    for ((owner, supplied), resolved) in loaded.extensions.iter().zip(supplied).zip(resolved) {
        let owner = owner.read().unwrap();
        assert_eq!(owner.path, supplied);
        assert_eq!(&owner.resolved_path, resolved);
        assert_eq!(owner.source_info.read().unwrap().path, supplied);
    }
    let [first, second, third] = [0, 1, 2].map(|index| &loaded.extensions[index]);
    let artifact = |path: &String| Some(FlagValue::String(path.clone()));
    assert_eq!(location(first), artifact(resolved[0]));
    assert_eq!(location(third), artifact(resolved[2]));
    let flag = first.read().unwrap().flags["location"].clone();
    assert_eq!(flag.read().unwrap().extension_path, supplied[0]);
    // One runtime: the first default is the value both location flags share.
    let values = loaded.runtime.flag_values.read().unwrap();
    assert_eq!(values["location"], FlagValue::String(resolved[0].clone()));
    let owner = second.read().unwrap();
    let command = owner.commands["hooks"].read().unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &command.source_info,
        &owner.source_info
    ));
}

#[test]
fn component_paths_use_the_domain_loader() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    let cwd = spelled(root);
    let decoy = root.join(".maestro").join("extensions").join("decoy.wasm");
    place(HOOKS, &decoy);
    for (component, directory) in [(LOCATION, "first"), (HOOKS, "second"), (LOCATION, "third")] {
        place(component, &root.join(directory).join("index.wasm"));
    }
    let [first, second, third] =
        ["first", "second", "third"].map(|name| spelled(&root.join(name).join("index.wasm")));
    local(async {
        let bus = TracedBus::new();
        let discovered = discover_and_load_extensions(
            &[],
            &cwd,
            &spelled(&root.join("agent")),
            &bus.controller,
            &loader(),
        )
        .await
        .unwrap();
        assert!(discovered.errors.is_empty());
        assert_eq!(discovered.extensions.len(), 1);
        assert_eq!(
            discovered.extensions[0].read().unwrap().resolved_path,
            spelled(&decoy)
        );
        let empty = load_extensions(Vec::new(), &cwd, &bus.controller, &loader())
            .await
            .unwrap();
        assert!(empty.extensions.is_empty() && empty.errors.is_empty());
        let supplied = [
            "first/../first/index.wasm",
            "./second/index.wasm",
            "third/index.wasm",
        ];
        let steps = Rc::default();
        let loaded = load_extensions(
            supplied.map(str::to_owned).to_vec(),
            &cwd,
            &bus.controller,
            &recording(&steps),
        )
        .await
        .unwrap();
        assert!(loaded.errors.is_empty());
        let expected: Vec<_> = [&first, &second, &third]
            .iter()
            .flat_map(|path| [format!("load {path}"), format!("ready {path}")])
            .collect();
        assert_eq!(*steps.borrow(), expected);
        records(&loaded, supplied, [&first, &second, &third]);
        let runner = runner(&loaded, &cwd);
        hooks(&loaded.extensions[1], &runner, &json!("count"))
            .await
            .unwrap();
        assert_eq!(bus.take(), [json!({ "count": 1 })]);
    });
}

/// Writes `document` as the manifest of `directory`.
fn manifest(directory: &Path, document: &Value) {
    std::fs::create_dir_all(directory).unwrap();
    std::fs::write(directory.join("package.json"), document.to_string()).unwrap();
}

/// Builds project, user and configured trees; returns the selected paths in load order, the
/// configured paths and the source-language file that discovery selects but cannot load.
fn discovery_tree(root: &Path) -> (Vec<String>, [String; 4], String) {
    let project = root.join("project");
    let extensions = project.join(".maestro").join("extensions");
    let agent = root.join("agent");
    let mut expected = Vec::new();
    let mut component = |path: std::path::PathBuf, selected: bool| {
        place(LOCATION, &path);
        if selected {
            expected.push(spelled(&path));
        }
    };
    component(extensions.join("a-direct.wasm"), true);
    component(extensions.join("b-indexed").join("index.wasm"), true);
    let declared = extensions.join("c-manifest");
    manifest(
        &declared,
        &json!({"maestro":{"extensions":["./second.wasm","./absent.wasm","./first.wasm"]}}),
    );
    component(declared.join("second.wasm"), true);
    component(declared.join("first.wasm"), true);
    component(declared.join("index.wasm"), false);
    for (name, document) in [
        ("d-without-field", json!({"name":"plain"})),
        ("e-gone", json!({"maestro":{"extensions":["./gone.wasm"]}})),
    ] {
        manifest(&extensions.join(name), &document);
        component(extensions.join(name).join("index.wasm"), true);
    }
    std::fs::create_dir_all(extensions.join("f-broken")).unwrap();
    std::fs::write(extensions.join("f-broken").join("package.json"), "{").unwrap();
    component(extensions.join("f-broken").join("index.wasm"), true);
    component(extensions.join("g-helpers").join("util.wasm"), false);
    component(
        extensions.join("h-deep").join("nested").join("index.wasm"),
        false,
    );
    std::fs::write(extensions.join("notes.txt"), "not an extension").unwrap();
    std::fs::write(extensions.join("z-source.ts"), "export default () => {};").unwrap();
    let source = spelled(&extensions.join("z-source.ts"));
    component(agent.join("extensions").join("user.wasm"), true);
    component(root.join("extra").join("file.wasm"), true);
    manifest(
        &root.join("extra-dir"),
        &json!({"maestro":{"extensions":["./x.wasm"]}}),
    );
    component(root.join("extra-dir").join("x.wasm"), true);
    component(root.join("extra-scan").join("scan.wasm"), true);
    let configured = [
        spelled(&root.join("extra").join("file.wasm")),
        spelled(&root.join("extra-dir")),
        spelled(&root.join("extra-scan")),
        spelled(&extensions.join("a-direct.wasm")),
    ];
    (expected, configured, source)
}

#[test]
fn component_discovery_uses_existing_entry_rules() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    let project = root.join("project");
    let agent = root.join("agent");
    let (expected, configured, source) = discovery_tree(root);
    local(async {
        let bus = TracedBus::new();
        let loaded = discover_and_load_extensions(
            &configured,
            &spelled(&project),
            &spelled(&agent),
            &bus.controller,
            &loader(),
        )
        .await
        .unwrap();
        let paths: Vec<_> = loaded
            .extensions
            .iter()
            .map(|owner| owner.read().unwrap().path.clone())
            .collect();
        assert_eq!(paths, expected);
        let locations: Vec<_> = loaded.extensions.iter().map(location).collect();
        assert_eq!(
            locations,
            expected
                .iter()
                .map(|path| Some(FlagValue::String(path.clone())))
                .collect::<Vec<_>>()
        );
        assert_eq!(loaded.errors.len(), 1);
        assert_eq!(loaded.errors[0].path, source);
        assert!(
            loaded.errors[0]
                .error
                .starts_with("Failed to load extension: "),
            "{}",
            loaded.errors[0].error
        );
        let invalid = root.join("invalid");
        manifest(&invalid, &json!({"maestro":{"extensions":[1]}}));
        let failure = discover_and_load_extensions(
            &[spelled(&invalid)],
            &spelled(&project),
            &spelled(&agent),
            &bus.controller,
            &loader(),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(failure.kind(), std::io::ErrorKind::InvalidData);
    });
}

/// Unloadable artifacts, authored initialization failures and a trap, then a healthy component.
fn failure_inputs(root: &Path) -> Vec<String> {
    let write = |name: &str, bytes: &[u8]| std::fs::write(root.join(name), bytes).unwrap();
    write("not-component.wasm", b"not a component");
    write("empty-component.wasm", b"\0asm\x0d\0\x01\0");
    write("core-module.wasm", b"\0asm\x01\0\0\0");
    let directories = [
        "fail-empty",
        "fail-lines",
        "fail-unicode",
        "fail-trap",
        "healthy",
    ];
    for directory in directories {
        place(HOOKS, &root.join(directory).join("index.wasm"));
    }
    let mut supplied = vec![
        spelled(&root.join("missing.wasm")),
        spelled(&root.join("not-component.wasm")),
        spelled(&root.join("empty-component.wasm")),
        spelled(&root.join("core-module.wasm")),
    ];
    supplied.extend(directories.map(|directory| spelled(&root.join(directory).join("index.wasm"))));
    supplied
}

#[test]
fn component_failures_are_attributed_and_do_not_stop_loading() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    let supplied = failure_inputs(root);
    local(async {
        let bus = TracedBus::new();
        let loaded = load_extensions(supplied.clone(), &spelled(root), &bus.controller, &loader())
            .await
            .unwrap();
        let failed: Vec<_> = loaded
            .errors
            .iter()
            .map(|error| error.path.clone())
            .collect();
        assert_eq!(failed, supplied[..8]);
        let errors: Vec<_> = loaded
            .errors
            .iter()
            .map(|error| error.error.as_str())
            .collect();
        for (error, path) in errors[..4].iter().zip(&supplied) {
            let fault = format!("Failed to load extension: component fault {path}: ");
            assert!(error.starts_with(&fault), "{error}");
        }
        assert_eq!(
            errors[4..7],
            [
                "Failed to load extension: ",
                "Failed to load extension: first line\nsecond line",
                "Failed to load extension: échec Ω ✓",
            ]
        );
        let trap = format!(
            "Failed to load extension: component fault {}: ",
            supplied[7]
        );
        assert!(errors[7].starts_with(&trap), "{}", errors[7]);
        assert!(errors[7].contains("unreachable"), "{}", errors[7]);
        assert_eq!(loaded.extensions.len(), 1);
        let runner = runner(&loaded, &spelled(root));
        hooks(&loaded.extensions[0], &runner, &json!("count"))
            .await
            .unwrap();
        assert_eq!(bus.take(), [json!({ "count": 1 })]);
    });
}

#[test]
fn reloading_components_uses_fresh_state_and_current_bytes() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    let artifact = root.join("live").join("index.wasm");
    place(HOOKS, &artifact);
    let path = spelled(&artifact);
    local(async {
        let bus = TracedBus::new();
        let loader = loader();
        let load = || load_extensions(vec![path.clone()], "/unrelated", &bus.controller, &loader);
        let earlier = load().await.unwrap();
        let earlier_runner = runner(&earlier, "/unrelated");
        for _ in 0..2 {
            hooks(&earlier.extensions[0], &earlier_runner, &json!("count"))
                .await
                .unwrap();
        }
        let later = load().await.unwrap();
        let later_runner = runner(&later, "/unrelated");
        hooks(&later.extensions[0], &later_runner, &json!("count"))
            .await
            .unwrap();
        hooks(&earlier.extensions[0], &earlier_runner, &json!("count"))
            .await
            .unwrap();
        assert_eq!(
            bus.take(),
            [1, 2, 1, 3].map(|count| json!({ "count": count }))
        );
        place(LOCATION, &artifact);
        let replaced = load().await.unwrap();
        let owner = &replaced.extensions[0];
        assert_eq!(location(owner), Some(FlagValue::String(path.clone())));
        assert!(owner.read().unwrap().commands.is_empty());
        assert!(earlier.extensions[0].read().unwrap().flags.is_empty());
    });
}

/// Registrations of one labelled set through a loaded hook component.
fn registrations(label: &str, names: &Value) -> Value {
    let mut request = names.clone();
    request["label"] = json!(label);
    json!({ "registrations": request })
}

#[test]
fn component_registration_keeps_domain_replacement_rules() {
    let scratch = Scratch::new().unwrap();
    let root = scratch.path();
    let [first, second] = ["first", "second"].map(|name| root.join(name).join("index.wasm"));
    place(HOOKS, &first);
    place(HOOKS, &second);
    local(async {
        let bus = TracedBus::new();
        let loaded = load_extensions(
            vec![spelled(&first), spelled(&second)],
            "/work",
            &bus.controller,
            &loader(),
        )
        .await
        .unwrap();
        let runner = Rc::new(runner(&loaded, "/work"));
        let [one, two] = [0, 1].map(|index| loaded.extensions[index].clone());
        let shared = json!({"commands":["solo","dup"],"tools":["shared"],"renderers":["note"],"shortcuts":["ctrl+shift+x"],"flags":["mode"],"events":["agent_start"]});
        hooks(&one, &runner, &registrations("first", &shared))
            .await
            .unwrap();
        let mut others = shared.clone();
        others["commands"] = json!(["dup"]);
        others["tools"] = json!(["shared", "own"]);
        hooks(&two, &runner, &registrations("second", &others))
            .await
            .unwrap();
        hooks(
            &one,
            &runner,
            &registrations("first again", &json!({"commands":["solo"]})),
        )
        .await
        .unwrap();
        assert!(bus.take().is_empty());
        commands(&runner, &bus).await;
        tools(&runner, &bus).await;
        renderers_shortcuts_and_flags(&runner, &bus, [&first, &second]).await;
        refreshes_bound_tools(&runner, &two).await;
    });
}

/// Same-owner replacement keeps one command; a name shared across owners gets numbered names.
async fn commands(runner: &ExtensionRunner<Local>, bus: &TracedBus) {
    let names: Vec<_> = runner
        .get_registered_commands()
        .into_iter()
        .map(|command| command.invocation_name)
        .collect();
    assert_eq!(names, ["hooks:1", "solo", "dup:1", "hooks:2", "dup:2"]);
    for name in ["solo", "dup:1", "dup:2"] {
        let command = runner.get_command(name).unwrap().command;
        (command.handler)(name.into(), runner.create_command_context())
            .await
            .unwrap();
    }
    assert_eq!(
        bus.take(),
        [
            json!({"command":"first again","args":"solo"}),
            json!({"command":"first","args":"dup:1"}),
            json!({"command":"second","args":"dup:2"}),
        ]
    );
    let mut event = maestro_extensions::AgentStartEvent {};
    assert!(runner.emit(&mut event).await.unwrap().is_none());
    assert_eq!(
        bus.take(),
        [json!({"event":"first"}), json!({"event":"second"})]
    );
}

/// The first owner of a tool name supplies its preparation, progress and result.
async fn tools(runner: &Rc<ExtensionRunner<Local>>, bus: &TracedBus) {
    let registered = runner.get_all_registered_tools();
    let names: Vec<_> = registered
        .iter()
        .map(|tool| tool.definition.metadata().definition.name)
        .collect();
    assert_eq!(names, ["shared", "own"]);
    let metadata = registered[0].definition.metadata();
    assert_eq!(metadata.definition.description, "first");
    assert_eq!(
        metadata.definition.parameters.as_json(),
        &json!({"type":"object","properties":{"n":{"type":"number"}}})
    );
    let wrapped = maestro_extensions::wrap_registered_tool(&registered[0], runner.clone());
    let prepared = wrapped.prepare_arguments.as_ref().unwrap()(&mut json!({"n":2.5})).unwrap();
    assert_eq!(prepared, json!({"prepared":"first","args":{"n":2.5}}));
    let updates = Rc::new(RefCell::new(Vec::new()));
    let received = updates.clone();
    let result = (wrapped.execute)(
        "call".into(),
        prepared.clone(),
        None,
        Some(Rc::new(move |update| received.borrow_mut().push(update))),
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(result.content).unwrap(),
        json!([{"type":"text","text":"first"}])
    );
    assert_eq!(result.details, Some(json!({"id":"call","params":prepared})));
    let updates: Vec<_> = updates
        .borrow()
        .iter()
        .map(|update| serde_json::to_value(update).unwrap())
        .collect();
    assert_eq!(
        updates,
        [json!({"content":[],"details":{"progress":"first"}})]
    );
    assert!(bus.take().is_empty());
}

/// Renderers and flags keep their first owner; a shared shortcut keeps its last owner and warns.
async fn renderers_shortcuts_and_flags(
    runner: &ExtensionRunner<Local>,
    bus: &TracedBus,
    owners: [&Path; 2],
) {
    let renderer = runner.get_message_renderer::<Value>("note").unwrap();
    let message = serde_json::from_value(json!({"role":"custom","customType":"note","content":"text","display":true,"timestamp":1.5})).unwrap();
    let view = bus.controller.for_extension().invoke_callback(|emitter| {
        renderer(
            Rc::new(message),
            maestro_extensions::MessageRenderOptions { expanded: false },
            maestro_theme::LiveTheme::default(),
            emitter,
        )
    });
    assert!(view.unwrap().is_none());
    assert_eq!(bus.take(), [json!({"renderer":"first","message":"note"})]);
    let shortcuts = runner.get_shortcuts(&Vec::default());
    let handler = shortcuts["ctrl+shift+x"].read().unwrap().handler.clone();
    handler(runner.create_context()).await.unwrap();
    assert_eq!(bus.take(), [json!({"shortcut":"second"})]);
    let [first, second] = owners.map(spelled);
    let diagnostics: Vec<_> = runner
        .get_shortcut_diagnostics()
        .read()
        .unwrap()
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect();
    assert_eq!(
        diagnostics,
        [format!(
            "Extension shortcut conflict: 'ctrl+shift+x' registered by both {first} and {second}. Using {second}."
        )]
    );
    assert_eq!(
        runner.get_flags()["mode"].read().unwrap().extension_path,
        first
    );
    assert_eq!(
        runner.get_flag_values()["mode"],
        FlagValue::String("first".into())
    );
}

/// A tool registered after binding refreshes the bound tool set once.
async fn refreshes_bound_tools(
    runner: &Rc<ExtensionRunner<Local>>,
    owner: &ExtensionHandle<Local>,
) {
    let refreshed = Rc::new(std::cell::Cell::new(0));
    let counted = refreshed.clone();
    let refreshed_tools = Rc::new(RefCell::new(Vec::new()));
    let captured = refreshed_tools.clone();
    let active = Rc::downgrade(runner);
    runner
        .bind_core(
            ExtensionActions {
                refresh_tools: Rc::new(move || {
                    counted.set(counted.get() + 1);
                    captured.replace(active.upgrade().unwrap().get_all_registered_tools());
                    Ok(())
                }),
                ..<Local as maestro_extensions::ExtensionExecution>::unbound_actions()
            },
            maestro_extensions::ExtensionContextActions::default(),
            maestro_extensions::types::ProviderActions::default(),
        )
        .unwrap();
    hooks(
        owner,
        runner,
        &registrations("late", &json!({"tools":["late"]})),
    )
    .await
    .unwrap();
    assert_eq!(refreshed.get(), 1);
    let wrapped = {
        let registered = refreshed_tools.borrow();
        let names: Vec<_> = registered
            .iter()
            .map(|tool| tool.definition.metadata().definition.name)
            .collect();
        assert_eq!(names, ["shared", "own", "late"]);
        maestro_extensions::wrap_registered_tool(&registered[2], runner.clone())
    };
    let result = (wrapped.execute)(
        "late-call".into(),
        json!({"n":3}),
        None,
        Some(Rc::new(|_| {})),
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(result.content).unwrap(),
        json!([{"type":"text","text":"late"}])
    );
    assert_eq!(
        result.details,
        Some(json!({"id":"late-call","params":{"n":3}}))
    );
}
