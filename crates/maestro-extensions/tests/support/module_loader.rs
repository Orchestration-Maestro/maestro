//! Recorded directory trees and a controlled module loader for extension path loading.
#![cfg(test)]
#![cfg(unix)]
use crate::local_support::{LocalBus, local};
use maestro_extensions::{
    EventError, EventListener, ExtensionAPI, ExtensionFactory, LoadExtensionsResult, ModuleLoader,
    discover_and_load_extensions, load_extensions,
    types::{
        ExtensionFlagOptions, ExtensionShortcutOptions, FlagType, FlagValue,
        RegisteredCommandOptions,
    },
};
use maestro_models::Local;
use maestro_test_directory::Scratch;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, fs, os::unix::fs::PermissionsExt, rc::Rc};

/// The recorded cases with their trees, calls and expected outcomes.
const CASES: &str = include_str!("../fixtures/extension_path_loading.json");

/// Listener and relay observations in delivery order.
pub type Trace = Rc<RefCell<Vec<Value>>>;

/// One recorded case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The case name.
    id: String,
    /// Entries created under the case root, in order.
    tree: Vec<Entry>,
    /// Loading calls, each with its own bus.
    calls: Vec<Call>,
    /// Each call's projected outcome and the listener trace.
    expected: Value,
}

/// One tree entry: its kind, its path below the root and its content.
#[derive(Deserialize)]
struct Entry(Kind, String, #[serde(default)] Value);

/// What a tree entry creates.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Kind {
    /// A file with literal text.
    File,
    /// A file whose text names the root.
    Text,
    /// A file holding the JSON content.
    Json,
    /// A symbolic link to the root-relative target.
    Link,
    /// A directory.
    Dir,
    /// Octal permissions for an existing entry.
    Mode,
}

/// One loading call.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Call {
    /// Explicit loading or discovery.
    op: Op,
    /// Explicit paths.
    #[serde(default)]
    paths: Vec<String>,
    /// Configured discovery paths.
    #[serde(default)]
    configured: Vec<String>,
    /// The working directory.
    cwd: String,
    /// The agent directory for discovery.
    agent_dir: Option<String>,
    /// Whether the test listens on the bus before the call.
    bus: Bus,
    /// The channel the test listens on.
    observe: Option<String>,
}

/// The loading operation.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Op {
    /// Load the supplied paths.
    Load,
    /// Discover, then load.
    Discover,
}

/// The bus a call receives.
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Bus {
    /// A new bus nobody else listens on.
    Fresh,
    /// A new bus with the test listener subscribed.
    Supplied,
}

/// Replaces the root placeholder.
fn fill(root: &str, text: &str) -> String {
    text.replace("$ROOT", root)
}

/// The recorded case named `id`.
fn case(id: &str) -> Case {
    let mut cases: Vec<Case> = serde_json::from_str(CASES).unwrap();
    let index = cases.iter().position(|case| case.id == id);
    cases.swap_remove(index.unwrap_or_else(|| panic!("no recorded case {id}")))
}

/// Creates the tree; false when a privileged run still lists a mode-000 directory.
fn build(root: &str, tree: &[Entry]) -> bool {
    for Entry(kind, path, content) in tree {
        let path = format!("{root}/{path}");
        let text = || fill(root, content.as_str().unwrap());
        if !matches!(kind, Kind::Dir | Kind::Mode) {
            fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
        }
        match kind {
            Kind::Dir => fs::create_dir_all(&path).unwrap(),
            Kind::File => fs::write(&path, content.as_str().unwrap()).unwrap(),
            Kind::Text => fs::write(&path, text()).unwrap(),
            Kind::Json => fs::write(&path, fill(root, &content.to_string())).unwrap(),
            Kind::Link => std::os::unix::fs::symlink(text(), &path).unwrap(),
            Kind::Mode => {
                let mode = u32::from_str_radix(content.as_str().unwrap(), 8).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
                if fs::read_dir(&path).is_ok() {
                    return false;
                }
            }
        }
    }
    true
}

/// Gives every mode entry its permissions back so the root can be removed.
fn restore(root: &str, tree: &[Entry]) {
    for Entry(_, path, _) in tree.iter().filter(|entry| entry.0 == Kind::Mode) {
        let path = format!("{root}/{path}");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// Runs one case in `root` and compares its outcome with the recorded one.
fn run_case(root: &str, case: &Case) {
    fs::create_dir_all(format!("{root}/work")).unwrap();
    fs::create_dir_all(format!("{root}/home")).unwrap();
    if build(root, &case.tree) {
        local(async {
            let actual = run_calls(root, case).await;
            assert_eq!(actual, case.expected, "case {}", case.id);
        });
    }
    restore(root, &case.tree);
}

/// Runs the named cases, each in its own directory tree.
pub fn check(ids: &[&str]) {
    let scratch = Scratch::new().unwrap();
    for id in ids {
        run_case(&format!("{}/{id}", scratch.path().display()), &case(id));
    }
}

/// Runs the named cases, each in a cleared child whose `HOME` is `<case root>/home`.
pub fn check_with_home(test: &str, ids: &[&str]) {
    let parent = (!maestro_test_process::selection().is_child()).then(|| Scratch::new().unwrap());
    let base = parent.as_ref().map_or_else(
        || {
            let home = std::env::var("HOME").unwrap();
            let case_root = home.strip_suffix("/home").unwrap();
            case_root.rsplit_once('/').unwrap().0.to_owned()
        },
        |scratch| scratch.path().display().to_string(),
    );
    let home = |id: &&str| BTreeMap::from([("HOME", format!("{base}/{id}/home"))]);
    let selected = maestro_test_process::run_per_environment(test, ids.iter().copied(), home);
    for id in selected.unwrap().into_iter().flatten() {
        run_case(&format!("{base}/{id}"), &case(id));
    }
}

/// Performs each call with its own bus and projects the outcomes with root placeholders.
async fn run_calls(root: &str, case: &Case) -> Value {
    let trace = Trace::default();
    let mut outcomes = Vec::new();
    for call in &case.calls {
        let bus = LocalBus::new();
        if call.bus == Bus::Supplied {
            let channel = call.observe.as_deref().unwrap();
            drop(
                bus.controller
                    .on(channel, recorder("test".into(), channel, &trace)),
            );
        }
        let loader = module_loader(&trace);
        let cwd = fill(root, &call.cwd);
        let fill_all = |paths: &[String]| paths.iter().map(|p| fill(root, p)).collect::<Vec<_>>();
        let result = match call.op {
            Op::Load => {
                load_extensions(fill_all(&call.paths), &cwd, &bus.controller, &loader).await
            }
            Op::Discover => {
                let agent_dir = fill(root, call.agent_dir.as_deref().unwrap());
                let configured = fill_all(&call.configured);
                discover_and_load_extensions(
                    &configured,
                    &cwd,
                    &agent_dir,
                    &bus.controller,
                    &loader,
                )
                .await
            }
        };
        bus.settle().await;
        outcomes.push(result.map_or_else(|_| json!({"rejected": true}), |r| project(&r)));
    }
    let text = json!({"outcomes": outcomes, "received": *trace.borrow()}).to_string();
    serde_json::from_str(&text.replace(root, "$ROOT")).unwrap()
}

/// A listener that records each delivery on `channel` under the name `listener`.
fn recorder(listener: String, channel: &str, trace: &Trace) -> EventListener<Local> {
    let (trace, channel) = (trace.clone(), channel.to_owned());
    Rc::new(move |data, _| {
        let entry = json!({"listener": listener, "channel": channel, "data": *data});
        trace.borrow_mut().push(entry);
        Ok(None)
    })
}

/// The loaded records, failures and shared flag values.
fn project(result: &LoadExtensionsResult<Local>) -> Value {
    let extensions: Vec<Value> = result
        .extensions
        .iter()
        .map(|handle| {
            let e = handle.read().unwrap();
            json!({
                "path": e.path, "resolvedPath": e.resolved_path,
                "sourceInfo": *e.source_info.read().unwrap(),
                "commands": e.commands.keys().collect::<Vec<_>>(),
                "flags": e.flags.keys().collect::<Vec<_>>(),
                "tools": e.tools.keys().collect::<Vec<_>>(),
                "handlers": e.handlers.keys().collect::<Vec<_>>(),
                "messageRenderers": e.message_renderers.keys().collect::<Vec<_>>(),
                "shortcuts": e.shortcuts.keys().map(ToString::to_string).collect::<Vec<_>>(),
            })
        })
        .collect();
    let errors: Vec<Value> = (result.errors.iter())
        .map(|e| json!({"path": e.path, "error": e.error}))
        .collect();
    let flags: serde_json::Map<String, Value> = (result.runtime.flag_values.read().unwrap())
        .iter()
        .map(|(name, value)| match value {
            FlagValue::String(text) => (name.clone(), json!(text)),
            FlagValue::Boolean(flag) => (name.clone(), json!(flag)),
        })
        .collect();
    json!({"extensions": extensions, "errors": errors, "flagValues": flags})
}

/// A loader that reads `;`-separated directives from the resolved file.
pub fn module_loader(trace: &Trace) -> ModuleLoader<Local> {
    let trace = trace.clone();
    Rc::new(move |resolved: &str| Box::pin(std::future::ready(module(resolved, &trace))))
}

/// Missing files fail; `none` exports nothing, `fail-load` fails, anything else is a factory.
fn module(resolved: &str, trace: &Trace) -> Result<Option<ExtensionFactory<Local>>, EventError> {
    if !std::path::Path::new(resolved).is_file() {
        return Err(format!("module not found: {resolved}").into());
    }
    let directives: Vec<(String, String)> = (fs::read_to_string(resolved)?.split(';'))
        .filter(|directive| !directive.is_empty())
        .map(|directive| {
            let (kind, argument) = directive.split_once(':').unwrap_or((directive, ""));
            (kind.to_owned(), argument.to_owned())
        })
        .collect();
    for (kind, argument) in &directives {
        match kind.as_str() {
            "none" => return Ok(None),
            "fail-load" => return Err(argument.clone().into()),
            _ => {}
        }
    }
    let (trace, resolved) = (trace.clone(), resolved.to_owned());
    Ok(Some(Rc::new(move |api| {
        Box::pin(apply(
            api,
            directives.clone(),
            resolved.clone(),
            trace.clone(),
        ))
    })))
}

/// Applies each directive through the registration facade, in order.
async fn apply(
    api: ExtensionAPI<Local>,
    directives: Vec<(String, String)>,
    resolved: String,
    trace: Trace,
) -> Result<(), EventError> {
    for (kind, argument) in directives {
        match kind.as_str() {
            "listen" => drop(
                api.events
                    .on(&argument, recorder(resolved.clone(), &argument, &trace)),
            ),
            "relay" => relay(&api, &argument, &trace),
            "emit" => api.events.emit(&argument, json!({"from": resolved})).await,
            "fail-init" => return Err(argument.into()),
            _ => register(&api, &kind, argument)?,
        }
    }
    Ok(())
}

/// Registers one command, flag, tool, renderer, handler or shortcut.
fn register(api: &ExtensionAPI<Local>, kind: &str, argument: String) -> Result<(), EventError> {
    match kind {
        "command" => api.register_command(
            argument,
            RegisteredCommandOptions {
                description: None,
                get_argument_completions: None,
                handler: Rc::new(|_, _| Box::pin(async { Ok(()) })),
            },
        ),
        "flag" => {
            let (name, value) = argument.split_once('=').unwrap();
            let options = ExtensionFlagOptions {
                description: None,
                r#type: FlagType::String,
                default: Some(FlagValue::String(value.to_owned())),
            };
            api.register_flag(name.to_owned(), options)
        }
        "tool" => api.register_tool(crate::tool_support::definition(&argument, &argument)),
        "renderer" => {
            api.register_message_renderer::<Value>(argument, Rc::new(|_, _, _, _| Ok(None)))
        }
        "handler" => api.on::<Value, Value>(argument, Rc::new(|_, _| Box::pin(async { Ok(None) }))),
        "shortcut" => api.register_shortcut(
            argument.as_str().into(),
            ExtensionShortcutOptions {
                description: None,
                handler: Rc::new(|_| Box::pin(async { Ok(()) })),
            },
        ),
        other => Err(format!("unknown directive: {other}").into()),
    }
}

/// On `from>to`, emits `to` from inside the `from` callback, then records its return.
fn relay(api: &ExtensionAPI<Local>, route: &str, trace: &Trace) {
    let (from, to) = route.split_once('>').unwrap();
    let (trace, to) = (trace.clone(), to.to_owned());
    drop(api.events.on(
        from,
        Rc::new(move |_, emitter| {
            emitter.emit(&to, json!({}));
            trace.borrow_mut().push(json!("relay returned"));
            Ok(None)
        }),
    ));
}
