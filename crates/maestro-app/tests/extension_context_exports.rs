//! Base extension context records through the application root.
use maestro_app::{CompactOptions, ContextUsage, ExtensionContext, ExtensionContextActions};
use maestro_app::{ExtensionActions, ExtensionError, ExtensionRuntime, create_extension_runtime};
use maestro_catalog::ModelRegistry;
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_extensions::ExtensionRunner;
use maestro_extensions::runner::ExtensionRunnerSession;
use maestro_extensions::{Extension, ExtensionAPI, ExtensionErrorListener, ExtensionExecution};
use maestro_models::Threaded;
use maestro_session::SessionManager;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// The default value of an operand whose type the application does not name,
/// such as the runner's live theme.
fn default_operand<T: Default>() -> T {
    T::default()
}

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

#[test]
fn native_command_context_exports_share_the_extension_records() {
    let runner = runner();
    let actions = maestro_app::ExtensionCommandContextActions {
        new_session: Rc::new(|_| {
            Box::pin(async {
                Ok(maestro_extensions::types::SessionChangeResult { cancelled: true })
            })
        }),
        ..Default::default()
    };
    runner.bind_command_context(Some(actions));
    let ctx: maestro_app::ExtensionCommandContext = runner.create_command_context();
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            assert!(ctx.new_session(None).await.unwrap().cancelled);
        });
}

/// Builds the real in-memory runner shared by application context export witnesses.
fn runner() -> ExtensionRunner {
    let auth = Arc::new(AuthStorage::in_memory(AuthStorageData::default()));
    let session = ExtensionRunnerSession {
        cwd: "/work".into(),
        session_manager: Rc::new(RefCell::new(SessionManager::in_memory(None).unwrap())),
        model_registry: maestro_models::SharedHandle::new(ModelRegistry::in_memory(auth).unwrap()),
    };
    ExtensionRunner::<maestro_models::Threaded>::new(
        Arc::new(RwLock::new(Vec::new())),
        maestro_extensions::create_extension_runtime::<maestro_models::Threaded>(),
        session,
        Arc::new(|_| {}),
        default_operand(),
    )
}

#[test]
fn native_runtime_exports_share_the_extension_records() {
    let runtime: Arc<ExtensionRuntime> = create_extension_runtime::<Threaded>();
    let mut actions: ExtensionActions = Threaded::unbound_actions();
    let tool = runtime_tool();
    let expected = serde_json::to_value(&tool).unwrap();
    actions.get_all_tools = Arc::new(move || Ok(vec![tool.clone()]));
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
fn runtime_tool() -> maestro_app::ToolInfo {
    let source = serde_json::from_value(serde_json::json!({"path":"/extension","source":"cli","scope":"temporary","origin":"top-level"})).unwrap();
    maestro_app::ToolInfo {
        name: "inspect".into(),
        description: "Inspect data".into(),
        parameters: serde_json::json!({"type":"object"}),
        source_info: source,
    }
}

/// A real provider model routed through the embedding SDK.
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
