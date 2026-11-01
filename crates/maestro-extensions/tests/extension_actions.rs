//! Runtime actions through retained public extension facades.
#![cfg(not(target_arch = "wasm32"))]
#![cfg(test)]
use maestro_catalog::ModelRegistry;
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_extensions::{
    EventError, ExtensionAPI, ExtensionContextActions, ExtensionExecution,
    create_extension_runtime,
    runner::ExtensionRunner,
    types::{
        ExtensionActions, ExtensionError, ExtensionRuntime, ProviderActions, ProviderConfig,
        ProviderModelConfig,
    },
};
use maestro_models::{
    Local, ModelCost, ModelInput, ModelThinkingLevel as ThinkingLevel, SharedHandle, Threaded,
    ToolInfo,
};
use maestro_session::messages::CustomMessageInput;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

const NOT_INITIALIZED: &str =
    "Extension runtime not initialized. Action methods cannot be called during extension loading.";

pub(crate) use crate::session::NoCommands;

pub(crate) fn catalog() -> SharedHandle<ModelRegistry> {
    let auth = SharedHandle::new(AuthStorage::in_memory(AuthStorageData::default()));
    SharedHandle::new(ModelRegistry::in_memory(auth, SharedHandle::new(NoCommands)).unwrap())
}

pub(crate) fn good(id: &str) -> SharedHandle<RwLock<ProviderConfig>> {
    SharedHandle::new(RwLock::new(ProviderConfig {
        base_url: Some("https://provider.test/v1".into()),
        api_key: Some("PROVIDER_TEST_KEY".into()),
        api: Some("openai-completions".into()),
        models: Some(vec![ProviderModelConfig {
            id: id.into(),
            name: "Instant Model".into(),
            api: None,
            base_url: None,
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost::default(),
            context_window: 128_000.0,
            max_tokens: 4096.0,
            headers: None,
            compat: None,
        }]),
        ..ProviderConfig::default()
    }))
}

fn source() -> Arc<RwLock<maestro_extensions::SourceInfo>> {
    Arc::new(RwLock::new(
        serde_json::from_value(serde_json::json!({
            "path":"/ext/a.ts", "source":"cli", "scope":"temporary", "origin":"top-level"
        }))
        .unwrap(),
    ))
}
fn local_api(
    runtime: Rc<ExtensionRuntime<Local>>,
    cwd: &str,
) -> (
    maestro_extensions::types::ExtensionHandle<Local>,
    ExtensionAPI<Local>,
) {
    let owner = Rc::new(RwLock::new(maestro_extensions::Extension::new(
        "/ext/a.ts".into(),
        "/ext/a.ts".into(),
        source(),
    )));
    let events = maestro_extensions::create_event_bus_for::<Local>(Rc::new(drop), Rc::new(|_| {}))
        .for_extension();
    let api = ExtensionAPI::new(&owner, runtime, events, cwd.into());
    (owner, api)
}
fn threaded_api(
    runtime: Arc<ExtensionRuntime<Threaded>>,
) -> (
    maestro_extensions::types::ExtensionHandle<Threaded>,
    ExtensionAPI<Threaded>,
) {
    let owner = Arc::new(RwLock::new(maestro_extensions::Extension::new(
        "/ext/a.ts".into(),
        "/ext/a.ts".into(),
        source(),
    )));
    let events =
        maestro_extensions::create_event_bus_for::<Threaded>(Arc::new(drop), Arc::new(|_| {}))
            .for_extension();
    let api = ExtensionAPI::new(&owner, runtime, events, "/work".into());
    (owner, api)
}

fn session(
    registry: SharedHandle<ModelRegistry>,
) -> maestro_extensions::runner::ExtensionRunnerSession {
    maestro_extensions::runner::ExtensionRunnerSession {
        cwd: "/work".into(),
        session_manager: Rc::new(RefCell::new(
            maestro_session::SessionManager::in_memory(Some("/work")).unwrap(),
        )),
        model_registry: registry,
    }
}
pub(crate) fn runner_local(
    runtime: Rc<ExtensionRuntime<Local>>,
    registry: SharedHandle<ModelRegistry>,
) -> ExtensionRunner<Local> {
    ExtensionRunner::<Local>::new(
        Rc::default(),
        runtime,
        session(registry),
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    )
}
fn runner_threaded(
    runtime: Arc<ExtensionRuntime<Threaded>>,
    registry: SharedHandle<ModelRegistry>,
) -> ExtensionRunner<Threaded> {
    ExtensionRunner::<Threaded>::new(
        Arc::default(),
        runtime,
        session(registry),
        Arc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    )
}
fn send_sync<T: Send + Sync>() {}

/// Local actions that capture single-thread state and record every call.
fn local_actions(log: &Rc<RefCell<Vec<String>>>) -> ExtensionActions<Local> {
    let mut actions = Local::unbound_actions();
    let l = Rc::clone(log);
    actions.send_message = Rc::new(move |m: CustomMessageInput, o| {
        l.borrow_mut()
            .push(format!("send:{}:{}", m.custom_type, o.is_some()));
        Ok(())
    });
    let l = Rc::clone(log);
    actions.get_session_name = Rc::new(move || {
        l.borrow_mut().push("name".into());
        Ok(Some("named".into()))
    });
    actions.set_model = Rc::new(|_| Box::pin(async { Ok(true) }));
    actions.get_thinking_level = Rc::new(|| Ok(ThinkingLevel::Off));
    actions
}

#[test]
fn loading_actions_fail_recoverably_but_tool_refresh_is_inert() {
    if maestro_test_process::run_alone("loading_actions_fail_recoverably_but_tool_refresh_is_inert")
        .unwrap()
    {
        return;
    }
    let runtime = create_extension_runtime::<Local>();
    let (_owner, api) = local_api(runtime.clone(), "/work");
    action_checks::loading(&api, &runtime);
}

#[test]
fn action_families_keep_local_captures_and_native_thread_safety() {
    if maestro_test_process::run_alone(
        "action_families_keep_local_captures_and_native_thread_safety",
    )
    .unwrap()
    {
        return;
    }
    action_checks::local_future();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runtime = create_extension_runtime::<Local>();
    let runner = runner_local(Rc::clone(&runtime), catalog());
    runner
        .bind_core(
            local_actions(&log),
            ExtensionContextActions::default(),
            ProviderActions::default(),
        )
        .unwrap();
    let (_owner, api) = local_api(runtime, "/work");
    api.send_message(
        CustomMessageInput {
            custom_type: "note".into(),
            content: maestro_models::UserContent::Text("hi".into()),
            display: true,
            details: maestro_session::Presence::Missing,
        },
        None,
    )
    .unwrap();
    assert_eq!(api.get_session_name().unwrap().as_deref(), Some("named"));
    assert_eq!(api.get_thinking_level().unwrap(), ThinkingLevel::Off);
    assert_eq!(*log.borrow(), ["send:note:false", "name"]);
    send_sync::<ExtensionRuntime<Threaded>>();
    send_sync::<ExtensionActions<Threaded>>();
    send_sync::<ProviderActions<Threaded>>();
    send_sync::<ExtensionAPI<Threaded>>();
    send_sync::<ModelRegistry>();
    let threaded = create_extension_runtime::<Threaded>();
    let mut actions = Threaded::unbound_actions();
    actions.get_session_name = Arc::new(|| Ok(Some("remote".into())));
    let runner = runner_threaded(Arc::clone(&threaded), catalog());
    runner
        .bind_core(
            actions,
            ExtensionContextActions::default(),
            ProviderActions::default(),
        )
        .unwrap();
    let (_owner, api) = threaded_api(threaded);
    let name = std::thread::spawn(move || api.get_session_name().unwrap())
        .join()
        .unwrap();
    assert_eq!(name.as_deref(), Some("remote"));
}

#[test]
fn queued_provider_override_preserves_oauth_message_and_stack() {
    let runtime = create_extension_runtime::<Local>();
    let runner = runner_local(runtime.clone(), catalog());
    let errors = Rc::new(RefCell::new(Vec::new()));
    let observed = errors.clone();
    let _off = runner.on_error(Rc::new(move |error| {
        observed.borrow_mut().push(error.clone());
        Ok(())
    }));
    runtime
        .register_provider("oauth".into(), good("oauth"), Some("/ext/oauth.ts".into()))
        .unwrap();
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions::default(),
            ProviderActions {
                register_provider: Some(Rc::new(|_, _| {
                    let mut diagnostic =
                        maestro_models::DiagnosticErrorInfo::error("login refused");
                    diagnostic.stack = Some("authored OAuth stack".into());
                    Err(Box::new(maestro_models::OAuthError::from(diagnostic)))
                })),
                unregister_provider: None,
            },
        )
        .unwrap();
    assert_eq!(
        *errors.borrow(),
        [ExtensionError {
            extension_path: "/ext/oauth.ts".into(),
            event: "register_provider".into(),
            error: "login refused".into(),
            stack: Some("authored OAuth stack".into()),
        }]
    );
}

#[test]
fn provider_queue_keeps_order_attribution_and_partial_success() {
    if maestro_test_process::run_alone("provider_queue_keeps_order_attribution_and_partial_success")
        .unwrap()
    {
        return;
    }
    let runtime = create_extension_runtime::<Local>();
    let registry = catalog();
    let runner = Rc::new(runner_local(
        Rc::clone(&runtime),
        SharedHandle::clone(&registry),
    ));
    let (_owner, api) = local_api(Rc::clone(&runtime), "/work");
    let api = Rc::new(api);
    let seen = Rc::new(RefCell::new(Vec::<String>::new()));
    let (s, r, a) = (Rc::clone(&seen), Rc::clone(&runtime), Rc::clone(&api));
    let off = runner.on_error(late_listener(s, r, a));
    let nobase = good("x");
    nobase.write().unwrap().base_url = None;
    runtime
        .register_provider("first-bad".into(), nobase, Some("/ext/c.ts".into()))
        .unwrap();
    runtime
        .register_provider("after".into(), good("after"), None)
        .unwrap();
    assert_eq!(
        runtime.pending_provider_registrations.read().unwrap().len(),
        2
    );
    let log = Rc::new(RefCell::new(Vec::new()));
    runner
        .bind_core(
            local_actions(&log),
            ExtensionContextActions::default(),
            ProviderActions::default(),
        )
        .unwrap();
    assert_eq!(
        *seen.borrow(),
        ["/ext/c.ts: Provider first-bad: \"baseUrl\" is required when defining models. [named]"]
    );
    assert!(
        runtime
            .pending_provider_registrations
            .read()
            .unwrap()
            .is_empty()
    );
    assert!(registry.find("after", "after").is_some());
    assert!(registry.find("late", "late").is_some());
    verify_immediate_after_flush(&api, &registry, &runner, off, &seen);
    action_checks::queued_validations();
}

#[test]
fn bound_provider_actions_and_catalog_fallback_take_effect_immediately() {
    if maestro_test_process::run_alone(
        "bound_provider_actions_and_catalog_fallback_take_effect_immediately",
    )
    .unwrap()
    {
        return;
    }
    action_checks::provider_modes();
}

#[test]
#[cfg(unix)]
fn exec_runs_in_the_extension_directory_unless_overridden() {
    if maestro_test_process::run_alone("exec_runs_in_the_extension_directory_unless_overridden")
        .unwrap()
    {
        return;
    }
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let dir = directory.path();
    let runtime = create_extension_runtime::<Local>();
    let (_owner, api) = local_api(runtime, &dir.display().to_string());
    let executor = crate::runtime_support::io_runtime();
    let args: Vec<String> = Vec::new();
    let out = executor
        .block_on(
            api.exec("pwd", &args, None, maestro_tools::ExecOptions::default())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(out.stdout, format!("{}\n", dir.display()));
    let out = executor
        .block_on(
            api.exec(
                "pwd",
                &args,
                Some("/"),
                maestro_tools::ExecOptions::default(),
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(out.stdout, "/\n");
    action_checks::exec_outputs(&api);
}

#[test]
fn tool_metadata_keeps_native_schemas_and_guest_schema_text() {
    if maestro_test_process::run_alone("tool_metadata_keeps_native_schemas_and_guest_schema_text")
        .unwrap()
    {
        return;
    }
    let source = serde_json::json!({"path": "<builtin>", "source": "builtin", "scope": "temporary", "origin": "top-level", "baseDir":"/builtin"});
    let guest: maestro_models::ToolInfo<String> = serde_json::from_value(serde_json::json!({
        "name": "read", "description": "Read", "parameters": "{\"type\":\"object\"}", "sourceInfo": source
    }))
    .unwrap();
    assert_eq!(
        serde_json::to_value(&guest).unwrap()["parameters"],
        "{\"type\":\"object\"}"
    );
    let native: ToolInfo<maestro_models::ParameterSchema<maestro_models::SchemaCode>> = serde_json::from_value(serde_json::json!({
        "name": "read", "description": "Read", "parameters": {"type": "object"}, "sourceInfo": source
    }))
    .unwrap();
    let runtime = create_extension_runtime::<Local>();
    let runner = runner_local(runtime.clone(), catalog());
    let (_owner, api) = local_api(runtime, "/work");
    let mut actions = Local::unbound_actions();
    actions.get_all_tools = Rc::new(move || Ok(vec![native.clone()]));
    runner
        .bind_core(
            actions,
            ExtensionContextActions::default(),
            ProviderActions::default(),
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(api.get_all_tools().unwrap()).unwrap(),
        serde_json::json!([{"name":"read","description":"Read","parameters":{"type":"object"},"sourceInfo":source}])
    );
}

#[test]
fn failed_error_delivery_keeps_only_failed_and_unattempted_registrations() {
    if maestro_test_process::run_alone(
        "failed_error_delivery_keeps_only_failed_and_unattempted_registrations",
    )
    .unwrap()
    {
        return;
    }
    let runtime = create_extension_runtime::<Local>();
    let registry = catalog();
    let runner = runner_local(Rc::clone(&runtime), registry);
    runtime
        .register_provider("succeeded".into(), good("ok"), None)
        .unwrap();
    runtime
        .register_provider("bad".into(), good("bad"), None)
        .unwrap();
    runtime
        .register_provider("after".into(), good("after"), None)
        .unwrap();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let o = Rc::clone(&observed);
    let provider = ProviderActions::<Local> {
        register_provider: Some(Rc::new(move |name, _| {
            o.borrow_mut().push(name.clone());
            if name == "bad" {
                Err("bad config".into())
            } else {
                Ok(())
            }
        })),
        unregister_provider: None,
    };
    let off = runner.on_error(Rc::new(|_| Err("listener refused".into())));
    assert_eq!(
        bind_empty(&runner, provider.clone())
            .unwrap_err()
            .to_string(),
        "listener refused"
    );
    assert_eq!(pending_names(&runtime), ["bad", "after"]);
    runtime
        .register_provider("queued-after-abort".into(), good("queued"), None)
        .unwrap();
    assert_eq!(
        pending_names(&runtime),
        ["bad", "after", "queued-after-abort"]
    );
    runtime.unregister_provider("queued-after-abort").unwrap();
    off();
    bind_empty(&runner, provider).unwrap();
    assert_eq!(*observed.borrow(), ["succeeded", "bad", "bad", "after"]);
    assert!(
        runtime
            .pending_provider_registrations
            .read()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn queued_provider_callbacks_edit_the_live_pending_work() {
    if maestro_test_process::run_alone("queued_provider_callbacks_edit_the_live_pending_work")
        .unwrap()
    {
        return;
    }
    action_checks::queue_edits();
    let runtime = create_extension_runtime::<Local>();
    let registry = catalog();
    let runner = runner_local(Rc::clone(&runtime), SharedHandle::clone(&registry));
    let bad = good("bad");
    bad.write().unwrap().base_url = None;
    runtime.register_provider("bad".into(), bad, None).unwrap();
    runtime
        .register_provider("later".into(), good("later"), None)
        .unwrap();
    let r = Rc::clone(&runtime);
    let _off = runner.on_error(Rc::new(move |_| {
        r.unregister_provider("later")?;
        r.register_provider("pushed".into(), good("pushed"), None)
    }));
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions::default(),
            ProviderActions::default(),
        )
        .unwrap();
    assert!(registry.find("later", "later").is_none());
    assert!(registry.find("pushed", "pushed").is_some());
}
#[test]
fn actions_are_bound_before_queued_providers_flush() {
    if maestro_test_process::run_alone("actions_are_bound_before_queued_providers_flush").unwrap() {
        return;
    }
    action_checks::binding_order();
}
#[test]
fn retained_facades_follow_the_live_owner_and_release_it() {
    if maestro_test_process::run_alone("retained_facades_follow_the_live_owner_and_release_it")
        .unwrap()
    {
        return;
    }
    let runtime = create_extension_runtime::<Local>();
    let (owner, api) = local_api(Rc::clone(&runtime), "/work");
    owner.write().unwrap().path = "/changed/owner.ts".into();
    api.register_provider("p".into(), good("p")).unwrap();
    assert_eq!(
        runtime
            .pending_provider_registrations
            .read()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .extension_path,
        "/changed/owner.ts"
    );
    let weak = Rc::downgrade(&owner);
    drop(owner);
    assert!(weak.upgrade().is_none());
    assert!(api.get_session_name().is_err());
}

fn late_listener(
    s: Rc<RefCell<Vec<String>>>,
    r: Rc<ExtensionRuntime<Local>>,
    a: Rc<ExtensionAPI<Local>>,
) -> maestro_extensions::types::ExtensionErrorListener {
    Rc::new(move |e: &ExtensionError| {
        s.borrow_mut().push(format!(
            "{}: {} [{}]",
            e.extension_path,
            e.error,
            a.get_session_name().unwrap().unwrap()
        ));
        if e.error.contains("first-bad") {
            r.register_provider("late".into(), good("late"), Some("/ext/late.ts".into()))
                .unwrap();
        }
        Ok(())
    })
}

fn pending_names(runtime: &ExtensionRuntime<Local>) -> Vec<String> {
    runtime
        .pending_provider_registrations
        .read()
        .unwrap()
        .values()
        .map(|p| p.name.clone())
        .collect()
}

#[test]
fn catalog_getter_and_constructor_share_one_direct_owner() {
    if maestro_test_process::run_alone("catalog_getter_and_constructor_share_one_direct_owner")
        .unwrap()
    {
        return;
    }
    let runtime = create_extension_runtime::<Local>();
    let registry = catalog();
    let runner = runner_local(Rc::clone(&runtime), SharedHandle::clone(&registry));
    let context = runner.create_context();
    assert!(SharedHandle::ptr_eq(
        &registry,
        &runner.create_command_context().model_registry().unwrap()
    ));
    let retained = context.model_registry().unwrap();
    let (_owner, api) = local_api(runtime, "/work");
    bind_empty(&runner, ProviderActions::default()).unwrap();
    api.register_provider("fallback".into(), good("fallback-model"))
        .unwrap();
    assert!(retained.find("fallback", "fallback-model").is_some());
    retained.unregister_provider("fallback").unwrap();
    assert!(registry.find("fallback", "fallback-model").is_none());
    assert!(SharedHandle::ptr_eq(
        &registry,
        &context.model_registry().unwrap()
    ));
    context
        .model_registry()
        .unwrap()
        .register_provider("p", good("m"))
        .unwrap();
    assert!(registry.find("p", "m").is_some());
    runner.invalidate(Some("stale".into()));
    assert_eq!(context.model_registry().err().unwrap().to_string(), "stale");
    retained.refresh().unwrap();
    assert!(retained.find("p", "m").is_some());
}

#[path = "support/bound_catalog.rs"]
mod bound_catalog;

#[path = "support/action_checks.rs"]
mod action_checks;

#[test]
fn prebind_unregister_removes_every_matching_provider() {
    if maestro_test_process::run_alone("prebind_unregister_removes_every_matching_provider")
        .unwrap()
    {
        return;
    }
    action_checks::prebind();
}

#[test]
fn action_callbacks_preserve_all_arguments_and_return_values() {
    if maestro_test_process::run_alone("action_callbacks_preserve_all_arguments_and_return_values")
        .unwrap()
    {
        return;
    }
    action_checks::forwarding();
}

#[test]
fn stale_actions_fail_before_reaching_bound_operations() {
    if maestro_test_process::run_alone("stale_actions_fail_before_reaching_bound_operations")
        .unwrap()
    {
        return;
    }
    action_checks::stale();
}

#[test]
fn action_errors_keep_their_original_identity() {
    if maestro_test_process::run_alone("action_errors_keep_their_original_identity").unwrap() {
        return;
    }
    action_checks::errors();
}

#[test]
fn error_listeners_follow_registration_order_and_unsubscribe() {
    if maestro_test_process::run_alone("error_listeners_follow_registration_order_and_unsubscribe")
        .unwrap()
    {
        return;
    }
    action_checks::listeners();
}

#[test]
fn rebinding_updates_actions_without_retargeting_started_model_selection() {
    if maestro_test_process::run_alone(
        "rebinding_updates_actions_without_retargeting_started_model_selection",
    )
    .unwrap()
    {
        return;
    }
    action_checks::rebind();
}

#[test]
fn catalog_callbacks_can_read_and_register_during_a_bound_registration() {
    if maestro_test_process::run_alone(
        "catalog_callbacks_can_read_and_register_during_a_bound_registration",
    )
    .unwrap()
    {
        return;
    }
    bound_catalog::catalog_callbacks_can_read_and_register_during_a_bound_registration();
}

#[test]
fn bound_provider_configuration_reaches_stream_oauth_and_request_consumers() {
    if maestro_test_process::run_alone(
        "bound_provider_configuration_reaches_stream_oauth_and_request_consumers",
    )
    .unwrap()
    {
        return;
    }
    bound_catalog::bound_provider_configuration_reaches_stream_oauth_and_request_consumers();
}

/// Binds an otherwise inert runtime for provider-only scenarios.
fn bind_empty(
    runner: &ExtensionRunner<Local>,
    providers: ProviderActions<Local>,
) -> Result<(), EventError> {
    runner.bind_core(
        Local::unbound_actions(),
        ExtensionContextActions::default(),
        providers,
    )
}

fn verify_immediate_after_flush(
    api: &ExtensionAPI<Local>,
    registry: &ModelRegistry,
    runner: &ExtensionRunner<Local>,
    off: impl Fn(),
    seen: &Rc<RefCell<Vec<String>>>,
) {
    api.register_provider("post".into(), good("post")).unwrap();
    assert!(registry.find("post", "post").is_some());
    api.unregister_provider("post").unwrap();
    assert!(registry.find("post", "post").is_none());
    off();
    runner
        .emit_error(&ExtensionError {
            extension_path: String::new(),
            event: String::new(),
            error: "ignored".into(),
            stack: None,
        })
        .unwrap();
    assert_eq!(seen.borrow().len(), 1);
}

use crate::context_actions;

use crate::model_support;
