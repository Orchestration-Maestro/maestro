//! Custom summaries through the shipped component, native runner and controlled provider.
#![cfg(test)]
use maestro_credentials::{ApiKeyCredential, AuthCredential};
use maestro_extensions::{
    ExtensionRunner, ExtensionUIContext, Local, SessionBeforeCompactEvent, create_event_bus_for,
    create_extension_runtime, load_extension_from_factory,
    types::{ExtensionUIContextPort, NotificationType},
};
use maestro_models::*;
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex, OnceLock, RwLock},
};

/// Notification host that never opens a terminal.
#[derive(Default)]
struct Ui(RefCell<Vec<(String, Option<NotificationType>)>>);
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn notify(
        &self,
        message: String,
        kind: Option<NotificationType>,
    ) -> Result<(), maestro_extensions::EventError> {
        self.0.borrow_mut().push((message, kind));
        Ok(())
    }
}

/// Captures the real model request before returning a deterministic response.
#[derive(Default)]
struct Provider(Mutex<Vec<Value>>, &'static str);
impl<E: ProviderExecution> Invoke<E, ProviderCall<E, ProviderStreamOptions<E>>> for Provider {
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, ProviderStreamOptions<E>>) -> Self::Return {
        if self.1 == "cancel" {
            call.options
                .as_ref()
                .unwrap()
                .common
                .signal
                .as_ref()
                .expect("compaction signal")
                .abort();
        }
        self.0.lock().unwrap().push(json!({"model":call.model,"context":call.context,"options":call.options.map(|options| options.common)}));
        if self.1 == "reject" {
            return Err(DiagnosticErrorInfo::new(None, "controlled failure"));
        }
        let reason = match self.1 {
            "provider_error" => StopReason::Error,
            "aborted_response" => StopReason::Aborted,
            _ => StopReason::Stop,
        };
        let content = match self.1 {
            "empty" => vec![AssistantContent::Text(faux_text(" \u{feff}\n\t"))],
            "thinking" => vec![AssistantContent::Thinking(faux_thinking("not a summary"))],
            _ => vec![AssistantContent::Text(faux_text(
                "# Summary\nAll work retained",
            ))],
        };
        let stream = AssistantMessageEventStream::new();
        let message = Arc::new(RwLock::new(faux_assistant_message(
            content,
            FauxAssistantMessageOptions {
                stop_reason: Some(reason.clone()),
                error_message: (reason == StopReason::Error).then(|| "controlled failure".into()),
                ..Default::default()
            },
        )));
        stream.push(match reason {
            StopReason::Error => AssistantMessageEvent::Error {
                reason: ErrorReason::Error,
                error: message,
            },
            StopReason::Aborted => AssistantMessageEvent::Error {
                reason: ErrorReason::Aborted,
                error: message,
            },
            _ => AssistantMessageEvent::Done {
                reason: DoneReason::Stop,
                message,
            },
        });
        Ok(stream)
    }
}

impl<E: ProviderExecution> Invoke<E, ProviderCall<E, SimpleStreamOptions<E>>> for Provider {
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, _: ProviderCall<E, SimpleStreamOptions<E>>) -> Self::Return {
        panic!("custom compaction uses raw completion")
    }
}

/// Builds the one shipped source once per integration-test process.
fn component() -> &'static Path {
    static PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        super::extension_guide::cargo::component(
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../maestro-extensions-wasm"
            )),
            &["--example", "custom_compaction"],
            "custom_compaction",
        )
        .unwrap()
    })
}

/// A compiled extension over a real, in-memory catalog and session.
struct Harness {
    /// Runs the registered callback, not a private helper.
    runner: ExtensionRunner<Local>,
    /// The application history remains untouched until it accepts a result.
    history: Rc<RefCell<maestro_session::SessionManager>>,
    /// Receives notifications without terminal effects.
    ui: Rc<Ui>,
    /// The same mutable catalog the live context reads.
    catalog: Arc<maestro_catalog::ModelRegistry>,
    /// Unhandled callback failures must not masquerade as intentional fallback.
    errors: Rc<RefCell<Vec<String>>>,
}
impl Harness {
    /// Registers a controlled Google descriptor and binds the compiled extension.
    async fn new(api: &str) -> Self {
        let (session, history) = super::session::runner_session("/custom-compaction");
        let config = serde_json::from_value(json!({
            "baseUrl":"https://unused.invalid", "apiKey":"controlled-key", "api":api,
            "headers":{"x-controlled":"retained"},
            "models":[{"id":"gemini-2.5-flash","name":"Flash","reasoning":false,"input":["text"],
                "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":200_000,"maxTokens":8192}]
        })).unwrap();
        session
            .model_registry
            .register_provider("google", Arc::new(RwLock::new(config)))
            .unwrap();
        let auth = AuthCredential::ApiKey(ApiKeyCredential {
            key: "controlled-key".into(),
        });
        session.model_registry.auth_storage().set("google", auth);
        let catalog = session.model_registry.clone();
        history
            .borrow_mut()
            .append_message(json!({"role":"user","content":"history must remain","timestamp":1}))
            .unwrap();
        let runtime = create_extension_runtime::<Local>();
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no bus errors")),
        );
        let path = component().to_str().unwrap().to_owned();
        let owner = load_extension_from_factory(
            crate::artifact(path.clone()),
            "/custom-compaction".into(),
            bus.for_extension(),
            runtime.clone(),
            Some(path),
        )
        .await
        .unwrap();
        let runner = ExtensionRunner::<Local>::new(
            Rc::new(RwLock::new(vec![owner])),
            runtime,
            session,
            Rc::new(|_| {}),
            maestro_theme::LiveTheme::default(),
        );
        super::support::bind_prompt(&runner, "controlled compaction");
        let ui = Rc::new(Ui::default());
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        let errors = Rc::new(RefCell::new(Vec::new()));
        let reported = errors.clone();
        let _remove = runner.on_error(Rc::new(move |error| {
            reported.borrow_mut().push(format!("{error:?}"));
            Ok(())
        }));
        Self {
            runner,
            history,
            ui,
            catalog,
            errors,
        }
    }
}

/// Supplied split-turn spans, previous context and retention boundary.
fn event() -> SessionBeforeCompactEvent {
    SessionBeforeCompactEvent {
        data: serde_json::from_value(json!({"preparation":{
            "firstKeptEntryId":"keep-this-entry", "messagesToSummarize":[{"role":"user","content":"older goal","timestamp":1}],
            "turnPrefixMessages":[{"role":"user","content":"split turn detail","timestamp":2}],
            "isSplitTurn":true,"tokensBefore":12_345,"previousSummary":"previous decisions",
            "fileOps":{"read":[],"written":[],"edited":[]},
            "settings":{"enabled":true,"reserveTokens":8192,"keepRecentTokens":20_000}
        },"branchEntries":[]})).unwrap(),
        signal: Cancellation::new(),
    }
}

/// Checks that the handoff retains every instruction needed to resume the session.
fn assert_handoff_instructions(prompt: &str) {
    for instruction in [
        "Write a session handoff from the conversation below.",
        "Earlier session context:\nprevious decisions",
        "- Goals the session is working toward.",
        "- Decisions and the reasons behind them.",
        "- Relevant code edits, affected files and technical details.",
        "- Work in progress and its current status.",
        "- Blockers, unresolved issues and unanswered questions.",
        "- Planned or proposed actions to take next.",
        "This summary replaces the full conversation history.",
        "Keep everything needed to resume the work, without unnecessary detail.",
        "Use Markdown headings to organize the handoff.",
    ] {
        assert!(
            prompt.contains(instruction),
            "missing instruction: {instruction}"
        );
    }
}

#[test]
fn custom_compaction_summarizes_both_spans_and_preserves_retention() {
    super::components::local(async {
        let provider = Arc::new(Provider::default());
        register_api_provider(
            ApiProvider {
                api: "custom-compaction-success".into(),
                stream: provider.clone(),
                stream_simple: provider.clone(),
            },
            Some("custom-compaction-success".into()),
        );
        let harness = Harness::new("custom-compaction-success").await;
        let before = serde_json::to_value(harness.history.borrow().get_entries()).unwrap();
        let mut event = event();
        let result = harness
            .runner
            .emit(&mut event)
            .await
            .unwrap()
            .expect("custom result");
        let result = serde_json::to_value(result).unwrap();
        assert_eq!(
            result,
            json!({"compaction":{"summary":"# Summary\nAll work retained","firstKeptEntryId":"keep-this-entry","tokensBefore":12_345.0}})
        );
        assert_eq!(
            serde_json::to_value(harness.history.borrow().get_entries()).unwrap(),
            before
        );
        let requests = provider.0.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["model"]["id"], "gemini-2.5-flash");
        let prompt = requests[0]["context"]["messages"][0]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert!(prompt.contains("previous decisions"));
        assert!(prompt.find("older goal").unwrap() < prompt.find("split turn detail").unwrap());
        assert_handoff_instructions(prompt);
        assert_eq!(requests[0]["options"]["maxTokens"], 8192.0);
        assert_eq!(requests[0]["options"]["apiKey"], "controlled-key");
        assert_eq!(
            requests[0]["options"]["headers"]["x-controlled"],
            "retained"
        );
        assert_eq!(
            *harness.ui.0.borrow(),
            vec![
                (
                    "Custom summary requested for compaction".into(),
                    Some(NotificationType::Info)
                ),
                (
                    "Summarizing 2 messages (12345 tokens) with gemini-2.5-flash".into(),
                    Some(NotificationType::Info)
                ),
            ]
        );
        unregister_api_providers("custom-compaction-success");
    });
}

/// Supplies missing prerequisites using only the catalog's existing public operations.
fn prerequisites(harness: &Harness, case: &str) {
    if case == "missing_model" {
        harness.catalog.get_all().write().unwrap().retain(|model| {
            let model = model.read().unwrap();
            model.provider != "google" || model.id != "gemini-2.5-flash"
        });
    }
    if matches!(case, "missing_auth" | "auth_failed") {
        harness.catalog.auth_storage().set(
            "google",
            AuthCredential::ApiKey(ApiKeyCredential { key: String::new() }),
        );
    }
    if case == "auth_failed" {
        let config =
            serde_json::from_value(json!({"apiKey":"controlled-key","authHeader":true})).unwrap();
        harness
            .catalog
            .register_provider("google", Arc::new(RwLock::new(config)))
            .unwrap();
    }
}

/// Observes fallback, diagnostics, cancellation and history through the real runner.
async fn fallback(case: &'static str) -> Value {
    let provider = Arc::new(Provider(Mutex::default(), case));
    register_api_provider(
        ApiProvider {
            api: "custom-compaction-fallback".into(),
            stream: provider.clone(),
            stream_simple: provider.clone(),
        },
        Some("custom-compaction-fallback".into()),
    );
    let harness = Harness::new("custom-compaction-fallback").await;
    prerequisites(&harness, case);
    let history = serde_json::to_value(harness.history.borrow().get_entries()).unwrap();
    let mut event = event();
    let boundary = (
        event.preparation.first_kept_entry_id.clone(),
        event.preparation.tokens_before,
    );
    if case == "pre_cancel" {
        event.signal.abort();
    }
    let result = harness.runner.emit(&mut event).await.unwrap();
    let notices: Vec<_> = harness
        .ui
        .0
        .borrow()
        .iter()
        .filter(|(_, kind)| {
            matches!(
                kind,
                Some(NotificationType::Warning | NotificationType::Error)
            )
        })
        .map(|(text, kind)| json!([kind, text]))
        .collect();
    let observed = json!({
        "case":case,"fallback":result.is_none(),"calls":provider.0.lock().unwrap().len(),
        "notices":notices,"errors":harness.errors.borrow().len(),"cancelled":event.signal.is_aborted(),
        "historyUnchanged":history == serde_json::to_value(harness.history.borrow().get_entries()).unwrap(),
        "boundaryUnchanged":boundary == (event.preparation.first_kept_entry_id.clone(), event.preparation.tokens_before),
    });
    unregister_api_providers("custom-compaction-fallback");
    observed
}

#[test]
fn custom_compaction_unusable_summaries_fall_back_without_editing_history() {
    super::components::local(async {
        let mut observed = Vec::new();
        let mut expected = Vec::new();
        let missing_model = json!([[
            "warning",
            "Summary model gemini-2.5-flash is unavailable; keeping default compaction"
        ]]);
        let missing_auth = json!([[
            "warning",
            "Summary provider google has no API key; keeping default compaction"
        ]]);
        let auth_failed = json!([[
            "warning",
            "Cannot authenticate the summary request: No API key found for \"google\""
        ]]);
        let empty = json!([[
            "warning",
            "Summary contains no text; keeping default compaction"
        ]]);
        let failed = json!([[
            "error",
            "Cannot create the custom summary: controlled failure"
        ]]);
        for (case, calls, notices, cancelled) in [
            ("missing_model", 0, missing_model, false),
            ("missing_auth", 0, missing_auth, false),
            ("auth_failed", 0, auth_failed, false),
            ("empty", 1, empty.clone(), false),
            ("thinking", 1, empty, false),
            ("provider_error", 1, failed.clone(), false),
            ("reject", 1, failed, false),
            ("cancel", 1, json!([]), true),
            ("aborted_response", 1, json!([]), false),
            ("pre_cancel", 0, json!([]), true),
        ] {
            observed.push(fallback(case).await);
            expected.push(
                json!({"case":case,"fallback":true,"calls":calls,"notices":notices,
                "errors":0,"cancelled":cancelled,"historyUnchanged":true,"boundaryUnchanged":true}),
            );
        }
        assert_eq!(observed, expected);
    });
}
