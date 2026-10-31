//! Base extension context records through the application root.
#[cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::no_commands;
#[cfg(not(target_arch = "wasm32"))]
use maestro_app::{CompactOptions, ContextUsage, ExtensionContext, ExtensionContextActions};
#[cfg(not(target_arch = "wasm32"))]
use maestro_app::{ExtensionActions, ExtensionError, ExtensionRuntime, create_extension_runtime};
#[cfg(not(target_arch = "wasm32"))]
use maestro_catalog::ModelRegistry;
#[cfg(not(target_arch = "wasm32"))]
use maestro_credentials::{AuthStorage, AuthStorageData};
#[cfg(not(target_arch = "wasm32"))]
use maestro_extensions::ExtensionRunner;
#[cfg(not(target_arch = "wasm32"))]
use maestro_extensions::runner::ExtensionRunnerSession;
#[cfg(not(target_arch = "wasm32"))]
use maestro_extensions::{Extension, ExtensionAPI, ExtensionErrorListener, ExtensionExecution};
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::Threaded;
#[cfg(not(target_arch = "wasm32"))]
use maestro_session::SessionManager;
#[cfg(not(target_arch = "wasm32"))]
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// The default value of an operand whose type the application does not name,
/// such as the runner's live theme.
#[cfg(not(target_arch = "wasm32"))]
fn default_operand<T: Default>() -> T {
    T::default()
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_context_exports_share_the_extension_records() {
    let usage = ContextUsage {
        tokens: Some(1_000.0),
        context_window: 200_000.0,
        percent: Some(0.5),
    };
    let requests = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&requests);
    let actions = ExtensionContextActions {
        get_context_usage: Rc::new(move || Ok(Some(usage))),
        compact: Rc::new(move |options: Option<CompactOptions>| {
            recorded
                .borrow_mut()
                .push(options.and_then(|o| o.custom_instructions));
            Ok(())
        }),
        ..ExtensionContextActions::default()
    };
    let runner = runner();
    runner
        .bind_core(
            <maestro_models::Threaded as maestro_extensions::ExtensionExecution>::unbound_actions(),
            actions,
            maestro_extensions::types::ProviderActions::default(),
        )
        .unwrap();
    let ctx: ExtensionContext = runner.create_context();
    assert_eq!(ctx.get_context_usage().unwrap(), Some(usage));
    let request = CompactOptions {
        custom_instructions: Some("keep".into()),
        ..CompactOptions::default()
    };
    ctx.compact(Some(request)).unwrap();
    assert_eq!(*requests.borrow(), [Some("keep".to_owned())]);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_command_context_exports_share_the_extension_records() {
    let runner = runner();
    let actions =
        maestro_app::ExtensionCommandContextActions::new(maestro_app::CommandContextOperations {
            new_session: Rc::new(|_| {
                Box::pin(async {
                    Ok(maestro_extensions::types::SessionChangeResult { cancelled: true })
                })
            }),
            ..crate::command_operations::silent()
        });
    runner.bind_command_context(Some(actions));
    let ctx: maestro_app::ExtensionCommandContext = runner.create_command_context();
    crate::support::run(async {
        assert!(ctx.new_session(None).await.unwrap().cancelled);
    });
}

/// Builds the real in-memory runner shared by application context export witnesses.
#[cfg(not(target_arch = "wasm32"))]
fn runner() -> ExtensionRunner {
    let auth = Arc::new(AuthStorage::in_memory(AuthStorageData::default()));
    let session = ExtensionRunnerSession {
        cwd: "/work".into(),
        session_manager: Rc::new(RefCell::new(SessionManager::in_memory(None).unwrap())),
        model_registry: maestro_models::SharedHandle::new(
            ModelRegistry::in_memory(auth, no_commands()).unwrap(),
        ),
    };
    ExtensionRunner::<maestro_models::Threaded>::new(
        Arc::new(RwLock::new(Vec::new())),
        maestro_extensions::create_extension_runtime::<maestro_models::Threaded>(),
        session,
        Arc::new(|_| {}),
        default_operand(),
    )
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_runtime_exports_share_the_extension_records() {
    let runtime: Arc<ExtensionRuntime> = create_extension_runtime::<Threaded>();
    let mut actions: ExtensionActions = Threaded::unbound_actions();
    let expected = serde_json::to_value(runtime_tool()).unwrap();
    actions.get_all_tools = Arc::new(|| Ok(vec![runtime_schema_tool()]));
    let config = runtime_provider();
    let owner = Arc::new(RwLock::new(Extension::new(
        "/extension".into(),
        "/extension".into(),
        Arc::new(RwLock::new(
            serde_json::from_value(expected["sourceInfo"].clone()).unwrap(),
        )),
    )));
    let events = maestro_extensions::create_event_bus(Arc::new(drop), Arc::new(|_| {}));
    let api = ExtensionAPI::new(
        &owner,
        runtime.clone(),
        events.for_extension(),
        "/work".into(),
    );
    api.register_provider("metadata".into(), Arc::new(RwLock::new(config)))
        .unwrap();
    let runner = runtime_runner(runtime);
    let errors = Rc::new(RefCell::new(Vec::new()));
    let output = errors.clone();
    let listener: ExtensionErrorListener = Rc::new(move |error| {
        output.borrow_mut().push(error.clone());
        Ok(())
    });
    let off = runner.on_error(listener);
    runner
        .bind_core(
            actions,
            ExtensionContextActions::default(),
            maestro_extensions::types::ProviderActions::default(),
        )
        .unwrap();
    assert!(errors.borrow().is_empty());
    assert_eq!(
        serde_json::to_value(api.get_all_tools().unwrap()).unwrap(),
        serde_json::json!([expected])
    );
    let error = ExtensionError {
        extension_path: "/extension".into(),
        event: "custom".into(),
        error: "supplied".into(),
        stack: Some("stack".into()),
    };
    runner.emit_error(&error).unwrap();
    assert_eq!(*errors.borrow(), [error]);
    assert!(
        runner
            .create_context()
            .model_registry()
            .unwrap()
            .find("metadata", "model")
            .is_some()
    );
    off();
}

/// Tool metadata constructed through the embedding SDK.
#[cfg(not(target_arch = "wasm32"))]
fn runtime_tool() -> maestro_app::ToolInfo {
    let source = serde_json::from_value(serde_json::json!({"path":"/extension","source":"cli","scope":"temporary","origin":"top-level"})).unwrap();
    maestro_app::ToolInfo {
        name: "inspect".into(),
        description: "Inspect data".into(),
        parameters: serde_json::json!({"type":"object"}).into(),
        source_info: source,
    }
}

/// Add local executable storage only after the threaded callback is invoked.
#[cfg(not(target_arch = "wasm32"))]
fn runtime_schema_tool()
-> maestro_app::ToolInfo<maestro_models::ParameterSchema<maestro_models::SchemaCode>> {
    let tool = runtime_tool();
    maestro_app::ToolInfo {
        name: tool.name,
        description: tool.description,
        parameters: tool.parameters.with_code(maestro_models::SchemaCode::new()),
        source_info: tool.source_info,
    }
}

/// A real provider model routed through the embedding SDK.
#[cfg(not(target_arch = "wasm32"))]
fn runtime_provider() -> maestro_app::ProviderConfig {
    maestro_app::ProviderConfig {
        base_url: Some("https://provider.invalid".into()),
        api_key: Some("literal".into()),
        api: Some("fixture-runtime".into()),
        models: Some(vec![maestro_app::ProviderModelConfig {
            id: "model".into(),
            name: "Model".into(),
            api: None,
            base_url: None,
            reasoning: true,
            thinking_level_map: None,
            input: vec![],
            cost: maestro_models::ModelCost::default(),
            context_window: 100.0,
            max_tokens: 10.0,
            headers: None,
            compat: None,
        }]),
        ..Default::default()
    }
}
/// Serves a supplied action runtime using the real context fixture's owners.
#[cfg(not(target_arch = "wasm32"))]
fn runtime_runner(runtime: Arc<maestro_app::ExtensionRuntime>) -> ExtensionRunner {
    let served = runner();
    ExtensionRunner::<maestro_models::Threaded>::new(
        Arc::default(),
        runtime,
        ExtensionRunnerSession {
            cwd: "/work".into(),
            session_manager: served.create_context().session_manager().unwrap(),
            model_registry: served.create_context().model_registry().unwrap(),
        },
        Arc::new(|_| {}),
        default_operand(),
    )
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_registration_exports_resolve_to_shared_records() {
    crate::support::run(async {
        let runtime = maestro_app::create_extension_runtime::<Threaded>();
        let bus = maestro_extensions::create_event_bus(Arc::new(|_| {}), Arc::new(|_| {}));
        let factory: maestro_app::ExtensionFactory = Arc::new(|api| {
            let tool: maestro_extensions::ToolDefinition<serde_json::Value, serde_json::Value, ()> =
                sdk_registered_definition();
            api.register_tool(maestro_app::define_tool(tool)).unwrap();
            Box::pin(async { Ok(()) })
        });
        let owner = maestro_app::load_extension_from_factory::<Threaded>(
            factory,
            "/factory".into(),
            bus.for_extension(),
            runtime.clone(),
            None,
        )
        .await
        .unwrap();
        let served = runner();
        let runner = Rc::new(maestro_app::ExtensionRunner::<Threaded>::new(
            Arc::new(RwLock::new(vec![owner])),
            runtime,
            ExtensionRunnerSession {
                cwd: "/registered".into(),
                session_manager: served.create_context().session_manager().unwrap(),
                model_registry: served.create_context().model_registry().unwrap(),
            },
            Arc::new(|_| {}),
            default_operand(),
        ));
        let registered: maestro_app::RegisteredTool = runner.get_all_registered_tools().remove(0);
        assert_eq!(registered.source_info.read().unwrap().path, "<inline>");
        let wrapped = maestro_app::wrap_registered_tool(&registered, runner);
        let result = (wrapped.execute)("sdk".into(), serde_json::json!({"value":3}), None, None)
            .await
            .unwrap();
        assert_eq!(result.details, Some(serde_json::json!({"value":3})));
        let event: maestro_app::ToolCallEvent = maestro_app::ToolCallEvent {
            tool_call_id: "sdk".into(),
            tool_name: "sdk".into(),
            input: result.details.unwrap(),
        };
        assert!(maestro_app::is_tool_call_event_type("sdk", &event));
        let response: maestro_app::ToolCallEventResult =
            serde_json::from_value(serde_json::json!({"block":false})).unwrap();
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            serde_json::json!({"block":false})
        );
    });
}

/// SDK callback captures are thread-safe while registered invocation is local.
#[cfg(not(target_arch = "wasm32"))]
fn sdk_registered_definition() -> maestro_app::ToolDefinition<
    ExtensionContext,
    serde_json::Value,
    serde_json::Value,
    (),
    maestro_models::Local,
> {
    maestro_app::ToolDefinition {
        definition: maestro_models::Tool {
            name: "sdk".into(),
            description: "SDK".into(),
            parameters: serde_json::json!({"type":"object"}).into(),
        },
        label: "SDK".into(),
        prompt_snippet: None,
        prompt_guidelines: None,
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: Arc::new(|id, params, _, _, context| {
            assert_eq!(id, "sdk");
            let context = context.unwrap();
            Box::pin(async move {
                tokio::task::yield_now().await;
                assert_eq!(context.cwd().unwrap(), "/registered");
                Ok(maestro_agent::AgentToolResult {
                    content: vec![],
                    details: Some(params),
                    terminate: None,
                    usage: None,
                })
            })
        }),
        render_call: None,
        render_result: None,
    }
}

/// Compilation proves root aliases retain the extension-owned contracts.
macro_rules! extension_export_identity {
    ($($name:ident $(<$($arg:ty),*>)?),* $(,)?) => {$(
        const _: fn(maestro_app::$name $(<$($arg),*>)?) -> maestro_extensions::$name $(<$($arg),*>)? = |value| value;
    )*};
}
extension_export_identity!(ResourcesDiscoverReason,ResourcesDiscoverEvent,ResourcesDiscoverResult,
    SessionStartReason,SessionStartEvent,SessionBeforeSwitchReason,SessionBeforeSwitchEvent,
    SessionBeforeForkEvent,SessionShutdownReason,SessionShutdownEvent,SessionBeforeSwitchResult,
    SessionBeforeForkResult,BeforeProviderRequestEvent,BeforeProviderRequestEventResult,AfterProviderResponseEvent,
    InputSource,InputEvent,InputTransform,InputEventResult,SessionBeforeCompactEventData,SessionBeforeCompactEvent,
    SessionBeforeCompactResult,ContextEvent,BeforeAgentStartEvent,AgentStartEvent,AgentEndEvent,TurnStartEvent,
    TurnEndEvent,MessageStartEvent,MessageUpdateEvent,MessageEndEvent,ContextEventResult,MessageEndEventResult,
    BeforeAgentStartEventResult,ModelSelectSource,ModelSelectEvent,ThinkingLevelSelectEvent,SessionCompactEvent,
    TreePreparation,SessionTreeEvent,SessionBeforeTreeSummary,SessionBeforeTreeResult,SessionBeforeTreeEvent,
    ToolExecutionStartEvent,ToolExecutionUpdateEvent,ToolExecutionEndEvent,ToolCallEvent,ToolResultEvent,
    CustomToolCallEvent,CustomToolResultEvent,UserBashEvent,UserBashEventResult<(),()>,ToolCallEventResult,
    ToolResultEventResult,SessionEvent,
    ExtensionEvent,ExtensionEventResult<()>,ExtensionFactory,RegisteredTool);
#[cfg(not(target_arch = "wasm32"))]
extension_export_identity!(
    BashToolCallEvent,
    BashToolResultEvent,
    ReadToolCallEvent,
    ReadToolResultEvent,
    EditToolCallEvent,
    EditToolResultEvent,
    WriteToolCallEvent,
    WriteToolResultEvent,
    GrepToolCallEvent,
    GrepToolResultEvent,
    FindToolCallEvent,
    FindToolResultEvent,
    LsToolCallEvent,
    LsToolResultEvent
);

const _: fn(
    maestro_app::BuildSystemPromptOptions,
) -> maestro_models::system_prompt::BuildSystemPromptOptions = |value| value;
const _: fn(maestro_app::ContextFile) -> maestro_models::system_prompt::ContextFile = |value| value;
const _: fn(&maestro_app::ToolResultEvent) -> bool = maestro_app::is_bash_tool_result;
const _: fn(&maestro_app::ToolResultEvent) -> bool = maestro_app::is_read_tool_result;
const _: fn(&maestro_app::ToolResultEvent) -> bool = maestro_app::is_edit_tool_result;
const _: fn(&maestro_app::ToolResultEvent) -> bool = maestro_app::is_write_tool_result;
const _: fn(&maestro_app::ToolResultEvent) -> bool = maestro_app::is_grep_tool_result;
const _: fn(&maestro_app::ToolResultEvent) -> bool = maestro_app::is_find_tool_result;
const _: fn(&maestro_app::ToolResultEvent) -> bool = maestro_app::is_ls_tool_result;

#[test]
fn hook_tool_exports_retain_their_domain_types() {
    let _: Option<maestro_app::AgentToolResult> = None::<maestro_agent::AgentToolResult>;
    let _: Option<maestro_app::AgentToolUpdateCallback> =
        None::<maestro_agent::AgentToolUpdateCallback>;
    let _: maestro_app::ToolExecutionMode = maestro_agent::ToolExecutionMode::Sequential;
}
