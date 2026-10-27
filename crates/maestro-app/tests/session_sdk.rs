//! Creating a session with its defaults: the working directory, the restored or chosen
//! model and thinking level, the first history entries, the provider request options and
//! hooks, withheld images and both resource routes, compared with the recorded SDK cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Auth, Harness, HarnessOptions, TestResourceLoader, answer, as_recorded, assistant,
    create_test_extensions_result, create_test_resource_loader, harness, on_task_set,
    recorded_number, script_responses, summary_provider, usage,
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
use maestro_session::session_manager::{CompactionOptions, ReadonlySessionManager};
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
        offline: None,
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
        assert_eq!(as_recorded(observed, &[(&root, "$ROOT")]), expected);
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
        assert_eq!(as_recorded(observed, &[(&root, "$ROOT")]), expected);
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

/// A harness serving `faux-1` whose model reports `window` context tokens.
async fn windowed_fixture(window: f64) -> Harness {
    harness(HarnessOptions {
        models: vec![("faux-1", true)],
        auth: Auth::Key,
        context_window: Some(window),
        ..HarnessOptions::default()
    })
    .await
}

/// A stored history with one answered question whose usage is `usage` and, when asked, a
/// compaction keeping that answer.
fn saved_history(cwd: &str, usage: &Value, compaction: bool) -> SessionManager {
    let mut manager = SessionManager::in_memory(Some(cwd)).unwrap();
    manager
        .append_message(json!({"role": "user", "content": "saved question", "timestamp": 0}))
        .unwrap();
    let answer = manager
        .append_message(assistant(
            "saved answer",
            0,
            &json!({"model": "faux-1", "usage": usage}),
        ))
        .unwrap();
    manager.append_model_change("faux", "faux-1").unwrap();
    if compaction {
        manager.branch(&answer).unwrap();
        manager
            .append_compaction("Saved summary", &answer, 13.0, CompactionOptions::default())
            .unwrap();
    }
    manager
}

/// Creates the session over `manager` with default preferences.
async fn open(h: &Harness, manager: SessionManager) -> CreateAgentSessionResult {
    let mut options = options(h, json!({}));
    options.session_manager = Some(Rc::new(RefCell::new(manager)));
    create(options).await
}

/// The live messages as stored JSON records.
fn live_records(created: &CreateAgentSessionResult) -> Vec<Value> {
    let messages = created.session.messages();
    let messages = messages.read().unwrap();
    messages
        .iter()
        .map(|message| serde_json::to_value(message).unwrap())
        .collect()
}

/// The session's context tokens and percentage.
fn context(created: &CreateAgentSessionResult) -> (Option<f64>, Option<f64>) {
    let usage = created.session.get_context_usage().unwrap();
    let usage = usage.expect("a context window");
    (usage.tokens, usage.percent)
}

/// A saved history whose compaction is followed by one answer carrying `extra`.
fn after_compaction(cwd: &str, extra: &Value) -> SessionManager {
    let mut manager = saved_history(cwd, &usage([7, 3, 2, 1], 1.0, None), true);
    manager
        .append_message(json!({"role": "user", "content": "later question", "timestamp": 1000}))
        .unwrap();
    manager
        .append_message(assistant("later answer", 1000, extra))
        .unwrap();
    manager
}

#[test]
fn saved_history_readers_keep_missing_totals() {
    on_task_set(|_| async {
        let h = windowed_fixture(100.0).await;
        saved_totals(&h).await;
        answers_after_compaction(&h).await;
    });
}

/// Restoring a saved answer without a total keeps the live record absent and reports the
/// category totals to the context, statistics and stored history readers.
async fn saved_totals(h: &Harness) {
    let missing = usage([7, 3, 2, 1], 1.0, None);
    let created = open(h, saved_history(&h.cwd, &missing, false)).await;
    let records = live_records(&created);
    assert_eq!(
        records[1]["usage"].get("totalTokens"),
        None,
        "{}",
        records[1]
    );
    assert_eq!(context(&created), (Some(13.0), Some(13.0)));
    let stats = created.session.get_session_stats().unwrap();
    assert_eq!(
        (
            stats.user_messages,
            stats.assistant_messages,
            stats.total_messages
        ),
        (1, 1, 2)
    );
    let tokens = &stats.tokens;
    assert_eq!(
        (
            tokens.input,
            tokens.output,
            tokens.cache_read,
            tokens.cache_write,
            tokens.total,
            stats.cost
        ),
        (7.0, 3.0, 2.0, 1.0, 13.0, 1.0)
    );

    let supplied = open(
        h,
        saved_history(&h.cwd, &usage([7, 3, 2, 1], 1.0, Some(97)), false),
    )
    .await;
    assert_eq!(context(&supplied), (Some(97.0), Some(97.0)));
    assert_eq!(
        supplied
            .session
            .get_session_stats()
            .unwrap()
            .tokens
            .total
            .to_bits(),
        13.0_f64.to_bits()
    );

    let compacted = open(h, saved_history(&h.cwd, &missing, true)).await;
    assert_eq!(context(&compacted), (None, None));
}

/// A stored compaction hides the usage of the answer it kept until a later answer reports
/// its own; a zero, aborted or failed answer leaves it unknown.
async fn answers_after_compaction(h: &Harness) {
    let answered = after_compaction(
        &h.cwd,
        &json!({"stopReason": "stop", "usage": usage([13, 0, 0, 0], 1.0, None)}),
    );
    assert_eq!(context(&open(h, answered).await), (Some(13.0), Some(13.0)));
    for (stop, usage, error) in [
        ("stop", usage([0, 0, 0, 0], 0.0, None), None),
        ("aborted", usage([0, 0, 0, 0], 0.0, None), None),
        ("error", usage([0, 0, 0, 0], 0.0, None), Some("failed")),
    ] {
        let mut extra = json!({"stopReason": stop, "usage": usage});
        if let Some(error) = error {
            extra["errorMessage"] = json!(error);
        }
        let created = open(h, after_compaction(&h.cwd, &extra)).await;
        assert_eq!(context(&created), (None, None), "{stop}");
    }
}

#[test]
fn saved_large_history_compacts_with_recorded_tail() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            models: vec![("faux-1", true)],
            auth: Auth::Key,
            context_window: Some(200_000.0),
            ..HarnessOptions::default()
        })
        .await;
        let fixture = format!(
            "{}/../maestro-session/tests/fixtures/session_history_large.jsonl",
            env!("CARGO_MANIFEST_DIR")
        );
        let path = h.scratch.path().join("saved.jsonl");
        std::fs::copy(&fixture, &path).unwrap();
        let manager = SessionManager::open(path.to_str().unwrap(), None, None).unwrap();
        let extensions =
            create_test_extensions_result(vec![(None, summary_provider("compacted"))], &h.cwd)
                .await;
        let mut options = options(
            &h,
            json!({"compaction": {"reserveTokens": 16384, "keepRecentTokens": 20000}}),
        );
        options.resources =
            SessionResources::Loader(Rc::new(create_test_resource_loader(Some(extensions))));
        options.session_manager = Some(Rc::new(RefCell::new(manager)));
        let created = create(options).await;
        let before = live_records(&created);
        assert_eq!(before.len(), 914);
        let omitted = before
            .iter()
            .filter(|record| {
                record["role"] == "assistant" && record["usage"].get("totalTokens").is_none()
            })
            .count();
        assert_eq!(omitted, 453);
        let _answered = script_responses(&h, vec![answer("prefix summary", None, None)]);
        let result = created.session.compact(None).await.unwrap();
        assert_eq!(result.tokens_before.to_bits(), 177_657.0_f64.to_bits());
        let after = live_records(&created);
        assert_eq!(after.len(), 182);
        assert_eq!(after[1..], before[before.len() - 181..]);
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

/// Exact-spelled request headers captured before protocol normalization.
type AttributionHeaders = indexmap::IndexMap<String, String>;

/// One request through the session-created attribution wrapper.
struct AttributionCase {
    /// Provider identity used by the selected descriptor.
    provider: String,
    /// Descriptor address, independent of provider identity.
    base_url: String,
    /// Explicit install telemetry preference.
    telemetry_enabled: bool,
    /// Resolved credential headers.
    credential_headers: Option<AttributionHeaders>,
    /// Headers supplied by this invocation.
    request_headers: Option<AttributionHeaders>,
}

/// The complete header map the provider receives for `case`'s one request, sent through the
/// request function a created session installs.
async fn capture_attribution_headers(case: AttributionCase) -> Option<AttributionHeaders> {
    let h = fixture(Auth::Key).await;
    let mut descriptor = h.model("faux-1").read().unwrap().clone();
    descriptor.provider.clone_from(&case.provider);
    descriptor.base_url = case.base_url;
    let config = maestro_catalog::ProviderConfigInput {
        api_key: Some("faux-key".into()),
        headers: case.credential_headers,
        ..maestro_catalog::ProviderConfigInput::default()
    };
    h.registry
        .register_provider(&case.provider, Arc::new(std::sync::RwLock::new(config)))
        .unwrap();
    let model = Arc::new(std::sync::RwLock::new(descriptor));
    let mut setup = options(
        &h,
        json!({"enableInstallTelemetry": case.telemetry_enabled}),
    );
    setup.model = Some(Arc::clone(&model));
    let created = create(setup).await;
    let captured = Arc::new(Mutex::new(Vec::new()));
    let capturing = Arc::clone(&captured);
    observe_next(&h, move |input| {
        capturing
            .lock()
            .unwrap()
            .push(common(input).headers.clone());
    });
    let agent = created.session.agent();
    let stream = Rc::clone(agent.stream_fn.read().unwrap().as_ref().unwrap());
    let request = maestro_models::SimpleStreamOptions {
        common: StreamOptions {
            headers: case.request_headers,
            ..StreamOptions::default()
        },
        ..maestro_models::SimpleStreamOptions::default()
    };
    let selected = model.read().unwrap().clone();
    let context = maestro_models::Context {
        system_prompt: None,
        messages: Vec::new(),
        tools: None,
    };
    let response = stream(
        selected,
        context,
        request,
        maestro_models::ProviderStreamOptions::default(),
    )
    .await
    .unwrap();
    let _answer = response.result().await;
    let mut captured = captured.lock().unwrap();
    assert_eq!(
        captured.len(),
        1,
        "the provider receives exactly one request"
    );
    captured.pop().unwrap()
}

/// Sends each recorded attribution case `ids` names from a child without the telemetry
/// variable and compares the presence and the whole map of the provider's headers.
fn check_attribution_cases(ids: &[&str]) {
    crate::isolated_home::with_environment_without(
        std::thread::current().name().unwrap(),
        &[],
        &["MAESTRO_TELEMETRY"],
        |_| {
            on_task_set(|_| async {
                for id in ids {
                    let row = crate::support::session_harness::case("attribution", id);
                    let input = AttributionCase {
                        provider: row["model"]["provider"].as_str().unwrap().to_owned(),
                        base_url: row["model"]["baseUrl"].as_str().unwrap().to_owned(),
                        telemetry_enabled: row["settings"]["enableInstallTelemetry"]
                            .as_bool()
                            .unwrap(),
                        credential_headers: serde_json::from_value(
                            row["credentialHeaders"].clone(),
                        )
                        .unwrap(),
                        request_headers: serde_json::from_value(row["requestHeaders"].clone())
                            .unwrap(),
                    };
                    let read = json!({
                        "name": id,
                        "model": {"provider": input.provider, "baseUrl": input.base_url},
                        "settings": {"enableInstallTelemetry": input.telemetry_enabled},
                        "credentialHeaders": input.credential_headers,
                        "requestHeaders": input.request_headers,
                        "expected": row["expected"],
                    });
                    assert_eq!(read, row, "{id} holds only the fields the case reads");
                    let actual = capture_attribution_headers(input).await;
                    assert_eq!(
                        json!({"present": actual.is_some(), "headers": actual}),
                        row["expected"],
                        "{id}"
                    );
                }
            });
        },
    );
}

#[test]
fn disables_attribution_without_losing_other_headers() {
    check_attribution_cases(&[
        "disabled_routed",
        "disabled_cloudflare",
        "disabled_all_maps_absent",
    ]);
}

#[test]
fn attributes_workers_ai_without_a_routed_url() {
    check_attribution_cases(&["workers_provider"]);
}

#[test]
fn attributes_gateway_provider_without_a_routed_url() {
    check_attribution_cases(&["gateway_provider"]);
}

#[test]
fn attributes_routed_provider_with_or_without_a_matching_url() {
    check_attribution_cases(&["routed_provider", "routed_provider_and_url"]);
}

#[test]
fn merges_attribution_auth_and_caller_headers() {
    check_attribution_cases(&["routed_url", "routed_url_defaults"]);
}

#[test]
fn attributes_cloudflare_api_url_for_unrelated_providers() {
    check_attribution_cases(&["cloudflare_api_url", "cloudflare_url_substring"]);
}

#[test]
fn attributes_cloudflare_gateway_url_for_unrelated_providers() {
    check_attribution_cases(&["cloudflare_gateway_url", "gateway_url_substring"]);
}

#[test]
fn leaves_unmatched_requests_without_attribution() {
    check_attribution_cases(&[
        "unmatched_preserves_headers",
        "provider_comparison_is_exact",
        "url_comparison_is_case_sensitive",
        "empty_unmatched_url",
    ]);
}

#[test]
fn selects_routed_attribution_before_cloudflare_matches() {
    check_attribution_cases(&[
        "routed_provider_precedes_cloudflare_url",
        "routed_url_precedes_cloudflare_provider",
    ]);
}

#[test]
fn caller_headers_win_after_credential_and_attribution_headers() {
    check_attribution_cases(&["three_layer_precedence", "provider_and_request_overrides"]);
}

#[test]
fn cloudflare_user_agent_respects_credential_and_caller_precedence() {
    check_attribution_cases(&[
        "cloudflare_credential_precedence",
        "cloudflare_caller_precedence",
    ]);
}

#[test]
fn keeps_header_names_distinct_at_application_merge() {
    check_attribution_cases(&["application_header_keys_are_exact"]);
}

#[test]
fn preserves_absent_and_explicit_empty_request_headers() {
    check_attribution_cases(&[
        "all_maps_absent",
        "empty_credential_map",
        "empty_request_map",
    ]);
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

/// Default construction and reload share the application's original tool services.
#[test]
fn session_tool_services_survive_resource_reload() {
    on_task_set(|local| async move {
        let h = fixture(Auth::Key).await;
        std::fs::create_dir_all(&h.cwd).unwrap();
        let bin_dir = format!("{}/selected-bin", h.scratch.path().display());
        let manager = Arc::new(
            maestro_tools::ToolsManager::new(bin_dir.clone(), None).with_offline(Some(true)),
        );
        let seen = Rc::new(RefCell::new(0));
        let (selected, bin, calls) = (manager.clone(), bin_dir.clone(), seen.clone());
        let factory: ExtensionFactory<Local> = Rc::new(move |api| {
            let services = api.tool_construction().unwrap();
            assert!(Arc::ptr_eq(&services.tools_manager, &selected));
            assert_eq!(services.bin_dir, bin);
            *calls.borrow_mut() += 1;
            Box::pin(std::future::ready(Ok(())))
        });
        let module_loader: maestro_extensions::ModuleLoader<Local> =
            Rc::new(|_| Box::pin(std::future::ready(Ok(None))));
        let mut resources = ResourceLoaderOptions::new(local, module_loader);
        resources.tool_construction = Some(maestro_extensions::ToolConstruction {
            tools_manager: manager.clone(),
            bin_dir: bin_dir.clone(),
        });
        resources.extension_factories.push(factory);
        let mut options = options(&h, json!({}));
        options.cwd = Some(h.cwd.clone());
        options.resources = SessionResources::Default(Box::new(resources));
        options.model = Some(h.model("faux-1"));
        let created = create(options).await;
        let services = created
            .extensions_result
            .runtime
            .tool_construction()
            .unwrap();
        assert!(Arc::ptr_eq(&services.tools_manager, &manager));
        assert_eq!(services.bin_dir, bin_dir);
        assert_eq!(*seen.borrow(), 1);
        created.session.reload().await.unwrap();
        assert_eq!(*seen.borrow(), 2);
    });
}
