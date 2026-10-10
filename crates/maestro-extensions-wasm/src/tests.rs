//! Component-level tests: a test-only host instantiates the built author component, and an
//! in-process host runs the component adapter's own functions natively; both are driven
//! through the same scenario with the same extension.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// Starts the extension on the built component and on the controlled adapter in turn and runs
/// the async test body `$body(&mut driver)` against each.
macro_rules! on_both_adapters {
    ($body:ident) => {{
        let component = $crate::tests::path_of_component()?;
        let mut built = $crate::tests::ComponentDriver::new(component);
        $crate::tests::block_on(async {
            $crate::tests::scenario::Driver::start(&mut built).await?;
            $body(&mut built).await
        })?;
        let mut controlled = $crate::tests::ControlledDriver::new();
        $crate::tests::block_on(async {
            $crate::tests::scenario::Driver::start(&mut controlled).await?;
            $body(&mut controlled).await
        })
    }};
}

mod agent_events;
mod application_messages;
mod author;
mod build;
mod component_driver;
mod controlled;
mod controlled_driver;
mod documents;
mod event_transport;
mod event_values;
mod finite_records;
#[macro_use]
mod fixtures;
mod guest_family;
pub(crate) mod host;
mod host_family;
mod join;
mod observed;
mod scenario;
mod selection_events;
mod session_corpus;
mod session_events;
mod session_records;
mod session_summary;
mod shared_records;
mod stream_events;
mod tool_callbacks;
mod tool_events;

use std::future::Future;

use wasmtime::component::ResourceType;
use wasmtime::component::types::{ComponentInstance, ComponentItem, Type};

use component_driver::ComponentDriver;
use controlled_driver::ControlledDriver;
use scenario::{Decision, Driver, EXPECTED};

/// Runs a future to completion on a current-thread runtime.
fn block_on<T>(future: impl Future<Output = T>) -> T {
    match tokio::runtime::Builder::new_current_thread().build() {
        Ok(runtime) => runtime.block_on(future),
        Err(error) => panic!("cannot build a runtime: {error}"),
    }
}

/// Path of the built author component.
fn path_of_component() -> Result<std::path::PathBuf, String> {
    Ok(build::author_component()?.to_path_buf())
}

/// Runs the scenario through the built component and through the controlled adapter and
/// returns both transcripts.
fn both_transcripts() -> Result<(Vec<String>, Vec<String>), String> {
    let component = block_on(scenario::run(&mut ComponentDriver::new(
        path_of_component()?
    )))?;
    let controlled = block_on(scenario::run(&mut ControlledDriver::new()))?;
    Ok((component, controlled))
}

#[test]
fn maestro_component_registers_invokes_and_releases() -> Result<(), String> {
    let (component, controlled) = both_transcripts()?;
    assert_eq!(component, EXPECTED);
    assert_eq!(controlled, EXPECTED);
    Ok(())
}

/// Runs the command against a host that rejects its session operation, and returns what the
/// host observed.
async fn rejected_session_transcript(mut driver: impl Driver) -> Result<Vec<String>, String> {
    driver.start().await?;
    driver.reject_next_session();
    scenario::command(&mut driver, "go").await;
    Ok(driver.transcript())
}

/// Checks how an adapter releases what it holds: the host sees the owner handles dropped, in
/// the order the extension released them. Each check drives a fresh adapter from `adapter`.
async fn check_release<D: Driver>(adapter: impl Fn() -> D) -> Result<(), String> {
    let mut scenario_driver = adapter();
    let transcript = scenario::run(&mut scenario_driver).await?;
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
    assert_eq!(
        scenario_driver.dropped(),
        [4, 10, 1, 2, 3, 5, 6, 7, 8, 9, 11],
        "the rejected identity drops at once, the continuation when its operation ends, and the \
         registered callbacks follow in registration order, ending with the one a destructor \
         registered"
    );
    assert_eq!(
        scenario_driver.live(),
        0,
        "every context, signal and callback the host lent was dropped"
    );
    let rejected = rejected_session_transcript(adapter()).await?;
    let position = |line: &str| rejected.iter().position(|seen| seen == line);
    assert!(
        position("new-session start parent=parent") < position(r#"entry released "continuation""#)
            && position(r#"entry released "continuation""#)
                < position("command failed: session rejected"),
        "a continuation the host never ran is released when its operation ends: {rejected:?}"
    );
    let mut missing_driver = adapter();
    missing_driver.start().await?;
    let event = r#"{"type":"input","text":"x","source":"rpc"}"#;
    let unknown = missing_driver.unregistered()?;
    let unregistered = missing_driver
        .deliver(unknown, event, "/work", None)
        .await?;
    assert_eq!(
        unregistered.decision,
        Decision::Failed("no callback registered for identity 4242".to_owned()),
        "an invalid handler gets the adapter's exact diagnostic and its decoded event back"
    );
    assert_eq!(unregistered.event, Ok(Some(event.to_owned())));
    assert_eq!(
        missing_driver.invoke_unknown_continuation().await,
        "no pending continuation for identity 4242",
        "an invalid continuation gets the adapter's exact diagnostic"
    );
    Ok(())
}

#[test]
fn maestro_callbacks_release_after_reentry_and_failed_registration() -> Result<(), String> {
    let path = path_of_component()?;
    block_on(check_release(|| ComponentDriver::new(path.clone())))?;
    block_on(check_release(ControlledDriver::new))
}

/// The instance that provides `interface` among the `items` of a component type.
fn instance(
    items: impl Iterator<Item = (String, ComponentItem)>,
    interface: &str,
) -> Result<ComponentInstance, String> {
    items
        .into_iter()
        .find_map(|(name, item)| match item {
            ComponentItem::ComponentInstance(instance) if name == interface => Some(instance),
            _ => None,
        })
        .ok_or_else(|| format!("the component has no {interface}"))
}

/// The functions of an interface with whether each is asynchronous.
fn functions(engine: &wasmtime::Engine, interface: &ComponentInstance) -> Vec<(String, bool)> {
    interface
        .exports(engine)
        .filter_map(|(name, extern_)| match extern_.ty {
            ComponentItem::ComponentFunc(function) => Some((name.to_owned(), function.async_())),
            _ => None,
        })
        .collect()
}

/// The resource types an interface declares, by name.
fn resources(
    engine: &wasmtime::Engine,
    interface: &ComponentInstance,
) -> Vec<(String, ResourceType)> {
    interface
        .exports(engine)
        .filter_map(|(name, extern_)| match extern_.ty {
            ComponentItem::Resource(resource) => Some((name.to_owned(), resource)),
            _ => None,
        })
        .collect()
}

/// Spells a type as the interface file writes it; a resource takes the name of the host type it
/// is.
fn spell(ty: &Type, resources: &[(String, ResourceType)]) -> String {
    let named = |resource: &ResourceType| {
        let known = resources.iter().find(|(_, known)| known == resource);
        known
            .map_or("unknown", |(name, _)| name.as_str())
            .to_owned()
    };
    let optional = |ty: Option<Type>| ty.map_or_else(|| "_".to_owned(), |ty| spell(&ty, resources));
    match ty {
        Type::String => "string".to_owned(),
        Type::Own(resource) => format!("own<{}>", named(resource)),
        Type::Borrow(resource) => format!("borrow<{}>", named(resource)),
        Type::Option(option) => format!("option<{}>", spell(&option.ty(), resources)),
        Type::Result(result) => {
            format!(
                "result<{}, {}>",
                optional(result.ok()),
                optional(result.err())
            )
        }
        Type::Record(record) => {
            let fields = record
                .fields()
                .map(|field| format!("{}: {}", field.name, spell(&field.ty, resources)));
            format!("record {{{}}}", fields.collect::<Vec<_>>().join(", "))
        }
        Type::Variant(variant) => {
            let cases = variant.cases().map(|case| match case.ty {
                Some(ty) => format!("{}({})", case.name, spell(&ty, resources)),
                None => case.name.to_owned(),
            });
            format!("variant {{{}}}", cases.collect::<Vec<_>>().join(", "))
        }
        other => format!("{other:?}"),
    }
}

/// The parameters and results of the function `name` of an interface, spelled by [`spell`].
fn signature(
    engine: &wasmtime::Engine,
    interface: &ComponentInstance,
    name: &str,
    resources: &[(String, ResourceType)],
) -> Result<(Vec<String>, Vec<String>), String> {
    let Some(ComponentItem::ComponentFunc(function)) =
        interface.get_export(engine, name).map(|found| found.ty)
    else {
        return Err(format!("the interface has no function {name}"));
    };
    Ok((
        function
            .params()
            .map(|(name, ty)| format!("{name}: {}", spell(&ty, resources)))
            .collect(),
        function.results().map(|ty| spell(&ty, resources)).collect(),
    ))
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

/// The interfaces and functions of the built component: its guest exports, its host imports,
/// the signature of the event export and the names of every other import.
struct Contract {
    /// Functions of the guest interface with whether each is async.
    exports: Vec<(String, bool)>,
    /// Functions of the host interface with whether each is async.
    imports: Vec<(String, bool)>,
    /// Names of the imported interfaces.
    interfaces: Vec<String>,
    /// The parameters of `invoke-event`, each as `name: type`.
    event_params: Vec<String>,
    /// The results of `invoke-event`.
    event_results: Vec<String>,
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
    let guest = instance(exported, "maestro:extension/guest@0.1.0")?;
    let host = instance(imported, "maestro:extension/host@0.1.0")?;
    let (event_params, event_results) =
        signature(&engine, &guest, "invoke-event", &resources(&engine, &host))?;
    Ok(Contract {
        exports: functions(&engine, &guest),
        imports: functions(&engine, &host),
        interfaces: kind
            .imports(&engine)
            .map(|(name, _)| name.to_owned())
            .collect(),
        event_params,
        event_results,
    })
}

/// The event export takes the callback, the document and the typed capabilities, and answers
/// with the two independent parts or a refusal.
fn assert_typed_event_export(contract: &Contract) {
    assert_eq!(
        contract.event_params,
        [
            "handler: borrow<callback>",
            "event: string",
            "resources: record {ctx: own<context>, signal: option<own<abort-signal>>}",
        ],
        "the event export takes the callback, the document and the typed capabilities"
    );
    assert_eq!(
        contract.event_results,
        [concat!(
            "result<record {event: result<option<string>, string>, ",
            "decision: variant {returned(result<option<string>, string>), failed(string)}}, string>"
        )],
        "the event export answers with the two independent parts or a refusal"
    );
}

#[test]
fn maestro_component_contract_keeps_sync_and_async_calls() -> Result<(), String> {
    let contract = contract()?;
    assert_eq!(
        names(&contract.exports, true),
        [
            "invoke-command",
            "invoke-event",
            "invoke-tool",
            "invoke-with-session",
            "start"
        ],
        "every event, command and pending call is native async"
    );
    assert_eq!(
        names(&contract.exports, false),
        ["invoke-prepare", "release"],
        "preparation and release stay synchronous"
    );
    assert_typed_event_export(&contract);
    assert_eq!(
        names(&contract.imports, true),
        [
            "[method]command-context.new-session",
            "[method]command-context.wait-for-idle",
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
