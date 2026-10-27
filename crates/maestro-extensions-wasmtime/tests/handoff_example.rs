//! The shipped handoff command through its compiled component and native owners.
#![cfg(test)]
use maestro_extensions::{
    EventError, ExtensionCommandContextActions, ExtensionContextActions, ExtensionExecution,
    ExtensionRunner, ExtensionUIContext, Local, ReplacedSessionContext, create_event_bus_for,
    create_extension_runtime, load_extension_from_factory,
    types::{
        ComponentDisposer, CustomClose, CustomComponentFactory, CustomFactoryOutcome,
        CustomUIOptions, ExtensionUIContextPort, ExtensionUIFuture, NotificationType,
        ProviderActions,
    },
};
use maestro_models::*;
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};

/// A native provider with literal output and observations at the real request interface.
#[derive(Default)]
struct Provider {
    /// Requests include the original configured authentication and headers.
    requests: Mutex<Vec<Value>>,
    /// Produce an aborted terminal response instead of text.
    aborted: std::sync::atomic::AtomicBool,
    /// Admission signal for the controlled pending request.
    start: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    /// Native request retained until the test observes loader cancellation.
    pending: Mutex<Option<(Cancellation, AssistantMessageEventStream)>>,
}
impl<E: ProviderExecution, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>>
    for Provider
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        let options = match call.options.unwrap().into() {
            StreamOptionsInput::Raw(options) => options.common,
            StreamOptionsInput::Simple(_) => panic!("handoff uses raw completion"),
        };
        assert!(options.signal.is_some());
        self.requests
            .lock()
            .unwrap()
            .push(json!({"context":call.context,"key":options.api_key,"headers":options.headers}));
        let stream = AssistantMessageEventStream::new();
        if let Some(start) = self.start.lock().unwrap().take() {
            *self.pending.lock().unwrap() = Some((options.signal.unwrap(), stream.clone()));
            start.send(()).unwrap();
            return Ok(stream);
        }
        let mut response = faux_assistant_message(
            "## Context\nGenerated",
            FauxAssistantMessageOptions::default(),
        );
        response
            .content
            .push(AssistantContent::Thinking(ThinkingContent {
                thinking: "not part of the draft".into(),
                thinking_signature: None,
                redacted: None,
            }));
        response.content.push(AssistantContent::Text(TextContent {
            text: "## Task\nContinue".into(),
            text_signature: None,
        }));
        let aborted = self.aborted.load(std::sync::atomic::Ordering::SeqCst);
        if aborted {
            response.stop_reason = StopReason::Aborted;
            stream.push(AssistantMessageEvent::Error {
                reason: ErrorReason::Aborted,
                error: Arc::new(RwLock::new(response)),
            });
        } else {
            stream.push(AssistantMessageEvent::Done {
                reason: DoneReason::Stop,
                message: Arc::new(RwLock::new(response)),
            });
        }
        Ok(stream)
    }
}

/// A deterministic UI with real loader construction, supplied input and editor answers.
#[derive(Default)]
struct Ui {
    /// Records which independently bound context received each effect.
    calls: RefCell<Vec<Value>>,
    /// Escape the loader before completing generation.
    escape: Cell<bool>,
    /// Editor answer, including cancellation and a deliberately empty prompt.
    edited: RefCell<Option<String>>,
    /// Escape only after the provider has admitted the pending request.
    wait: RefCell<Option<tokio::sync::oneshot::Receiver<()>>>,
    /// Lets a controlled response wait until closing will dispose the installed loader.
    installed: RefCell<Option<tokio::sync::oneshot::Sender<()>>>,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::from_theme(Rc::new(
            maestro_theme::load_theme_from_path(
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../maestro-theme/assets/theme/dark.json"
                ),
                Some(maestro_theme::ColorMode::Truecolor),
            )
            .unwrap(),
        ))
    }
    fn notify(&self, text: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        self.calls.borrow_mut().push(json!(["notify", text, kind]));
        Ok(())
    }
    fn set_editor_text(&self, text: String) -> Result<(), EventError> {
        self.calls.borrow_mut().push(json!(["set", text]));
        Ok(())
    }
    fn editor(&self, title: String, prefill: Option<String>) -> ExtensionUIFuture<Option<String>> {
        self.calls
            .borrow_mut()
            .push(json!(["editor", title, prefill]));
        let edited = self.edited.borrow().clone();
        Box::pin(async move { Ok(edited) })
    }
    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        _: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        let (send, receive) = tokio::sync::oneshot::channel();
        let disposer = Rc::new(RefCell::new(None::<ComponentDisposer>));
        let close = loader_close(send, disposer.clone());
        let (tui, _) = super::canonical_ui::screen();
        let keys = maestro_extensions::widgets::ExtensionKeybindings {
            manager: maestro_tui::KeybindingsManager::new(
                maestro_tui::TUI_KEYBINDINGS.clone(),
                Vec::new(),
            ),
            reload: None,
        };
        let built = factory.call((tui, self.theme(), keys, close));
        let escape = self.escape.get();
        let wait = self.wait.borrow_mut().take();
        let installed = self.installed.borrow_mut().take();
        Box::pin(async move {
            let mut built = match built? {
                CustomFactoryOutcome::Ready(built) => built,
                CustomFactoryOutcome::Async(built) => built.await?,
            };
            assert!(
                built
                    .component
                    .try_render(80)
                    .unwrap()
                    .join("\n")
                    .contains("Generating handoff prompt...")
            );
            *disposer.borrow_mut() = built.dispose.take();
            if let Some(installed) = installed {
                installed.send(()).unwrap();
            }
            if let Some(wait) = wait {
                wait.await.unwrap();
            }
            if escape {
                built
                    .component
                    .input_handler()
                    .unwrap()
                    .try_handle_input("\x1b")
                    .unwrap();
            }
            receive
                .await
                .map_err(|error| EventError::from(error.to_string()))?;
            let dispose = disposer.borrow_mut().take();
            if let Some(dispose) = dispose {
                dispose.call(())?;
            }
            Ok(())
        })
    }
}

/// Settles once and disposes an installed loader synchronously, as the interactive host does.
fn loader_close(
    send: tokio::sync::oneshot::Sender<()>,
    disposer: Rc<RefCell<Option<ComponentDisposer>>>,
) -> CustomClose {
    let send = RefCell::new(Some(send));
    Rc::new(move || {
        let Some(send) = send.borrow_mut().take() else {
            return Ok(false);
        };
        let _ = send.send(());
        let dispose = disposer.borrow_mut().take();
        if let Some(dispose) = dispose {
            dispose.call(())?;
        }
        Ok(true)
    })
}

/// The model supplier varies independently of the configured registry.
type ModelSelection = Rc<RefCell<Option<Arc<RwLock<Model>>>>>;

/// Owns the real native runner and the observations supplied to its public interfaces.
struct Driver {
    /// The loaded component's session.
    runner: Rc<ExtensionRunner<Local>>,
    /// Writable history owned by the caller, not by the extension.
    history: Rc<RefCell<maestro_session::SessionManager>>,
    /// Optional current model.
    model: ModelSelection,
    /// Counts selection reads to prove headless validation runs first.
    model_reads: Rc<Cell<usize>>,
    /// Configured catalog used by the component.
    registry: SharedHandle<maestro_catalog::ModelRegistry>,
    /// Original and replacement UI owners.
    ui: Rc<Ui>,
    /// UI bound after the host's replacement attempt.
    replacement: Rc<Ui>,
    /// The replacement operation's parent arguments.
    parents: Rc<RefCell<Vec<Option<String>>>>,
    /// Caller-supplied veto.
    veto: Rc<Cell<bool>>,
    /// Host-owned event tails, awaited during teardown.
    tails: Rc<RefCell<Vec<tokio::task::JoinHandle<()>>>>,
    /// Controller keeps listener cleanup explicit.
    bus: maestro_extensions::EventBusController<Local>,
}
impl Driver {
    /// Loads the shipped component with no ambient credentials or filesystem effects.
    async fn new(api: &str) -> Self {
        let tails = Rc::new(RefCell::new(Vec::new()));
        let tasks = tails.clone();
        let bus = create_event_bus_for::<Local>(
            Rc::new(move |work| {
                tasks
                    .borrow_mut()
                    .push(tokio::task::spawn_local(async move {
                        work.await;
                    }));
            }),
            Rc::new(|error| panic!("unexpected tail error: {error:?}")),
        );
        let runtime = create_extension_runtime::<Local>();
        let path = std::env::var("MAESTRO_HANDOFF_COMPONENT").expect("nextest builds handoff");
        let owner = load_extension_from_factory(
            crate::artifact(path.clone()),
            "/handoff".into(),
            bus.for_extension(),
            runtime.clone(),
            Some(path),
        )
        .await
        .unwrap();
        let (session, history) = super::session::runner_session("/handoff");
        let registry = session.model_registry.clone();
        registry
            .auth_storage()
            .set_runtime_api_key("fixture", "controlled-only");
        let runner = Rc::new(ExtensionRunner::<Local>::new(
            Rc::new(RwLock::new(vec![owner])),
            runtime,
            session,
            Rc::new(|_| {}),
            maestro_theme::LiveTheme::default(),
        ));
        let (model, model_reads) = bind_model(&runner, api);
        let ui = Rc::new(Ui::default());
        *ui.edited.borrow_mut() = Some(" Edited\n雪 ".into());
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        let mut driver = Self {
            runner,
            history,
            model,
            model_reads,
            registry,
            ui,
            replacement: Rc::default(),
            parents: Rc::default(),
            veto: Rc::default(),
            tails,
            bus,
        };
        driver.bind_replacement();
        driver
    }
    /// Supplies replacement and invalidates the old context before invoking its continuation.
    fn bind_replacement(&mut self) {
        let (old, ui, parents, veto) = (
            Rc::downgrade(&self.runner),
            self.replacement.clone(),
            self.parents.clone(),
            self.veto.clone(),
        );
        let mut operations = super::context_actions::silent_command_operations();
        operations.new_session = Rc::new(move |options| {
            Box::pin(replace_session(
                options.unwrap(),
                old.clone(),
                ui.clone(),
                parents.clone(),
                veto.get(),
            ))
        });
        self.runner
            .bind_command_context(Some(ExtensionCommandContextActions::new(operations)));
    }
    /// Delivers the registered command, awaiting all admitted independent work afterwards.
    async fn command(&self, goal: &str) {
        let command = self.runner.get_command("handoff").unwrap().command;
        assert_eq!(
            command.description.as_deref(),
            Some("Transfer context to a new focused session")
        );
        (command.handler)(goal.into(), self.runner.create_command_context())
            .await
            .unwrap();
        let tails = self.tails.take();
        for tail in tails {
            tail.await.unwrap();
        }
    }
    /// Appends a literal user message through the real history owner.
    fn message(&self, text: &str) -> String {
        self.history
            .borrow_mut()
            .append_message(json!({"role":"user","content":text,"timestamp":1.0}))
            .unwrap()
    }
    /// Records compaction through the real session owner without changing the selected branch.
    fn compact(&self, summary: &str, kept: &str, tokens: f64) {
        self.history
            .borrow_mut()
            .append_compaction(
                summary,
                kept,
                tokens,
                maestro_session::session_manager::CompactionOptions::default(),
            )
            .unwrap();
    }
    /// Checks the single notification emitted by an early exit.
    fn notification(&self, text: &str, kind: &str) {
        assert_eq!(self.ui.calls.take(), [json!(["notify", text, kind])]);
        assert!(self.parents.borrow().is_empty());
    }
}
impl Drop for Driver {
    fn drop(&mut self) {
        self.bus.clear();
    }
}

/// Runs the host's supplied replacement operation, with no persistence or prompt submission.
async fn replace_session(
    options: maestro_extensions::types::NewSessionCommandOptions,
    old: std::rc::Weak<ExtensionRunner<Local>>,
    ui: Rc<Ui>,
    parents: Rc<RefCell<Vec<Option<String>>>>,
    veto: bool,
) -> Result<maestro_extensions::types::SessionChangeResult, EventError> {
    parents.borrow_mut().push(options.parent_session);
    if veto {
        old.upgrade()
            .unwrap()
            .set_ui_context(Some(ExtensionUIContext::new(ui)));
        return Ok(maestro_extensions::types::SessionChangeResult { cancelled: true });
    }
    old.upgrade().unwrap().invalidate(None);
    let fresh = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::default()),
        create_extension_runtime::<Local>(),
        super::session::runner_session("/replacement").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    fresh.set_ui_context(Some(ExtensionUIContext::new(ui)));
    let replacement = ReplacedSessionContext::new(
        fresh.create_command_context(),
        Rc::new(|_, _| panic!("draft is not sent")),
        Rc::new(|_, _| panic!("draft is not submitted")),
    );
    options.with_session.unwrap()(replacement).await?;
    Ok(maestro_extensions::types::SessionChangeResult { cancelled: false })
}

/// Supplies model reads independently of history, UI and configured credentials.
fn bind_model(runner: &ExtensionRunner<Local>, api: &str) -> (ModelSelection, Rc<Cell<usize>>) {
    let selected = super::model_support::model("configured-handoff");
    selected.write().unwrap().api = api.into();
    selected.write().unwrap().headers = Some([("x-handoff".into(), "configured".into())].into());
    let model = Rc::new(RefCell::new(Some(selected)));
    let read = model.clone();
    let model_reads = Rc::new(Cell::new(0));
    let reads = model_reads.clone();
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_model: Rc::new(move || {
                    reads.set(reads.get() + 1);
                    Ok(read.borrow().clone())
                }),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
    (model, model_reads)
}

/// Run one body on a real local executor, with a distinct registered provider per test.
fn run(name: &str, body: impl AsyncFnOnce(Arc<Provider>, Driver)) {
    let provider = Arc::new(Provider::default());
    register_api_provider(
        ApiProvider {
            api: name.into(),
            stream: provider.clone(),
            stream_simple: provider.clone(),
        },
        Some(name.into()),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        body(provider, Driver::new(name).await).await;
    });
    unregister_api_providers(name);
}

#[test]
fn handoff_requires_ui_model_goal_and_conversation() {
    run("handoff-guards", async |provider, driver| {
        driver.runner.set_ui_context(None);
        driver.command("goal").await;
        assert_eq!(driver.model_reads.get(), 0);
        assert!(provider.requests.lock().unwrap().is_empty());
        driver
            .runner
            .set_ui_context(Some(ExtensionUIContext::new(driver.ui.clone())));
        let model = driver.model.borrow_mut().take();
        driver.command("goal").await;
        driver.notification("No model selected", "error");
        *driver.model.borrow_mut() = model;
        driver.command(" \u{feff}\t ").await;
        driver.notification("Usage: /handoff <goal for new thread>", "error");
        driver
            .history
            .borrow_mut()
            .append_session_info("metadata only")
            .unwrap();
        driver.command("goal").await;
        driver.notification("No conversation to hand off", "error");
        assert!(provider.requests.lock().unwrap().is_empty());
    });
}

#[test]
fn handoff_reviews_draft_and_uses_replacement_context() {
    for edited in [" Edited\n雪 ", ""] {
        run("handoff-replacement", async move |provider, driver| {
            *driver.ui.edited.borrow_mut() = Some(edited.into());
            let scratch = maestro_test_directory::Scratch::new().unwrap();
            let parent = scratch.path().join("parent.jsonl").display().to_string();
            driver
                .history
                .borrow_mut()
                .set_session_file(&parent)
                .unwrap();
            driver.message("Discuss src/auth.rs");
            driver.command(" \u{feff}implement teams\u{feff} ").await;
            let calls = driver.ui.calls.borrow();
            assert_eq!(
                *calls,
                [json!([
                    "editor",
                    "Edit handoff prompt",
                    "## Context\nGenerated\n## Task\nContinue"
                ])]
            );
            assert_eq!(
                *driver.replacement.calls.borrow(),
                [
                    json!(["set", edited]),
                    json!(["notify", "Handoff ready. Submit when ready.", "info"])
                ]
            );
            assert_eq!(*driver.parents.borrow(), [Some(parent)]);
            let requests = provider.requests.lock().unwrap();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0]["key"], "controlled-only");
            assert_eq!(requests[0]["headers"]["x-handoff"], "configured");
            assert_eq!(
                requests[0]["context"]["messages"][0]["content"],
                json!([{"type":"text", "text":"## Conversation History\n\n[User]: Discuss src/auth.rs\n\n## User's Goal for New Thread\n\nimplement teams"}])
            );
            let prompt = requests[0]["context"]["systemPrompt"].as_str().unwrap();
            assert!(prompt.starts_with("You are a context transfer assistant."));
            assert!(
                prompt.ends_with("[Clear description of what to do next based on user's goal]")
            );
        });
    }
}

#[test]
fn handoff_delayed_completion_disposes_loader_synchronously() {
    run("handoff-delayed", async |provider, driver| {
        driver.message("history");
        let (start, admitted) = tokio::sync::oneshot::channel();
        *provider.start.lock().unwrap() = Some(start);
        let (installed, shown) = tokio::sync::oneshot::channel();
        *driver.ui.installed.borrow_mut() = Some(installed);
        let complete = async {
            admitted.await.unwrap();
            shown.await.unwrap();
            let (signal, stream) = provider.pending.lock().unwrap().take().unwrap();
            assert!(!signal.is_aborted());
            stream.end(Some(Arc::new(RwLock::new(faux_assistant_message(
                "Delayed draft",
                FauxAssistantMessageOptions::default(),
            )))));
        };
        tokio::join!(driver.command("goal"), complete);
        assert_eq!(
            *driver.ui.calls.borrow(),
            [json!(["editor", "Edit handoff prompt", "Delayed draft"])]
        );
        assert_eq!(
            *driver.replacement.calls.borrow(),
            [
                json!(["set", " Edited\n雪 "]),
                json!(["notify", "Handoff ready. Submit when ready.", "info"])
            ]
        );
        assert_eq!(driver.parents.borrow().len(), 1);
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
    });
}

#[test]
fn handoff_selected_history_uses_latest_compaction_and_kept_prefix() {
    run("handoff-history", async |provider, driver| {
        driver.veto.set(true);
        driver.message("old discarded");
        driver.compact("old summary", "missing", 20.0);
        let kept = driver.message("kept");
        driver.compact("kept summary", &kept, 25.0);
        driver.compact("latest summary", &kept, 30.0);
        let tip = driver.message("after");
        driver.message("other branch excluded");
        driver.history.borrow_mut().branch(&tip).unwrap();
        driver.command("next").await;
        let text =
            provider.requests.lock().unwrap()[0]["context"]["messages"][0]["content"][0]["text"]
                .as_str()
                .unwrap()
                .to_owned();
        let entries = driver
            .history
            .borrow()
            .get_entries()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let compaction = entries
            .iter()
            .rev()
            .find(|entry| entry["type"] == "compaction")
            .unwrap();
        assert_eq!(
            text.as_bytes(),
            expected_history(compaction["timestamp"].as_str().unwrap()).as_bytes(),
            "the compiled zero timestamp and parsed timestamp give byte-identical request text"
        );
        assert!(text.contains("latest summary"));
        assert!(text.find("[User]: kept").unwrap() < text.find("kept summary").unwrap());
        assert!(text.find("kept summary").unwrap() < text.find("[User]: after").unwrap());
        for excluded in ["old discarded", "old summary", "other branch excluded"] {
            assert!(!text.contains(excluded));
        }
        assert_eq!(
            driver.ui.calls.borrow().len(),
            1,
            "only the editor used the original UI"
        );
        assert_eq!(
            *driver.replacement.calls.borrow(),
            [json!(["notify", "New session cancelled", "info"])],
            "a veto notification reacquires the current UI without setting any editor"
        );
    });
}

#[test]
fn handoff_cancellation_and_auth_failure_leave_session_unchanged() {
    run("handoff-cancel", async |provider, driver| {
        driver.message("history");
        escape_pending_loader(&provider, &driver).await;
        driver.ui.escape.set(false);
        driver
            .registry
            .auth_storage()
            .remove_runtime_api_key("fixture");
        driver.command("goal").await;
        driver.notification("Cancelled", "info");
        assert!(provider.requests.lock().unwrap().is_empty());
        driver
            .registry
            .auth_storage()
            .set_runtime_api_key("fixture", "controlled-only");
        provider
            .aborted
            .store(true, std::sync::atomic::Ordering::SeqCst);
        driver.command("goal").await;
        driver.notification("Cancelled", "info");
        provider
            .aborted
            .store(false, std::sync::atomic::Ordering::SeqCst);
        *driver.ui.edited.borrow_mut() = None;
        driver.command("goal").await;
        assert_eq!(
            driver.ui.calls.borrow().last().unwrap(),
            &json!(["notify", "Cancelled", "info"])
        );
        assert!(driver.parents.borrow().is_empty());
        assert!(driver.replacement.calls.borrow().is_empty());
    });
}

/// Cancels only after the real provider sees the request, without relying on callback scheduling.
async fn escape_pending_loader(provider: &Provider, driver: &Driver) {
    let (start, wait) = tokio::sync::oneshot::channel();
    *provider.start.lock().unwrap() = Some(start);
    *driver.ui.wait.borrow_mut() = Some(wait);
    driver.ui.escape.set(true);
    driver.command("goal").await;
    driver.notification("Cancelled", "info");
    let (signal, stream) = provider.pending.lock().unwrap().take().unwrap();
    assert!(
        signal.is_aborted(),
        "Escape cancels the admitted native request"
    );
    stream.end(Some(Arc::new(RwLock::new(faux_assistant_message(
        "",
        FauxAssistantMessageOptions::default(),
    )))));
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    provider.requests.lock().unwrap().clear();
}

/// The same native helper path as the compiled command, with a real parsed compaction time.
fn expected_history(timestamp: &str) -> String {
    let timestamp = maestro_session::messages::parse_timestamp_millis(timestamp)
        .unwrap()
        .to_string()
        .parse::<f64>()
        .unwrap();
    assert!(timestamp > 0.0);
    let messages = json!([[{"role":"compactionSummary","summary":"latest summary", "tokensBefore":30.0,"timestamp":timestamp},
        {"role":"user","content":"kept","timestamp":1.0},
        {"role":"compactionSummary","summary":"kept summary","tokensBefore":25.0,"timestamp":timestamp},
        {"role":"user","content":"after","timestamp":1.0}]]);
    let converted =
        maestro_extensions::helpers::call("conversation.convert_to_llm", &messages.to_string())
            .unwrap();
    let serialized: String = serde_json::from_str(
        &maestro_extensions::helpers::call(
            "conversation.serialize_conversation",
            &format!("[{converted}]"),
        )
        .unwrap(),
    )
    .unwrap();
    format!("## Conversation History\n\n{serialized}\n\n## User's Goal for New Thread\n\nnext")
}
