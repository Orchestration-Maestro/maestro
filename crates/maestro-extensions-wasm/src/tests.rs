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

include!(concat!(env!("OUT_DIR"), "/example_tests.rs"));
example_tests!();

mod abort_controller;
pub(crate) mod action_host;
mod advanced_ui;
mod agent_events;
mod application_messages;
pub(crate) mod author;
mod authored_editor;
mod authored_streams;
mod basic_ui;
mod build;
mod bus_callbacks;
pub(crate) mod bus_host;
pub(crate) mod catalog_host;
mod completion_contract;
mod component_driver;
mod context_catalog;
pub(crate) mod controlled;
mod controlled_driver;
mod documents;
mod event_transport;
mod event_values;
mod example_support;
mod exec_records;
pub(crate) mod file_trigger_path;
mod finite_records;
pub(crate) mod stream_host;
mod tui_ui;
#[macro_use]
mod fixtures;
pub(crate) mod custom_host;
pub(crate) mod editor_component;
mod editor_driver;
pub(crate) mod editor_host;
mod editor_ui;
mod foreign_host;
mod guest_family;
pub(crate) mod host;
mod host_family;
mod host_widgets;
mod join;
mod json_payloads;
mod model_streams;
pub(crate) mod native_http_host;
mod native_io;
pub(crate) mod native_io_host;
pub(crate) mod native_watch_host;
mod observed;
pub(crate) mod overlay_host;
mod parameter_schemas;
mod provider_catalog;
mod provider_component;
mod provider_composition;
pub(crate) mod provider_host;
pub(crate) mod reader_host;
mod registration_callbacks;
mod registration_corpus;
mod registration_fixture;
mod registration_host;
mod renderer_callbacks;
pub(crate) mod renderer_host;
mod replacement_commands;
pub(crate) mod replacement_host;
mod scenario;
pub(crate) mod schema_host;
mod selection_events;
mod session_actions;
mod session_corpus;
mod session_events;
mod session_reader;
mod session_records;
mod session_summary;
pub(crate) mod setup_host;
mod setup_sessions;
mod shared_messages;
mod shared_records;
mod shell_callbacks;
pub(crate) mod shell_host;
mod signal_callbacks;
mod stream_events;
pub(crate) mod theme_host;
mod tool_callbacks;
mod tool_events;
pub(crate) mod ui_host;
pub(crate) mod widget_component;
mod widget_controlled;
pub(crate) mod widget_editors;
pub(crate) mod widget_host;
pub(crate) mod widget_keys;
pub(crate) mod widget_lists;
mod writer_controlled;
mod writer_ui;

use std::future::Future;

use wasmtime::component::ResourceType;
use wasmtime::component::types::{ComponentInstance, ComponentItem, Type};

use component_driver::ComponentDriver;
use controlled_driver::ControlledDriver;
use scenario::{Decision, Driver, EXPECTED};

/// Runs a future to completion on a current-thread runtime.
pub(crate) fn block_on<T>(future: impl Future<Output = T>) -> T {
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
        Type::U8 => "u8".to_owned(),
        Type::U32 => "u32".to_owned(),
        Type::String => "string".to_owned(),
        Type::Float64 => "f64".to_owned(),
        Type::Bool => "bool".to_owned(),
        Type::List(list) => format!("list<{}>", spell(&list.ty(), resources)),
        Type::Tuple(tuple) => format!(
            "tuple<{}>",
            tuple
                .types()
                .map(|ty| spell(&ty, resources))
                .collect::<Vec<_>>()
                .join(", ")
        ),
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
        Type::Enum(enumeration) => format!(
            "enum {{{}}}",
            enumeration.names().collect::<Vec<_>>().join(", ")
        ),
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

/// A component function's spelled parameter and result sequences.
type Signature = (Vec<String>, Vec<String>);

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
    /// Ordinary action and compaction resource signatures.
    action_signatures: Vec<Signature>,
    /// Typed registration imports and callback exports.
    registration_signatures: Vec<Signature>,
    /// Returned execution resource and synchronous output types.
    shell_signatures: Vec<Signature>,
    /// Renderer registration, invocation and exported resource methods.
    renderer_signatures: Vec<Signature>,
    /// Signatures of the new retained-capability and usage routes.
    catalog_signatures: Vec<Vec<String>>,
    /// Writable reader acquisition and operation-scoped callback signatures.
    setup_signatures: Vec<Signature>,
    /// Native-async replacement operations.
    replacement_signatures: Vec<Signature>,
    /// Retained UI requests.
    ui_signatures: Vec<Signature>,
    /// Awaited model hooks and independently retained selected cancellation.
    model_hook_signatures: Vec<Signature>,
}

/// Reads the contract of the built component.
fn contract() -> Result<Contract, String> {
    let path = build::author_component()?;
    let (engine, component) = build::compile(path).map_err(|error| error.to_string())?;
    let kind = component.component_type();
    let exported = kind
        .exports(&engine)
        .map(|(name, found)| (name.to_owned(), found.ty));
    let imported = kind
        .imports(&engine)
        .map(|(name, found)| (name.to_owned(), found.ty));
    let guest = instance(exported, "maestro:extension/guest@0.1.0")?;
    let host = instance(imported, "maestro:extension/host@0.1.0")?;
    let (event_params, event_results) = signature(&engine, &guest, "invoke-event", &{
        let mut declared = resources(&engine, &host);
        declared.extend(resources(&engine, &guest));
        declared
    })?;
    Ok(Contract {
        exports: functions(&engine, &guest),
        imports: functions(&engine, &host),
        interfaces: kind
            .imports(&engine)
            .map(|(name, _)| name.to_owned())
            .collect(),
        catalog_signatures: [
            "[method]context.ui",
            "[method]context.model-registry",
            "[method]context.signal",
            "[method]context.get-context-usage",
            "[method]model-registry.get-api-key-and-headers",
        ]
        .into_iter()
        .map(|name| {
            signature(&engine, &host, name, &resources(&engine, &host)).map(|(_, result)| result)
        })
        .collect::<Result<_, _>>()?,
        setup_signatures: [
            (&host, "[method]session-manager.reader"),
            (&host, "[method]command-context.new-session"),
            (&guest, "invoke-setup"),
        ]
        .into_iter()
        .map(|(interface, name)| signature(&engine, interface, name, &resources(&engine, &host)))
        .collect::<Result<_, _>>()?,
        replacement_signatures: replacement_signatures(&engine, &host)?,
        ui_signatures: ui_signatures(&engine, &host)?,
        action_signatures: action_signatures(&engine, &host, &guest)?,
        registration_signatures: registration_signatures(&engine, &host, &guest)?,
        renderer_signatures: renderer_signatures(&engine, &host, &guest)?,
        shell_signatures: shell_signatures(&engine, &host, &guest)?,
        model_hook_signatures: model_hook_signatures(&engine, &host, &guest)?,
        event_params,
        event_results,
    })
}

/// The event export takes the callback, the document and the typed capabilities, and answers
/// with independent event, decision and operations channels or a refusal.
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
            "decision: variant {returned(result<option<string>, string>), failed(string)}, operations: option<own<callback>>}, string>"
        )],
        "the event export answers with independent event, decision and operations channels or a refusal"
    );
}

/// Reader acquisition and node traversal remain synchronous component imports.
fn assert_reader_imports(contract: &Contract) {
    let methods: Vec<_> = contract
        .imports
        .iter()
        .filter(|(name, _)| {
            name.starts_with("[method]")
                && (name.contains("session-manager") || name.contains("session-tree-node"))
                && !name.starts_with("[method]session-manager.")
        })
        .collect();
    assert!(methods.iter().all(|(_, asynchronous)| !asynchronous));
    assert_eq!(
        names(&contract.imports, false)
            .into_iter()
            .filter(|name| {
                name.starts_with("[method]")
                    && (name.contains("session-manager") || name.contains("session-tree-node"))
                    && !name.starts_with("[method]session-manager.")
            })
            .collect::<Vec<_>>(),
        [
            "[method]command-context.session-manager",
            "[method]context.session-manager",
            "[method]readonly-session-manager.get-branch",
            "[method]readonly-session-manager.get-cwd",
            "[method]readonly-session-manager.get-entries",
            "[method]readonly-session-manager.get-entry",
            "[method]readonly-session-manager.get-header",
            "[method]readonly-session-manager.get-label",
            "[method]readonly-session-manager.get-leaf-entry",
            "[method]readonly-session-manager.get-leaf-id",
            "[method]readonly-session-manager.get-session-dir",
            "[method]readonly-session-manager.get-session-file",
            "[method]readonly-session-manager.get-session-id",
            "[method]readonly-session-manager.get-session-name",
            "[method]readonly-session-manager.get-tree",
            "[method]session-tree-node.children",
            "[method]session-tree-node.entry",
            "[method]session-tree-node.label",
            "[method]session-tree-node.label-timestamp",
        ]
    );
}

/// The new scalar routes are synchronous; usage and retained resources have typed results.
fn assert_catalog_imports(contract: &Contract) {
    let context_methods = [
        "ui",
        "has-ui",
        "model-registry",
        "model",
        "is-idle",
        "signal",
        "abort",
        "has-pending-messages",
        "shutdown",
        "get-context-usage",
        "get-system-prompt",
    ];
    let mut expected = ["context", "command-context"]
        .into_iter()
        .flat_map(|resource| context_methods.map(|method| format!("[method]{resource}.{method}")))
        .collect::<Vec<_>>();
    expected.extend(
        [
            "get-all",
            "get-available",
            "find",
            "refresh",
            "get-error",
            "has-configured-auth",
            "get-provider-auth-status",
            "get-provider-display-name",
            "is-using-oauth",
            "register-provider",
            "unregister-provider",
        ]
        .map(|method| format!("[method]model-registry.{method}")),
    );
    expected.sort_unstable();
    let actual = names(&contract.imports, false)
        .into_iter()
        .filter(|name| expected.iter().any(|route| route == name))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert_eq!(
        contract.catalog_signatures,
        [
            vec!["result<own<extension-ui-context>, string>"],
            vec!["result<own<model-registry>, string>"],
            vec!["result<option<own<abort-signal>>, string>"],
            vec![
                "result<option<record {tokens: option<f64>, context-window: f64, percent: option<f64>}>, string>"
            ],
            vec![
                "result<variant {ok(record {api-key: option<string>, headers: option<list<tuple<string, string>>>}), error(string)}, string>"
            ],
        ]
    );
}

#[test]
fn maestro_component_contract_keeps_sync_and_async_calls() -> Result<(), String> {
    let contract = contract()?;
    assert_async_exports(&contract);
    assert_eq!(
        names(&contract.exports, false),
        ["describe-schema", "release", "retain-schema"],
        "describe, retain and release only touch guest callback storage"
    );
    assert_shell_contract(&contract);
    assert_model_hook_contract(&contract);
    assert_renderer_contract(&contract);
    assert_typed_event_export(&contract);
    assert_reader_imports(&contract);
    assert_catalog_imports(&contract);
    assert_registration_contract(&contract);
    assert_action_contract(&contract);
    assert_setup_contract(&contract);
    assert_replacement_contract(&contract);
    assert_ui_contract(&contract);
    assert_async_imports(&contract);
    assert_interface_contract(&contract);
    Ok(())
}

/// Checks every asynchronous host import the guest calls.
fn assert_async_imports(contract: &Contract) {
    assert_eq!(
        names(&contract.imports, true),
        [
            "[method]auth-lock-update.update-async",
            "[method]auth-storage.get-api-key",
            "[method]auth-storage.login",
            "[method]builtin-tool.execute",
            "[method]child-process.wait",
            "[method]command-context.fork",
            "[method]command-context.navigate-tree",
            "[method]command-context.new-session",
            "[method]command-context.reload",
            "[method]command-context.switch-session",
            "[method]command-context.wait-for-idle",
            "[method]completion-provider.get-suggestions",
            "[method]extension-ui-context.confirm",
            "[method]extension-ui-context.custom",
            "[method]extension-ui-context.editor",
            "[method]extension-ui-context.input",
            "[method]extension-ui-context.select",
            "[method]file-mutation.wait",
            "[method]local-bash-operations.exec",
            "[method]model-registry.get-api-key-and-headers",
            "[method]model-registry.get-api-key-for-provider",
            "[method]oauth-interaction.input",
            "[method]oauth-provider.login",
            "[method]oauth-provider.refresh-token",
            "[method]provider-controls.invoke",
            "[method]readable.read",
            "[method]replaced-session-context.send-message",
            "[method]replaced-session-context.send-user-message",
            "[method]response.json",
            "[method]response.text",
            "[method]widget.drain-input",
            "[method]writable.end",
            "[method]writable.write",
            "call",
            "call-host-operation",
            "event-emit",
            "fetch",
            "get-all-tools",
            "register-tool",
            "set-model",
            "sleep",
            "spawn",
        ]
    );
}

/// Reads the retained UI request signatures of the built component.
fn ui_signatures(
    engine: &wasmtime::Engine,
    host: &ComponentInstance,
) -> Result<Vec<Signature>, String> {
    let resources = resources(engine, host);
    [
        "notify",
        "set-status",
        "set-working-message",
        "set-working-visible",
        "set-working-indicator",
        "set-hidden-thinking-label",
        "set-widget",
        "set-title",
    ]
    .into_iter()
    .map(|name| {
        signature(
            engine,
            host,
            &format!("[method]extension-ui-context.{name}"),
            &resources,
        )
    })
    .collect()
}

/// The retained UI resource is borrowed while its requests stay synchronous and typed.
fn assert_ui_contract(contract: &Contract) {
    let unit = vec!["own<import-route>".to_owned()];
    let self_ = "self: borrow<extension-ui-context>".to_owned();
    let params = [
        vec![
            self_.clone(),
            "message: string".to_owned(),
            "kind: option<enum {info, warning, error}>".to_owned(),
        ],
        vec![
            self_.clone(),
            "key: string".to_owned(),
            "text: option<string>".to_owned(),
        ],
        vec![self_.clone(), "message: option<string>".to_owned()],
        vec![self_.clone(), "visible: bool".to_owned()],
        vec![
            self_.clone(),
            "options: option<record {frames: option<list<string>>, interval-ms: option<f64>}>"
                .to_owned(),
        ],
        vec![self_.clone(), "label: option<string>".to_owned()],
        vec![
            self_.clone(),
            "key: string".to_owned(),
            "content: option<list<string>>".to_owned(),
            "options: option<record {placement: option<enum {above-editor, below-editor}>}>"
                .to_owned(),
        ],
        vec![self_, "title: string".to_owned()],
    ];
    for (index, ((actual, results), expected)) in
        contract.ui_signatures.iter().zip(params).enumerate()
    {
        assert_eq!(actual, &expected, "retained UI request {index} parameters");
        assert_eq!(
            results, &unit,
            "retained UI request {index} stays synchronous through its route"
        );
    }
}

/// Reads canonical registration input/output types from the built component.
fn registration_signatures(
    engine: &wasmtime::Engine,
    host: &ComponentInstance,
    guest: &ComponentInstance,
) -> Result<Vec<Signature>, String> {
    let resources = resources(engine, host);
    [
        (host, "register-command"),
        (host, "register-shortcut"),
        (host, "register-flag"),
        (host, "get-flag"),
        (host, "contribute-load-time"),
        (guest, "invoke-completion"),
        (guest, "invoke-shortcut"),
    ]
    .into_iter()
    .map(|(interface, name)| signature(engine, interface, name, &resources))
    .collect()
}

/// Typed callback resources and flag variants preserve the synchronous/native-async split.
fn assert_registration_contract(contract: &Contract) {
    let sync = names(&contract.imports, false);
    for name in [
        "register-command",
        "register-shortcut",
        "register-flag",
        "get-flag",
        "contribute-load-time",
    ] {
        assert!(sync.contains(&name));
    }
    let expected = [
        (vec!["name: string", "description: option<string>", "completions: option<borrow<callback>>", "handler: borrow<callback>"], vec!["result<_, string>"]),
        (vec!["shortcut: string", "description: option<string>", "handler: borrow<callback>"], vec!["result<_, string>"]),
        (vec!["name: string", "options: record {description: option<string>, type: enum {boolean, string}, default: option<variant {boolean(bool), string(string)}>}"], vec!["result<_, string>"]),
        (vec!["name: string"], vec!["result<option<variant {boolean(bool), string(string)}>, string>"]),
        (vec!["contributions: string"], vec!["result<_, string>"]),
        (vec!["handler: borrow<callback>", "prefix: string"], vec!["result<option<list<record {value: string, label: string, description: option<string>}>>, string>"]),
        (vec!["handler: borrow<callback>", "ctx: own<context>"], vec!["result<_, string>"]),
    ].map(|(params, results)| (params.into_iter().map(str::to_owned).collect::<Vec<_>>(), results.into_iter().map(str::to_owned).collect::<Vec<_>>()));
    assert_eq!(contract.registration_signatures, expected);
}

/// Inspect the actual lowered sync admission and async model-selection types.
fn action_signatures(
    engine: &wasmtime::Engine,
    host: &ComponentInstance,
    guest: &ComponentInstance,
) -> Result<Vec<Signature>, String> {
    let resources = resources(engine, host);
    [
        (host, "send-message"),
        (host, "send-user-message"),
        (host, "set-model"),
        (host, "[method]context.compact"),
        (host, "[method]command-context.compact"),
        (guest, "invoke-compaction-complete"),
        (guest, "invoke-compaction-error"),
    ]
    .into_iter()
    .map(|(interface, name)| signature(engine, interface, name, &resources))
    .collect()
}
/// These imports and notifications carry typed borrowed identities, not futures or polling.
fn assert_action_contract(contract: &Contract) {
    let sync = names(&contract.imports, false);
    for name in [
        "send-message",
        "send-user-message",
        "set-session-name",
        "get-session-name",
        "set-label",
        "get-active-tools",
        "set-active-tools",
        "get-commands",
        "get-thinking-level",
        "set-thinking-level",
        "register-provider",
        "unregister-provider",
        "[method]context.compact",
        "[method]command-context.compact",
    ] {
        assert!(sync.contains(&name), "sync route {name}");
    }
    let expected = expected_action_signatures();
    assert_eq!(contract.action_signatures, expected);
}

/// The component admits only its author world and supported system interfaces.
fn assert_interface_contract(contract: &Contract) {
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
    let wasi: Vec<_> = contract
        .interfaces
        .iter()
        .filter(|name| name.starts_with("wasi:"))
        .collect();
    assert!(
        wasi.iter().any(|name| name.contains("@0.2.")),
        "standard-library baseline remains available"
    );
    assert!(
        wasi.iter().any(|name| name.contains("@0.3.")),
        "native asynchronous standard imports are available"
    );
    assert!(
        wasi.iter()
            .all(|name| name.contains("@0.2.") || name.contains("@0.3.")),
        "only supported standard generations: {wasi:?}"
    );
}

/// Expected parameter/result sequences for the action wire.
fn expected_action_signatures() -> [Signature; 7] {
    [
        (
            vec!["message: string", "options: option<string>"],
            vec!["own<import-route>"],
        ),
        (
            vec!["content: string", "options: option<string>"],
            vec!["own<import-route>"],
        ),
        (vec!["model: string"], vec!["result<bool, string>"]),
        (
            vec![
                "self: borrow<context>",
                "options: option<record {custom-instructions: option<string>}>",
                "on-complete: option<borrow<callback>>",
                "on-error: option<borrow<callback>>",
            ],
            vec!["own<import-route>"],
        ),
        (
            vec![
                "self: borrow<command-context>",
                "options: option<record {custom-instructions: option<string>}>",
                "on-complete: option<borrow<callback>>",
                "on-error: option<borrow<callback>>",
            ],
            vec!["own<import-route>"],
        ),
        (
            vec![
                "handler: borrow<callback>",
                "result: string",
                "emitter: borrow<callback-emitter>",
            ],
            vec!["result<_, string>"],
        ),
        (
            vec![
                "handler: borrow<callback>",
                "error: string",
                "emitter: borrow<callback-emitter>",
            ],
            vec!["result<_, string>"],
        ),
    ]
    .map(|(params, results)| {
        (
            params.into_iter().map(str::to_owned).collect::<Vec<_>>(),
            results.into_iter().map(str::to_owned).collect::<Vec<_>>(),
        )
    })
}

/// Writer methods remain synchronous while setup crosses the native async callback seam.
fn assert_setup_contract(contract: &Contract) {
    let methods: Vec<_> = names(&contract.imports, false)
        .into_iter()
        .filter_map(|n| n.strip_prefix("[method]session-manager."))
        .collect();
    assert_eq!(
        methods,
        [
            "append-compaction",
            "append-custom-entry",
            "append-custom-message-entry",
            "append-label-change",
            "append-message",
            "append-model-change",
            "append-session-info",
            "append-thinking-level-change",
            "branch",
            "branch-with-summary",
            "build-session-context",
            "create-branched-session",
            "get-children",
            "is-persisted",
            "new-session",
            "reader",
            "reset-leaf",
            "set-session-file"
        ]
    );
    let expected = [
        (
            vec!["self: borrow<session-manager>"],
            vec!["own<readonly-session-manager>"],
        ),
        (
            vec![
                "self: borrow<command-context>",
                "data: option<record {parent-session: option<string>}>",
                "setup: option<borrow<callback>>",
                "with-session: option<borrow<callback>>",
            ],
            vec!["result<record {cancelled: bool}, string>"],
        ),
        (
            vec!["handler: borrow<callback>", "writer: own<session-manager>"],
            vec!["result<_, string>"],
        ),
    ]
    .map(|(p, r)| {
        (
            p.into_iter().map(str::to_owned).collect::<Vec<_>>(),
            r.into_iter().map(str::to_owned).collect::<Vec<_>>(),
        )
    });
    assert_eq!(contract.setup_signatures, expected);
}

/// Inspect each actual import signature, including the borrowed callback/context operands.
fn replacement_signatures(
    engine: &wasmtime::Engine,
    host: &ComponentInstance,
) -> Result<Vec<Signature>, String> {
    let resources = resources(engine, host);
    [
        "command-context.fork",
        "command-context.navigate-tree",
        "command-context.switch-session",
        "command-context.reload",
        "replaced-session-context.send-message",
        "replaced-session-context.send-user-message",
    ]
    .into_iter()
    .map(|name| signature(engine, host, &format!("[method]{name}"), &resources))
    .collect()
}
/// Replacement sends remain awaitable while ordinary action sends remain synchronous.
fn assert_replacement_contract(contract: &Contract) {
    let change = vec!["result<record {cancelled: bool}, string>".to_owned()];
    let unit = vec!["result<_, string>".to_owned()];
    let params = [
        vec![
            "self: borrow<command-context>",
            "entry-id: string",
            "data: option<string>",
            "with-session: option<borrow<callback>>",
        ],
        vec![
            "self: borrow<command-context>",
            "target-id: string",
            "options: option<string>",
        ],
        vec![
            "self: borrow<command-context>",
            "path: string",
            "options: option<record {with-session: option<borrow<callback>>}>",
        ],
        vec!["self: borrow<command-context>"],
        vec![
            "self: borrow<replaced-session-context>",
            "message: string",
            "options: option<string>",
        ],
        vec![
            "self: borrow<replaced-session-context>",
            "content: string",
            "options: option<string>",
        ],
    ];
    for (index, ((actual_params, actual_results), expected)) in contract
        .replacement_signatures
        .iter()
        .zip(params)
        .enumerate()
    {
        assert_eq!(actual_params, &expected);
        assert_eq!(actual_results, if index < 3 { &change } else { &unit });
    }
}

/// Reads imported and exported resource signatures using their declared ownership direction.
fn renderer_signatures(
    engine: &wasmtime::Engine,
    host: &ComponentInstance,
    guest: &ComponentInstance,
) -> Result<Vec<Signature>, String> {
    let mut declared = resources(engine, host);
    declared.extend(resources(engine, guest));
    [
        (host, "register-message-renderer"),
        (guest, "invoke-renderer"),
        (guest, "component-render"),
        (guest, "component-handle-input"),
        (guest, "component-wants-key-release"),
        (guest, "component-invalidate"),
    ]
    .into_iter()
    .map(|(interface, name)| signature(engine, interface, name, &declared))
    .collect()
}
/// Checks the renderer and the retained-component exports keyed by callback identity.
fn assert_renderer_contract(contract: &Contract) {
    let expected = [
        (
            vec!["custom-type: string", "handler: borrow<callback>"],
            vec!["result<_, string>"],
        ),
        (
            vec![
                "handler: borrow<callback>",
                "request: record {message: string, expanded: bool, theme: own<theme>}",
                "emitter: borrow<callback-emitter>",
            ],
            vec!["result<option<own<callback>>, string>"],
        ),
        (
            vec![
                "handler: borrow<callback>",
                "width: u32",
                "emitter: borrow<callback-emitter>",
            ],
            vec!["result<list<string>, string>"],
        ),
        (
            vec![
                "handler: borrow<callback>",
                "data: string",
                "emitter: borrow<callback-emitter>",
            ],
            vec!["result<_, string>"],
        ),
        (
            vec![
                "handler: borrow<callback>",
                "emitter: borrow<callback-emitter>",
            ],
            vec!["result<bool, string>"],
        ),
        (
            vec![
                "handler: borrow<callback>",
                "emitter: borrow<callback-emitter>",
            ],
            vec!["result<_, string>"],
        ),
    ];
    let expected = expected
        .into_iter()
        .map(|(params, results)| {
            (
                params.into_iter().map(str::to_owned).collect(),
                results.into_iter().map(str::to_owned).collect(),
            )
        })
        .collect::<Vec<Signature>>();
    assert_eq!(contract.renderer_signatures, expected);
    assert!(
        contract
            .imports
            .contains(&("register-message-renderer".into(), false))
    );
}

/// Reads the actual owned execution and raw-output resource methods.
fn shell_signatures(
    engine: &wasmtime::Engine,
    host: &ComponentInstance,
    guest: &ComponentInstance,
) -> Result<Vec<Signature>, String> {
    let mut declared = resources(engine, host);
    declared.extend(resources(engine, guest));
    [(guest, "bash-exec"), (host, "[method]bash-output.on-data")]
        .into_iter()
        .map(|(interface, name)| signature(engine, interface, name, &declared))
        .collect()
}
/// Owned output and optional signal coexist with literal numeric/environment operands.
fn assert_shell_contract(contract: &Contract) {
    assert!(
        contract
            .imports
            .contains(&("[method]bash-output.on-data".into(), false))
    );
    let expected = [
        (vec!["handler: borrow<callback>", "request: record {command: string, cwd: string, timeout: option<f64>, env: option<list<tuple<string, option<string>>>>}", "resources: record {output: own<bash-output>, signal: option<own<abort-signal>>}"],vec!["result<option<f64>, string>"]),
        (vec!["self: borrow<bash-output>","data: list<u8>"],vec!["result<_, string>"])
    ].map(|(params,results)| (params.into_iter().map(str::to_owned).collect::<Vec<_>>(),results.into_iter().map(str::to_owned).collect::<Vec<_>>()));
    assert_eq!(contract.shell_signatures, expected);
}

/// Complete asynchronous export inventory, including the returned operation method.
fn assert_async_exports(contract: &Contract) {
    assert_eq!(
        names(&contract.exports, true),
        [
            "bash-exec",
            "component-focused",
            "component-handle-input",
            "component-invalidate",
            "component-render",
            "component-set-focused",
            "component-wants-key-release",
            "invoke-abort",
            "invoke-assistant",
            "invoke-auth-backend",
            "invoke-auth-fallback",
            "invoke-autocomplete",
            "invoke-autocomplete-factory",
            "invoke-branch-change",
            "invoke-command",
            "invoke-compaction-complete",
            "invoke-compaction-error",
            "invoke-completion",
            "invoke-component-factory",
            "invoke-custom-factory",
            "invoke-custom-future",
            "invoke-dispose",
            "invoke-edit",
            "invoke-editor",
            "invoke-editor-factory",
            "invoke-event",
            "invoke-find",
            "invoke-grep",
            "invoke-highlight",
            "invoke-listener",
            "invoke-ls",
            "invoke-model-hook",
            "invoke-oauth-callback",
            "invoke-prepare",
            "invoke-provider",
            "invoke-read",
            "invoke-renderer",
            "invoke-schema",
            "invoke-set-expanded",
            "invoke-setup",
            "invoke-shortcut",
            "invoke-stream",
            "invoke-style",
            "invoke-tail",
            "invoke-terminal-input",
            "invoke-tool",
            "invoke-tool-render",
            "invoke-watch",
            "invoke-widget",
            "invoke-with-session",
            "invoke-write",
            "start"
        ],
        "every event, command and pending call is native async"
    );
}

/// Inspect the actual awaited hook export and synchronous cancellation-retention method.
fn model_hook_signatures(
    engine: &wasmtime::Engine,
    host: &ComponentInstance,
    guest: &ComponentInstance,
) -> Result<Vec<Signature>, String> {
    [
        (guest, "invoke-model-hook"),
        (host, "[method]abort-signal.retain"),
    ]
    .into_iter()
    .map(|(interface, name)| signature(engine, interface, name, &resources(engine, host)))
    .collect()
}

/// Selected signals and their local binding owners cross as resources, never JSON identifiers.
fn assert_model_hook_contract(contract: &Contract) {
    assert!(
        contract
            .imports
            .contains(&("[method]abort-signal.retain".into(), false))
    );
    let expected = [
        (vec!["handler: borrow<callback>","phase: enum {payload, response}","data: string","model: string","signal: option<own<abort-signal>>"],vec!["result<record {data: string, signal: option<own<abort-signal>>, owner: option<own<callback>>}, string>"]),
        (vec!["self: borrow<abort-signal>"],vec!["own<abort-signal>"]),
    ].map(|(params,results)|(params.into_iter().map(str::to_owned).collect::<Vec<_>>(),results.into_iter().map(str::to_owned).collect::<Vec<_>>()));
    assert_eq!(contract.model_hook_signatures, expected);
}

mod tool_record_views;

mod credential_host;
mod native_helpers;

mod custom_ticks;

mod snake_frames;

mod package_boundary;
