//! Custom summaries through the shipped component, native runner and controlled provider.
#![cfg(test)]
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
struct Provider(Mutex<Vec<Value>>);
impl<E: ProviderExecution> Invoke<E, ProviderCall<E, ProviderStreamOptions<E>>> for Provider {
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, ProviderStreamOptions<E>>) -> Self::Return {
        self.0.lock().unwrap().push(json!({"model":call.model,"context":call.context,"options":call.options.map(|options| options.common)}));
        let stream = AssistantMessageEventStream::new();
        let message = Arc::new(RwLock::new(faux_assistant_message(
            "# Summary\nAll work retained",
            FauxAssistantMessageOptions::default(),
        )));
        stream.push(AssistantMessageEvent::Done {
            reason: DoneReason::Stop,
            message,
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
        Self {
            runner,
            history,
            ui,
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
        assert!(prompt.contains("ENTIRE conversation history"));
        assert_eq!(requests[0]["options"]["maxTokens"], 8192.0);
        assert_eq!(requests[0]["options"]["apiKey"], "controlled-key");
        assert_eq!(
            requests[0]["options"]["headers"]["x-controlled"],
            "retained"
        );
        assert!(
            harness
                .ui
                .0
                .borrow()
                .iter()
                .any(|(text, _)| text.contains("2 messages"))
        );
        unregister_api_providers("custom-compaction-success");
    });
}
