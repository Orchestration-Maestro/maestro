//! Component-level tests: a test-only host instantiates the built author component and the
//! same extension runs under the controlled adapter.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod author;
mod build;
mod component_driver;
#[allow(
    dead_code,
    reason = "the integration targets use the rest of the adapter"
)]
mod controlled;
mod controlled_driver;
#[macro_use]
mod fixtures;
#[macro_use]
mod event_fixtures;
#[allow(dead_code, reason = "each test target builds the fixtures it needs")]
mod guest_family;
pub(crate) mod host;
#[allow(dead_code, reason = "each test target builds the fixtures it needs")]
mod host_family;
mod join;
mod scenario;

use std::future::Future;

use scenario::EXPECTED;

/// Runs a future to completion on a current-thread runtime.
fn block_on<T>(future: impl Future<Output = T>) -> T {
    match tokio::runtime::Builder::new_current_thread().build() {
        Ok(runtime) => runtime.block_on(future),
        Err(error) => panic!("cannot build a runtime: {error}"),
    }
}

/// Runs the scenario through the built component and through the controlled adapter and
/// returns both transcripts.
fn both_transcripts() -> Result<(Vec<String>, Vec<String>), String> {
    let path = build::author_component()?.to_path_buf();
    let component = block_on(scenario::run(&mut component_driver::ComponentDriver::new(
        path,
    )))?;
    let controlled = block_on(scenario::run(
        &mut controlled_driver::ControlledDriver::new(),
    ))?;
    Ok((component, controlled))
}

#[test]
fn maestro_component_registers_invokes_and_releases() -> Result<(), String> {
    let (component, controlled) = both_transcripts()?;
    assert_eq!(component, EXPECTED);
    assert_eq!(controlled, EXPECTED);
    Ok(())
}

/// Where the component reports its release of a callback: the host sees the owner handles
/// dropped, in the order the extension released them.
#[test]
fn maestro_callbacks_release_after_reentry_and_failed_registration() -> Result<(), String> {
    let path = build::author_component()?.to_path_buf();
    let mut driver = component_driver::ComponentDriver::new(path);
    let transcript = block_on(scenario::run(&mut driver))?;
    let position = |line: &str| transcript.iter().position(|seen| seen == line);
    assert!(
        position(r#"entry released "rejected-handler""#)
            < position(r#"entry rejection "registration rejected: rejected""#),
        "a rejected registration drops its closure before the failure is reported"
    );
    assert!(
        position(r#"entry released "reentrant-guard""#) < position("register event late"),
        "a destructor registered a handler while its own entry was being released, without a conflict"
    );
    let dropped = driver
        .dropped_identities()
        .map_err(|error| error.to_string())?;
    assert_eq!(
        dropped,
        [7, 9, 1, 2, 3, 4, 5, 6, 8],
        "the rejected identity drops at once, the continuation when its operation ends, and the \
         registered callbacks follow in registration order"
    );
    let missing = block_on(driver.invoke_unknown_callbacks()).map_err(|error| error.to_string())?;
    assert_eq!(
        missing,
        [
            "no callback registered for identity 4242".to_owned(),
            "no pending continuation for identity 4242".to_owned(),
        ],
        "an invalid callback gets the adapter's exact diagnostic"
    );
    Ok(())
}

/// The functions of one interface of a component type with whether each is asynchronous.
fn functions(
    engine: &wasmtime::Engine,
    items: impl Iterator<Item = (String, wasmtime::component::types::ComponentItem)>,
    interface: &str,
) -> Result<Vec<(String, bool)>, String> {
    use wasmtime::component::types::ComponentItem;
    let instance = items
        .into_iter()
        .find_map(|(name, item)| match item {
            ComponentItem::ComponentInstance(instance) if name == interface => Some(instance),
            _ => None,
        })
        .ok_or_else(|| format!("the component has no {interface}"))?;
    Ok(instance
        .exports(engine)
        .filter_map(|(name, extern_)| match extern_.ty {
            ComponentItem::ComponentFunc(function) => Some((name.to_owned(), function.async_())),
            _ => None,
        })
        .collect())
}

/// The sorted names of the synchronous or the asynchronous functions.
fn names(functions: &[(String, bool)], asynchronous: bool) -> Vec<&str> {
    let mut names: Vec<&str> = functions
        .iter()
        .filter(|(_, async_)| *async_ == asynchronous)
        .map(|(name, _)| name.as_str())
        .collect();
    names.sort_unstable();
    names
}

/// The events the host delivers, each through its own export.
const EVENTS: [&str; 30] = [
    "resources-discover",
    "session-start",
    "session-before-switch",
    "session-before-fork",
    "session-before-compact",
    "session-compact",
    "session-shutdown",
    "session-before-tree",
    "session-tree",
    "context",
    "before-provider-request",
    "after-provider-response",
    "before-agent-start",
    "agent-start",
    "agent-end",
    "turn-start",
    "turn-end",
    "message-start",
    "message-update",
    "message-end",
    "tool-execution-start",
    "tool-execution-update",
    "tool-execution-end",
    "model-select",
    "thinking-level-select",
    "user-bash",
    "input",
    "tool-call",
    "tool-result",
    "custom",
];

/// The sorted names of the exports that are native async.
fn asynchronous_exports() -> Vec<String> {
    let mut exports: Vec<String> = EVENTS
        .iter()
        .map(|event| format!("invoke-{event}"))
        .collect();
    exports.extend(
        [
            "start",
            "invoke-tool",
            "invoke-command",
            "invoke-completions",
            "invoke-shortcut",
            "invoke-setup",
            "invoke-with-session",
            "invoke-listener-tail",
        ]
        .map(str::to_owned),
    );
    exports.sort_unstable();
    exports
}

/// The interfaces and functions of the built component: its guest exports, its host imports
/// and the names of every other import.
struct Contract {
    /// Functions of the guest interface with whether each is async.
    exports: Vec<(String, bool)>,
    /// Functions of the host interface with whether each is async.
    imports: Vec<(String, bool)>,
    /// Names of the imported interfaces.
    interfaces: Vec<String>,
}

/// Reads the contract of the built component.
fn contract() -> Result<Contract, String> {
    let path = build::author_component()?;
    let mut config = wasmtime::Config::new();
    config.wasm_component_model_async(true);
    let engine = wasmtime::Engine::new(&config).map_err(|error| error.to_string())?;
    let component = wasmtime::component::Component::from_file(&engine, path)
        .map_err(|error| error.to_string())?;
    let kind = component.component_type();
    let exported = kind
        .exports(&engine)
        .map(|(name, found)| (name.to_owned(), found.ty));
    let imported = kind
        .imports(&engine)
        .map(|(name, found)| (name.to_owned(), found.ty));
    Ok(Contract {
        exports: functions(&engine, exported, "maestro:extension/guest@0.1.0")?,
        imports: functions(&engine, imported, "maestro:extension/host@0.1.0")?,
        interfaces: kind
            .imports(&engine)
            .map(|(name, _)| name.to_owned())
            .collect(),
    })
}

#[test]
fn maestro_component_contract_keeps_sync_and_async_calls() -> Result<(), String> {
    let contract = contract()?;
    assert_eq!(
        names(&contract.exports, true),
        asynchronous_exports(),
        "every event, tool, command and pending call is native async"
    );
    assert_eq!(
        names(&contract.exports, false),
        [
            "[method]component.handle-input",
            "[method]component.invalidate",
            "[method]component.render",
            "[method]component.wants-key-release",
            "invoke-compaction-complete",
            "invoke-compaction-error",
            "invoke-listener",
            "invoke-renderer",
            "prepare-arguments",
            "release",
        ],
        "preparation, rendering, listener prefixes, notifications and release stay synchronous"
    );
    assert_eq!(
        names(&contract.imports, true),
        [
            "[method]command-context.fork",
            "[method]command-context.navigate-tree",
            "[method]command-context.new-session",
            "[method]command-context.reload",
            "[method]command-context.switch-session",
            "[method]command-context.wait-for-idle",
            "[method]event-bus.emit",
            "[method]model-registry.get-api-key-and-headers",
            "[method]replaced-session-context.send-message",
            "[method]replaced-session-context.send-user-message",
            "set-model",
        ]
    );
    let polling = contract
        .imports
        .iter()
        .chain(&contract.exports)
        .any(|(name, _)| name.contains("poll"));
    assert!(!polling, "there is no polling function in the author world");
    let outside: Vec<&String> = contract
        .interfaces
        .iter()
        .filter(|name| !name.starts_with("maestro:extension/") && !name.starts_with("wasi:"))
        .collect();
    assert!(
        outside.is_empty(),
        "only the author world and WASI interfaces are imported: {outside:?}"
    );
    let wasi = contract
        .interfaces
        .iter()
        .filter(|name| name.starts_with("wasi:"));
    assert!(wasi.clone().count() > 0 && wasi.into_iter().all(|name| name.contains("@0.2.")));
    Ok(())
}
