//! Creating a session with its defaults: the working directory, the restored or chosen
//! model and thinking level, the first history entries, the provider request options and
//! hooks, withheld images and both resource routes, compared with the recorded SDK cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Auth, Harness, HarnessOptions, TestResourceLoader, create_test_extensions_result,
    create_test_resource_loader, harness, on_task_set, recorded_number,
};
use maestro_app::{
    CreateAgentSessionOptions, CreateAgentSessionResult, PromptOptions, ResourceLoaderOptions,
    SessionResources, create_agent_session,
};
use maestro_extensions::{
    AfterProviderResponseEvent, BeforeProviderRequestEvent, BeforeProviderRequestEventResult,
    ExtensionFactory,
};
use maestro_models::{
    AssistantMessage, DiagnosticErrorInfo, FauxResponseInput, FauxResponseStep, ImageContent,
    Local, ModelThinkingLevel, PayloadHookInput, ProviderResponse, StreamOptions,
    StreamOptionsInput,
};
use maestro_session::SessionManager;
use maestro_session::session_manager::ReadonlySessionManager;
use maestro_settings::SettingsManager;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

/// The recorded SDK case `name`.
fn expected(name: &str) -> Value {
    crate::support::session_harness::corpus("sdk")[name]["ok"].clone()
}

/// A harness whose scripted provider serves the reasoning model `faux-1` as `auth` says.
async fn fixture(auth: Auth) -> Harness {
    harness(HarnessOptions {
        models: vec![("faux-1", true)],
        auth,
        ..HarnessOptions::default()
    })
    .await
}

/// Creation options over the fixture's catalog, in-memory history at `cwd` and `settings`,
/// with the test resource loader and every other input absent.
fn options(h: &Harness, settings: Value) -> CreateAgentSessionOptions {
    let settings = serde_json::from_value(settings).unwrap();
    CreateAgentSessionOptions {
        cwd: None,
        agent_dir: Some(h.scratch.path().display().to_string()),
        auth_storage: None,
        model_registry: Some(Arc::clone(&h.registry)),
        model: None,
        thinking_level: None,
        scoped_models: Vec::new(),
        no_tools: None,
        tools: None,
        custom_tools: Vec::new(),
        resources: SessionResources::Loader(Rc::new(create_test_resource_loader(None))),
        session_manager: Some(Rc::new(RefCell::new(
            SessionManager::in_memory(Some(&h.cwd)).unwrap(),
        ))),
        settings_manager: Some(Rc::new(RefCell::new(SettingsManager::in_memory(settings)))),
        session_start_event: None,
        theme: maestro_theme::LiveTheme::default(),
    }
}

/// An in-memory history holding one exchange answered by `provider`/`model_id`, its model
/// change and, when given, a thinking change.
fn seeded(cwd: &str, provider: &str, model_id: &str, thinking: Option<&str>) -> SessionManager {
    let mut manager = SessionManager::in_memory(Some(cwd)).unwrap();
    manager
        .append_message(json!({"role": "user", "content": "seed question", "timestamp": 0}))
        .unwrap();
    manager
        .append_message(json!({
            "role": "assistant", "content": [{"type": "text", "text": "seed answer"}],
            "api": "faux", "provider": provider, "model": model_id, "stopReason": "stop",
            "timestamp": 0,
            "usage": {"input": 1, "output": 1, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 2,
                "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}},
        }))
        .unwrap();
    manager.append_model_change(provider, model_id).unwrap();
    if let Some(level) = thinking {
        manager.append_thinking_level_change(level).unwrap();
    }
    manager
}

/// Creates the session.
async fn create(options: CreateAgentSessionOptions) -> CreateAgentSessionResult {
    create_agent_session(options).await.unwrap()
}

/// `provider/id` of the session's model.
fn selected(created: &CreateAgentSessionResult) -> String {
    let model = created.session.model();
    let model = model.read().unwrap();
    format!("{}/{}", model.provider, model.id)
}

/// The types of the session's stored entries.
fn entry_types(created: &CreateAgentSessionResult) -> Value {
    let manager = created.session.session_manager().borrow();
    json!(
        manager
            .get_entries()
            .into_iter()
            .map(|entry| entry["type"].clone())
            .collect::<Vec<_>>()
    )
}

/// The session's working directory, as its extension contexts report it.
fn session_cwd(created: &CreateAgentSessionResult) -> String {
    let context = created.session.extension_runner().create_context();
    context.cwd().unwrap().to_owned()
}

#[test]
fn resolves_the_working_directory_from_the_explicit_option() {
    let expected = expected("cwd_explicit_option_wins");
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let root = h.scratch.path().display().to_string();
        let mut options = options(&h, json!({}));
        options.cwd = Some(format!("{root}/explicit-cwd"));
        let ignored = SessionManager::in_memory(Some(&format!("{root}/ignored-cwd"))).unwrap();
        options.session_manager = Some(Rc::new(RefCell::new(ignored)));
        options.model = Some(h.model("faux-1"));
        let created = create(options).await;
        let manager_cwd = created
            .session
            .session_manager()
            .borrow()
            .get_cwd()
            .to_owned();
        let observed = json!({"manager_cwd": manager_cwd, "session_cwd": session_cwd(&created)});
        assert_eq!(
            observed.to_string().replace(&root, "$ROOT"),
            expected.to_string()
        );
    });
}

#[test]
fn resolves_the_working_directory_from_the_session_manager() {
    let expected = expected("cwd_from_supplied_session_manager");
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let root = h.scratch.path().display().to_string();
        let mut options = options(&h, json!({}));
        let manager = SessionManager::in_memory(Some(&format!("{root}/session-cwd"))).unwrap();
        options.session_manager = Some(Rc::new(RefCell::new(manager)));
        options.model = Some(h.model("faux-1"));
        let created = create(options).await;
        let manager_cwd = created
            .session
            .session_manager()
            .borrow()
            .get_cwd()
            .to_owned();
        let observed = json!({
            "manager_cwd": manager_cwd, "session_cwd": session_cwd(&created),
            "model_fallback_message": created.model_fallback_message,
        });
        assert_eq!(
            observed.to_string().replace(&root, "$ROOT"),
            expected.to_string()
        );
    });
}

#[test]
fn restores_an_authenticated_recorded_model() {
    let expected = expected("recorded_model_restored");
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let mut options = options(&h, json!({}));
        let manager = seeded(&h.cwd, "faux", "faux-1", None);
        options.session_manager = Some(Rc::new(RefCell::new(manager)));
        let created = create(options).await;
        let observed = json!({"model": selected(&created), "model_fallback_message": created.model_fallback_message});
        assert_eq!(observed, expected);
    });
}

#[test]
fn reports_the_recorded_model_fallback_message() {
    let expected = expected("recorded_model_without_credentials_falls_back");
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let settings = json!({"defaultProvider": "faux", "defaultModel": "faux-1"});
        let mut options = options(&h, settings);
        let manager = seeded(&h.cwd, "gone-provider", "gone-model", None);
        options.session_manager = Some(Rc::new(RefCell::new(manager)));
        let created = create(options).await;
        let provider = created.session.model().read().unwrap().provider.clone();
        let observed =
            json!({"model": provider, "model_fallback_message": created.model_fallback_message});
        assert_eq!(observed, expected);
    });
}

#[test]
fn falls_back_from_a_recorded_model_without_credentials() {
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let recorded = h.registry.find("openai-codex", "gpt-5.5").unwrap();
        assert!(!h.registry.has_configured_auth(&recorded).unwrap());
        let settings = json!({"defaultProvider": "faux", "defaultModel": "faux-1"});
        let mut options = options(&h, settings);
        let manager = seeded(&h.cwd, "openai-codex", "gpt-5.5", None);
        options.session_manager = Some(Rc::new(RefCell::new(manager)));
        let created = create(options).await;
        assert_eq!(
            created.model_fallback_message.as_deref(),
            Some("Could not restore model openai-codex/gpt-5.5. Using faux/faux-1")
        );
    });
}

#[test]
fn honours_an_explicit_model() {
    let expected = expected("explicit_model_wins");
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let mut options = options(&h, json!({}));
        let manager = seeded(&h.cwd, "gone-provider", "gone-model", None);
        options.session_manager = Some(Rc::new(RefCell::new(manager)));
        options.model = Some(h.model("faux-1"));
        let created = create(options).await;
        let observed = json!({"model": selected(&created), "model_fallback_message": created.model_fallback_message});
        assert_eq!(observed, expected);
    });
}

#[test]
fn starts_without_a_model_when_none_is_available() {
    on_task_set(|_| async {
        let h = fixture(Auth::None).await;
        let created = create(options(&h, json!({}))).await;
        assert_eq!(selected(&created), "unknown/unknown");
        assert_eq!(created.session.thinking_level(), ModelThinkingLevel::Off);
        let docs = maestro_app::configuration::config::get_docs_path().unwrap();
        let guidance = maestro_credentials::format_no_models_available_message(&docs);
        assert_eq!(created.model_fallback_message, Some(guidance));
        assert_eq!(entry_types(&created), json!(["thinking_level_change"]));
    });
}

/// The history a thinking case starts from.
enum History {
    /// A new history.
    New,
    /// A resumed exchange with the recorded model and, when given, a thinking change.
    Resumed(Option<&'static str>),
}

/// Creates a session for a thinking case over `history` with the explicit level and
/// `settings`.
async fn thinking_case(
    h: &Harness,
    explicit: Option<ModelThinkingLevel>,
    history: History,
    settings: Value,
) -> Value {
    let mut options = options(h, settings);
    if let History::Resumed(level) = history {
        let manager = seeded(&h.cwd, "faux", "faux-1", level);
        options.session_manager = Some(Rc::new(RefCell::new(manager)));
    }
    options.model = Some(h.model("faux-1"));
    options.thinking_level = explicit;
    let created = create(options).await;
    json!({"thinking_level": created.session.thinking_level(), "entry_types": entry_types(&created)})
}

#[test]
fn clamps_an_explicit_thinking_level() {
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let observed =
            thinking_case(&h, Some(ModelThinkingLevel::High), History::New, json!({})).await;
        assert_eq!(observed, expected("thinking_explicit_clamped"));
    });
}

#[test]
fn restores_a_recorded_thinking_level() {
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let observed = thinking_case(&h, None, History::Resumed(Some("low")), json!({})).await;
        assert_eq!(observed, expected("thinking_recorded_entry"));
    });
}

#[test]
fn appends_the_thinking_level_when_the_branch_has_none() {
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let settings = json!({"defaultThinkingLevel": "minimal"});
        let observed = thinking_case(&h, None, History::Resumed(None), settings).await;
        assert_eq!(
            observed,
            expected("thinking_recorded_without_entry_uses_settings")
        );
    });
}

#[test]
fn uses_the_settings_default_thinking_level() {
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let settings = json!({"defaultThinkingLevel": "low"});
        let observed = thinking_case(&h, None, History::New, settings).await;
        assert_eq!(observed, expected("thinking_settings_default"));
    });
}

#[test]
fn defaults_thinking_to_medium() {
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let observed = thinking_case(&h, None, History::New, json!({})).await;
        assert_eq!(observed, expected("thinking_defaults_to_medium"));
        assert_eq!(
            observed["entry_types"],
            json!(["model_change", "thinking_level_change"]),
            "the creation appends its model and thinking entries"
        );
    });
}

#[test]
fn restores_stored_messages_into_agent_state() {
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let mut options = options(&h, json!({}));
        let manager = seeded(&h.cwd, "faux", "faux-1", Some("low"));
        options.session_manager = Some(Rc::new(RefCell::new(manager)));
        let created = create(options).await;
        let messages = created.session.messages();
        let roles: Vec<Value> = messages
            .read()
            .unwrap()
            .iter()
            .map(|message| serde_json::to_value(message).unwrap()["role"].clone())
            .collect();
        assert_eq!(json!(roles), json!(["user", "assistant"]));
        assert_eq!(
            entry_types(&created),
            expected("thinking_recorded_entry")["entry_types"]
        );
    });
}

/// A scripted answer that hands each request to `observe`.
struct Observed<F>(F);

impl<F: Fn(&FauxResponseInput<Local>) + 'static>
    maestro_models::Invoke<Local, FauxResponseInput<Local>> for Observed<F>
{
    type Return = <Local as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, input: FauxResponseInput<Local>) -> Self::Return {
        (self.0)(&input);
        let answer = crate::support::session_harness::answer("ok", None, None);
        Box::pin(std::future::ready(Ok(answer)))
    }
}

impl<F>
    maestro_models::Invoke<maestro_models::Threaded, FauxResponseInput<maestro_models::Threaded>>
    for Observed<F>
{
    type Return = <maestro_models::Threaded as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, _: FauxResponseInput<maestro_models::Threaded>) -> Self::Return {
        unreachable!("the session requests through the local family")
    }
}

/// The common options of a request.
fn common(input: &FauxResponseInput<Local>) -> &StreamOptions<Local> {
    match input.options.as_ref().unwrap() {
        StreamOptionsInput::Simple(options) => &options.common,
        StreamOptionsInput::Raw(options) => &options.common,
    }
}

/// Answers the next request through `observe`.
fn observe_next(h: &Harness, observe: impl Fn(&FauxResponseInput<Local>) + Send + Sync + 'static) {
    h.faux
        .set_responses(vec![FauxResponseStep::Factory(Arc::new(Observed(observe)))]);
}

#[test]
fn carries_provider_retry_settings_into_the_request() {
    let expected = expected("request_options_from_provider_retry_settings");
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let settings = json!({"retry": {"provider": {"timeoutMs": 1234, "maxRetries": 7, "maxRetryDelayMs": 4321}}});
        let mut options = options(&h, settings);
        options.model = Some(h.model("faux-1"));
        let created = create(options).await;
        let captured = Arc::new(Mutex::new(Vec::new()));
        let capturing = Arc::clone(&captured);
        observe_next(&h, move |input| {
            let common = common(input);
            let number = |value: Option<f64>| value.map(recorded_number);
            capturing.lock().unwrap().push(json!({
                "api_key": common.api_key, "timeout_ms": number(common.timeout_ms),
                "max_retries": number(common.max_retries),
                "max_retry_delay_ms": number(common.max_retry_delay_ms),
                "headers": common.headers,
            }));
        });
        created
            .session
            .prompt("hi", PromptOptions::default())
            .await
            .unwrap();
        let observed = json!({"stream_options": *captured.lock().unwrap()});
        assert_eq!(observed, expected);
    });
}

#[test]
fn merges_attribution_auth_and_caller_headers() {
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let config = maestro_catalog::ProviderConfigInput {
            base_url: Some("https://openrouter.ai/api/v1".into()),
            api_key: Some("faux-key".into()),
            api: Some(h.faux.api.clone()),
            headers: Some(
                [
                    ("X-OpenRouter-Title", "provider title"),
                    ("X-Provider", "provider"),
                ]
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .into_iter()
                .collect(),
            ),
            models: Some(vec![maestro_catalog::ProviderModelInput {
                id: "routed".into(),
                name: "Routed".into(),
                api: Some(h.faux.api.clone()),
                base_url: Some("https://openrouter.ai/api/v1".into()),
                reasoning: false,
                thinking_level_map: None,
                input: Vec::new(),
                cost: maestro_models::ModelCost::default(),
                context_window: 1000.0,
                max_tokens: 100.0,
                headers: None,
                compat: None,
            }]),
            ..maestro_catalog::ProviderConfigInput::default()
        };
        h.registry
            .register_provider("faux", Arc::new(std::sync::RwLock::new(config)))
            .unwrap();
        let mut options = options(&h, json!({"enableInstallTelemetry": true}));
        options.model = h.registry.find("faux", "routed");
        let created = create(options).await;
        let captured = Arc::new(Mutex::new(None));
        let capturing = Arc::clone(&captured);
        observe_next(&h, move |input| {
            *capturing.lock().unwrap() = Some(json!(common(input).headers));
        });
        created
            .session
            .prompt("hi", PromptOptions::default())
            .await
            .unwrap();
        assert_eq!(
            captured.lock().unwrap().take().unwrap(),
            json!({
                "HTTP-Referer": "https://github.com/Orchestration-Maestro/maestro",
                "X-OpenRouter-Title": "provider title",
                "X-OpenRouter-Categories": "cli-agent",
                "X-Provider": "provider",
            })
        );
    });
}

/// An extension that replaces each payload with itself marked `injected` and records the
/// payload and response events into `seen`.
fn hooks(seen: Rc<RefCell<Vec<String>>>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let (before, after) = (Rc::clone(&seen), Rc::clone(&seen));
        let request = api.on::<BeforeProviderRequestEvent, BeforeProviderRequestEventResult>(
            "before_provider_request".into(),
            Rc::new(move |event, _| {
                before.borrow_mut().push("before".into());
                let mut payload = event.payload.clone();
                payload["injected"] = json!(true);
                Box::pin(std::future::ready(Ok(Some(payload))))
            }),
        );
        let response = api.on::<AfterProviderResponseEvent, ()>(
            "after_provider_response".into(),
            Rc::new(move |event, _| {
                after.borrow_mut().push(format!("after:{}", event.status));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        Box::pin(std::future::ready(request.and(response)))
    })
}

/// A scripted answer that runs the request's payload hook on `{"model": "faux-1"}`, keeps
/// what it forwards, then reports a 201 response, as the recorded case did.
struct HookCall {
    /// The payload the hook forwarded.
    forwarded: Arc<Mutex<Option<Value>>>,
}

impl maestro_models::Invoke<Local, FauxResponseInput<Local>> for HookCall {
    type Return = <Local as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, input: FauxResponseInput<Local>) -> Self::Return {
        let payload_hook = common(&input).on_payload.clone().unwrap();
        let response_hook = common(&input).on_response.clone().unwrap();
        let forwarded = Arc::clone(&self.forwarded);
        let model = Arc::new(input.model);
        Box::pin(async move {
            let request = PayloadHookInput {
                payload: json!({"model": "faux-1"}),
                signal: None,
            };
            let request = payload_hook(request, Arc::clone(&model)).await?;
            *forwarded.lock().unwrap() = Some(request.payload);
            let response = ProviderResponse {
                status: 201.0,
                headers: std::collections::BTreeMap::new(),
            };
            response_hook(response, model).await?;
            Ok(crate::support::session_harness::answer(
                "hooked", None, None,
            ))
        })
    }
}

impl maestro_models::Invoke<maestro_models::Threaded, FauxResponseInput<maestro_models::Threaded>>
    for HookCall
{
    type Return = <maestro_models::Threaded as maestro_models::Execution>::Future<
        'static,
        Result<AssistantMessage, DiagnosticErrorInfo>,
    >;
    fn invoke(&self, _: FauxResponseInput<maestro_models::Threaded>) -> Self::Return {
        unreachable!("the session requests through the local family")
    }
}

#[test]
fn forwards_before_and_after_provider_hooks() {
    let expected = crate::support::session_harness::corpus("request_hooks")["ok"].clone();
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let seen = Rc::new(RefCell::new(Vec::new()));
        let extensions =
            create_test_extensions_result(vec![(Some("<hooks>"), hooks(Rc::clone(&seen)))], &h.cwd)
                .await;
        let mut options = options(&h, json!({}));
        options.resources =
            SessionResources::Loader(Rc::new(create_test_resource_loader(Some(extensions))));
        options.model = Some(h.model("faux-1"));
        let created = create(options).await;
        let forwarded = Arc::new(Mutex::new(None));
        let call = HookCall {
            forwarded: Arc::clone(&forwarded),
        };
        h.faux
            .set_responses(vec![FauxResponseStep::Factory(Arc::new(call))]);
        created
            .session
            .prompt("hi", PromptOptions::default())
            .await
            .unwrap();
        assert_eq!(json!(*seen.borrow()), expected["hook_seen"]);
        assert_eq!(
            forwarded.lock().unwrap().take().unwrap(),
            expected["captures"][0]["forwarded"]
        );
        let history = created.session.messages();
        assert_eq!(
            json!(history.read().unwrap().len()),
            expected["message_count"]
        );
        assert_eq!(json!(h.faux.state.call_count()), expected["call_count"]);
    });
}

/// Prompts `look` with two images under `block_images` and returns the request's messages.
async fn image_request(block_images: bool) -> Vec<Value> {
    let h = fixture(Auth::Key).await;
    let mut options = options(&h, json!({"images": {"blockImages": block_images}}));
    options.model = Some(h.model("faux-1"));
    let created = create(options).await;
    let captured = Arc::new(Mutex::new(Vec::new()));
    let capturing = Arc::clone(&captured);
    observe_next(&h, move |input| {
        let messages = serde_json::to_value(&input.context.messages).unwrap();
        capturing
            .lock()
            .unwrap()
            .extend(messages.as_array().unwrap().clone());
    });
    let images = ["AA==", "BB=="].map(|data| ImageContent {
        data: data.into(),
        mime_type: "image/png".into(),
    });
    let prompt = PromptOptions {
        images: Some(images.to_vec()),
        ..PromptOptions::default()
    };
    created.session.prompt("look", prompt).await.unwrap();
    let messages = captured.lock().unwrap().clone();
    messages
        .into_iter()
        .map(|message| json!({"role": message["role"], "content": message["content"]}))
        .collect()
}

#[test]
fn collapses_consecutive_blocked_image_placeholders() {
    let expected = expected("block_images_filters_per_conversion");
    on_task_set(|_| async {
        assert_eq!(json!(image_request(true).await), expected["messages"]);
    });
}

#[test]
fn keeps_images_when_blocking_is_off() {
    let expected = expected("block_images_off_keeps_images");
    on_task_set(|_| async {
        let recorded = expected["messages"][0]["content"].as_array().unwrap();
        let images: Vec<Value> = recorded[1..]
            .iter()
            .map(|image| json!({"type": "image", "data": image["source"]["data"], "mimeType": image["source"]["mediaType"]}))
            .collect();
        let messages = image_request(false).await;
        assert_eq!(messages[0]["content"][0], recorded[0]);
        assert_eq!(
            json!(messages[0]["content"].as_array().unwrap()[1..]),
            json!(images)
        );
    });
}

#[test]
fn builds_the_default_resource_loader_and_loads_it() {
    on_task_set(|local| async move {
        let h = fixture(Auth::Key).await;
        std::fs::create_dir_all(&h.cwd).unwrap();
        std::fs::write(
            format!("{}/AGENTS.md", h.cwd),
            "Project rule: answer briefly.",
        )
        .unwrap();
        let module_loader: maestro_extensions::ModuleLoader<Local> =
            Rc::new(|_| Box::pin(std::future::ready(Ok(None))));
        let mut options = options(&h, json!({}));
        options.cwd = Some(h.cwd.clone());
        options.resources =
            SessionResources::Default(Box::new(ResourceLoaderOptions::new(local, module_loader)));
        options.model = Some(h.model("faux-1"));
        let created = create(options).await;
        assert!(
            created
                .session
                .system_prompt()
                .contains("Project rule: answer briefly."),
            "the loaded context file reached the prompt"
        );
    });
}

#[test]
fn uses_the_supplied_resource_loader_as_it_is() {
    on_task_set(|_| async {
        let h = fixture(Auth::Key).await;
        let loader = TestResourceLoader {
            system_prompt: Some(Rc::from("Supplied base prompt")),
            ..create_test_resource_loader(None)
        };
        let mut options = options(&h, json!({}));
        options.resources = SessionResources::Loader(Rc::new(loader));
        options.model = Some(h.model("faux-1"));
        let created = create(options).await;
        assert!(
            created
                .session
                .system_prompt()
                .starts_with("Supplied base prompt")
        );
    });
}
