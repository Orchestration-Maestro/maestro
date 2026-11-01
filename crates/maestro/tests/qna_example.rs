//! Runs the shipped question extractor through the production component runner.
use crate::support::theme;
use maestro_app::{
    AgentSession, CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions, EventError,
    ExtensionBindings, ExtensionUIContext, ExtensionUIContextPort, ExtensionUIFuture,
    NotificationType,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The editor contents and literal notifications from the command.
#[derive(Default)]
struct Ui {
    /// Host requests in arrival order.
    requests: RefCell<Vec<Value>>,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        theme()
    }
    fn notify(&self, message: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        self.requests
            .borrow_mut()
            .push(json!(["notify", message, format!("{kind:?}")]));
        Ok(())
    }
    fn set_editor_text(&self, text: String) -> Result<(), EventError> {
        self.requests.borrow_mut().push(json!(["editor", text]));
        Ok(())
    }
}

/// Invokes the registered command without prompting the agent or calling a provider.
async fn command(session: &AgentSession) {
    let runner = session.extension_runner();
    let registered = runner.get_command("qna").unwrap();
    assert_eq!(
        registered.command.description.as_deref(),
        Some("Extract questions from last assistant message into editor")
    );
    (registered.command.handler)("ignored".into(), runner.create_command_context())
        .await
        .unwrap();
}

/// Adds a literal message to the actual current session branch.
fn append(session: &AgentSession, stop: &str, content: &Value) {
    let message = json!({"role":"assistant", "content":content,
        "api":"openai-completions", "provider":"compiled-stream", "model":"registered-case",
        "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
            "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
        "stopReason":stop,"timestamp":0});
    session
        .session_manager()
        .borrow_mut()
        .append_message(message)
        .unwrap();
}

/// Runs a compiled example on an isolated native session and local executor.
fn run(body: impl AsyncFnOnce(Rc<AgentSession>)) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let root = super::support::Root::new();
        let session = super::support::session::component_session(
            &root,
            local.clone(),
            std::env::var("MAESTRO_QNA_COMPONENT").expect("nextest author-component setup"),
            Some(maestro_app::NoTools::All),
            Rc::new(maestro_chat::ExtensionWidgetHost::new(
                theme(),
                Rc::new(maestro_theme::SyntectHighlighter::new().unwrap()),
                maestro_tui::TerminalImage::default(),
            )),
        )
        .await;
        body(session.clone()).await;
        session.dispose();
    });
}

#[test]
fn qna_rejects_missing_and_incomplete_replies_and_missing_model() {
    run(async |session| {
        let ui = Rc::new(Ui::default());
        bind(&session, ui.clone()).await;
        command(&session).await;
        assert_eq!(
            *ui.requests.borrow(),
            [json!([
                "notify",
                "No assistant messages found",
                "Some(Error)"
            ])]
        );
        for stop in ["length", "toolUse", "error", "aborted"] {
            ui.requests.borrow_mut().clear();
            append(
                &session,
                stop,
                &json!([{"type":"text", "text":"Incomplete?"}]),
            );
            command(&session).await;
            assert_eq!(
                *ui.requests.borrow(),
                [json!([
                    "notify",
                    format!("Last assistant message incomplete ({stop})"),
                    "Some(Error)"
                ])]
            );
        }
        ui.requests.borrow_mut().clear();
        let context = unbound_context(&session, ui.clone());
        let handler = session
            .extension_runner()
            .get_command("qna")
            .unwrap()
            .command
            .handler;
        handler(String::new(), context).await.unwrap();
        assert_eq!(
            *ui.requests.borrow(),
            [json!(["notify", "No model selected", "Some(Error)"])]
        );
    });
}
/// Supplies an unbound runner's missing-model context to the already registered command.
fn unbound_context(session: &AgentSession, ui: Rc<Ui>) -> maestro_app::ExtensionCommandContext {
    let loaded = session.resource_loader().get_extensions();
    let runner = maestro_app::ExtensionRunner::<maestro_app::Local>::new(
        Rc::new(std::sync::RwLock::new(loaded.extensions.clone())),
        loaded.runtime.clone(),
        maestro_app::ExtensionRunnerSession {
            cwd: "/controlled".into(),
            session_manager: session.session_manager().clone(),
            model_registry: session.model_registry().clone(),
        },
        Rc::new(|_| {}),
        theme(),
    );
    runner.set_ui_context(Some(ExtensionUIContext::new(ui)));
    runner.create_command_context()
}

/// A real loader view hosted on the shared controlled terminal and clock.
struct LoaderUi {
    /// UI requests visible to the caller.
    ui: Rc<Ui>,
    /// Terminal supplied to the guest custom factory.
    tui: maestro_tui::TUI,
    /// The host's real editable multiline input.
    editor: maestro_tui::Editor,
    /// Clock whose active timers prove loader disposal.
    clock: super::tools_example::manual_runtime::ManualRuntime,
    /// Escape before model invocation, or wait for completion.
    escape: bool,
    /// A host action after the loader is built but before extraction can complete.
    on_loader: RefCell<Option<Box<dyn FnOnce()>>>,
    /// The provider request must start before cancellation is delivered.
    entered: RefCell<Option<tokio::sync::oneshot::Receiver<()>>>,
    /// Releases the fixture response only after the custom view is disposed.
    release: RefCell<Option<std::sync::mpsc::Sender<()>>>,
}
impl ExtensionUIContextPort for LoaderUi {
    fn theme(&self) -> maestro_theme::LiveTheme {
        theme()
    }
    fn notify(&self, text: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        self.ui.notify(text, kind)
    }
    fn set_editor_text(&self, text: String) -> Result<(), EventError> {
        self.editor.set_text(&text);
        self.ui.set_editor_text(text)
    }
    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        _: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        let closed = Rc::new(std::cell::Cell::new(false));
        let signal = Rc::new(tokio::sync::Notify::new());
        let disposer = Rc::new(RefCell::new(None::<maestro_app::UIHostCall<(), ()>>));
        let closing = disposer.clone();
        let (accepted, wake) = (closed.clone(), signal.clone());
        let built = factory.call((
            self.tui.clone(),
            self.theme(),
            maestro_app::ExtensionKeybindings {
                manager: maestro_tui::KeybindingsManager::new(Vec::new(), Vec::new()),
                reload: None,
            },
            Rc::new(move || {
                let first = !accepted.replace(true);
                if first && let Some(dispose) = closing.borrow_mut().take() {
                    dispose.call(())?;
                }
                wake.notify_one();
                Ok(first)
            }),
        ));
        if let Some(on_loader) = self.on_loader.borrow_mut().take() {
            on_loader();
        }
        let (escape, clock, ui) = (self.escape, self.clock.clone(), self.ui.clone());
        let entered = self.entered.borrow_mut().take();
        let release = self.release.borrow_mut().take();
        Box::pin(async move {
            let built = match built? {
                CustomFactoryOutcome::Ready(built) => built,
                CustomFactoryOutcome::Async(built) => built.await?,
            };
            disposer.replace(built.dispose.clone());
            assert_loader(built.component.try_render(80).unwrap());
            assert!(clock.pending() > 0, "loader is animating");
            ui.requests.borrow_mut().push(json!(["loader"]));
            if let Some(entered) = entered {
                entered.await.unwrap();
            }
            if escape {
                built
                    .component
                    .input_handler()
                    .unwrap()
                    .try_handle_input("\x1b")
                    .unwrap();
            }
            signal.notified().await;
            assert!(closed.get());
            assert!(disposer.borrow().is_none(), "close disposes synchronously");
            for result in clock.advance_to(std::time::Duration::from_millis(clock.millis() + 80)) {
                result.unwrap();
            }
            assert_eq!(clock.pending(), 0, "closing disposes the loader timer");
            ui.requests.borrow_mut().push(json!(["disposed"]));
            if let Some(release) = release {
                release.send(()).unwrap();
            }
            Ok(())
        })
    }
}

/// The native loader shows the descriptor selected when its view opened.
fn assert_loader(rows: Vec<String>) {
    assert!(
        rows.into_iter()
            .any(|row| row.contains("Extracting questions using registered-case..."))
    );
}

/// Supplies the terminal, close acknowledgement and exact editor receipts.
fn loader_ui(escape: bool) -> Rc<LoaderUi> {
    let clock = super::tools_example::manual_runtime::ManualRuntime::new();
    let terminal = super::tools_example::recording_terminal::RecordingTerminal::new(80, 24);
    let tui = maestro_tui::TUI::new(
        terminal.handle(),
        clock.handle(),
        maestro_tui::TerminalImage::default(),
        Some(false),
    );
    let editor = maestro_tui::Editor::new(
        &tui,
        maestro_theme::get_editor_theme(theme()),
        maestro_tui::EditorOptions::default(),
    );
    Rc::new(LoaderUi {
        ui: Rc::new(Ui::default()),
        tui,
        editor,
        clock,
        escape,
        on_loader: RefCell::default(),
        entered: RefCell::default(),
        release: RefCell::default(),
    })
}

/// Binds one controlled interactive host to the real session.
async fn bind(session: &Rc<AgentSession>, ui: Rc<dyn ExtensionUIContextPort>) {
    session
        .bind_extensions(ExtensionBindings {
            ui_context: Some(ExtensionUIContext::new(ui).into()),
            ..ExtensionBindings::default()
        })
        .await
        .unwrap();
}

/// Publishes the loopback model through the session's configured registry.
async fn provider(session: &AgentSession, base: &str) {
    use std::sync::{Arc, RwLock};
    let mut config: maestro_app::ProviderConfigInput = serde_json::from_value(json!({
        "baseUrl":base, "apiKey":"controlled-qna-key", "api":"openai-completions",
        "headers":{"x-qna":"literal"}, "models":[{"id":"registered-case", "name":"controlled", "reasoning":false,
            "input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":10000,"maxTokens":100}]
    })).unwrap();
    let mut replacement = config.models.as_ref().unwrap()[0].clone();
    replacement.id = "extraction-case".into();
    config.models.as_mut().unwrap().push(replacement);
    session
        .model_registry()
        .register_provider("qna-controlled", Arc::new(RwLock::new(config)))
        .unwrap();
    session
        .set_model(
            session
                .model_registry()
                .find("qna-controlled", "registered-case")
                .unwrap(),
        )
        .await
        .unwrap();
}

/// A single controlled HTTP completion, optionally held until the loader is cancelled.
fn completion(
    text: &str,
    finish: &str,
    gate: Option<(
        tokio::sync::oneshot::Sender<()>,
        std::sync::mpsc::Receiver<()>,
    )>,
) -> (String, std::thread::JoinHandle<Value>) {
    use std::{io::Write, net::TcpListener};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let chunk = json!({"id":"qna-controlled", "choices":[{"index":0,"delta":{"content":text},"finish_reason":finish}]});
    let events = format!("data: {chunk}\n\ndata: [DONE]\n\n");
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = super::support::provider::request(&mut stream);
        if let Some((entered, resume)) = gate {
            entered.send(()).unwrap();
            resume.recv().unwrap();
        }
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{events}",
            events.len()
        );
        request
    });
    (base, worker)
}

#[test]
fn qna_loads_exact_completion_into_editor_and_disposes_loader() {
    run(async |session| {
        let (base, request) = completion(
            " Q: Which database?\nA: \n\nQ: Unicode 雪?\nA: ",
            "stop",
            None,
        );
        provider(&session, &base).await;
        append(
            &session,
            "stop",
            &json!([{"type":"text","text":"old branch question?"}]),
        );
        append(
            &session,
            "stop",
            &json!([{"type":"thinking","thinking":"ignore me"}, {"type":"text","text":" First? "}, {"type":"text","text":"Second?\n"}]),
        );
        append(
            &session,
            "stop",
            &json!([{"type":"thinking","thinking":"skip empty assistant"}]),
        );
        let ui = loader_ui(false);
        bind(&session, ui.clone()).await;
        command(&session).await;
        assert_eq!(
            *ui.ui.requests.borrow(),
            [
                json!(["loader"]),
                json!(["disposed"]),
                json!(["editor", " Q: Which database?\nA: \n\nQ: Unicode 雪?\nA: "]),
                json!([
                    "notify",
                    "Questions loaded. Edit and submit when ready.",
                    "Some(Info)"
                ])
            ]
        );
        let context = request.join().unwrap();
        assert_eq!(context["messages"].as_array().unwrap().len(), 2);
        assert_eq!(
            context["messages"][1]["content"],
            json!([{"type":"text", "text":" First? \nSecond?\n"}])
        );
        assert_eq!(
            context["messages"][0]["content"],
            "You are a question extractor. Given text from a conversation, extract any questions that need answering and format them for the user to fill in.\n\nOutput format:\n- List each question on its own line, prefixed with \"Q: \"\n- After each question, add a blank line for the answer prefixed with \"A: \"\n- If no questions are found, output \"No questions found in the last message.\"\n\nExample output:\nQ: What is your preferred database?\nA: \n\nQ: Should we use TypeScript or JavaScript?\nA: \n\nKeep questions in the order they appeared. Be concise."
        );
        maestro_tui::tui::InputHandler::handle_input(&ui.editor, "SQLite");
        assert!(ui.editor.get_text().ends_with("Q: Unicode 雪?\nA: SQLite"));
    });
}

#[test]
fn qna_cancellation_and_missing_auth_leave_editor_unchanged() {
    run(async |session| {
        append(
            &session,
            "stop",
            &json!([{"type":"text","text":"Question?"}]),
        );
        let ui = loader_ui(false);
        ui.editor.set_text("unsent draft");
        bind(&session, ui.clone()).await;
        command(&session).await;
        assert_eq!(ui.editor.get_text(), "unsent draft");
        assert_eq!(
            *ui.ui.requests.borrow(),
            [
                json!(["loader"]),
                json!(["disposed"]),
                json!(["notify", "Cancelled", "Some(Info)"])
            ]
        );
        let (base, request) = completion("", "length", None);
        provider(&session, &base).await;
        ui.ui.requests.borrow_mut().clear();
        command(&session).await;
        assert_eq!(
            *ui.ui.requests.borrow(),
            [
                json!(["loader"]),
                json!(["disposed"]),
                json!(["editor", ""]),
                json!([
                    "notify",
                    "Questions loaded. Edit and submit when ready.",
                    "Some(Info)"
                ])
            ]
        );
        request.join().unwrap();
        let (started, entered) = tokio::sync::oneshot::channel();
        let (release, resume) = std::sync::mpsc::channel();
        let (base, cancelled) = completion("not loaded", "stop", Some((started, resume)));
        provider(&session, &base).await;
        let ui = loader_ui(true);
        ui.editor.set_text("unsent draft");
        ui.entered.replace(Some(entered));
        ui.release.replace(Some(release));
        bind(&session, ui.clone()).await;
        command(&session).await;
        assert_eq!(
            *ui.ui.requests.borrow(),
            [
                json!(["loader"]),
                json!(["disposed"]),
                json!(["notify", "Cancelled", "Some(Info)"])
            ]
        );
        cancelled.join().unwrap();
        assert_eq!(ui.editor.get_text(), "unsent draft");
    });
}

#[test]
fn qna_provider_errors_leave_editor_unchanged() {
    run(async |session| {
        append(
            &session,
            "stop",
            &json!([{"type":"text","text":"Question?"}]),
        );
        let ui = loader_ui(false);
        bind(&session, ui.clone()).await;
        for text in ["", "Q: Partial response?\nA: "] {
            let (base, request) = completion(text, "content_filter", None);
            provider(&session, &base).await;
            ui.editor.set_text("unsent draft");
            ui.ui.requests.borrow_mut().clear();
            command(&session).await;
            request.join().unwrap();
            assert_eq!(ui.editor.get_text(), "unsent draft");
            assert_eq!(
                *ui.ui.requests.borrow(),
                [
                    json!(["loader"]),
                    json!(["disposed"]),
                    json!(["notify", "Cancelled", "Some(Info)"])
                ]
            );
        }
    });
}

#[test]
fn qna_requires_interactive_ui() {
    run(async |session| {
        let ui = Rc::new(Ui::default());
        let port: Rc<dyn ExtensionUIContextPort> = ui.clone();
        session
            .bind_extensions(ExtensionBindings {
                ui_context: Some(maestro_app::ExtensionUIContextBinding::Borrowed {
                    port: Rc::downgrade(&port),
                    fallback: true,
                }),
                ..Default::default()
            })
            .await
            .unwrap();
        command(&session).await;
        assert_eq!(
            *ui.requests.borrow(),
            [json!([
                "notify",
                "qna requires interactive mode",
                "Some(Error)"
            ])]
        );
    });
}

#[test]
fn qna_uses_live_configured_model_after_loader_opens() {
    run(async |session| {
        let (base, request) = completion("Q: Current model?\nA: ", "stop", None);
        provider(&session, &base).await;
        append(
            &session,
            "stop",
            &json!([{"type":"text", "text":"Which model?"}]),
        );
        let ui = loader_ui(false);
        let changing = Rc::downgrade(&session);
        ui.on_loader.replace(Some(Box::new(move || {
            let session = changing.upgrade().unwrap();
            session.agent().state().write().unwrap().model = session
                .model_registry()
                .find("qna-controlled", "extraction-case")
                .unwrap();
        })));
        bind(&session, ui).await;
        command(&session).await;
        assert_eq!(request.join().unwrap()["model"], "extraction-case");
    });
}

#[test]
fn qna_uses_current_ui_after_extraction() {
    run(async |session| {
        let (base, request) = completion("Q: Current UI?\nA: ", "stop", None);
        provider(&session, &base).await;
        append(
            &session,
            "stop",
            &json!([{"type":"text", "text":"Question?"}]),
        );
        let old = loader_ui(false);
        old.editor.set_text("old draft");
        let current = loader_ui(false);
        let rebound = current.clone();
        let changing = Rc::downgrade(&session);
        old.on_loader.replace(Some(Box::new(move || {
            changing
                .upgrade()
                .unwrap()
                .extension_runner()
                .set_ui_context(Some(ExtensionUIContext::new(rebound)));
        })));
        bind(&session, old.clone()).await;
        command(&session).await;
        request.join().unwrap();
        assert_eq!(old.editor.get_text(), "old draft");
        assert_eq!(
            *old.ui.requests.borrow(),
            [json!(["loader"]), json!(["disposed"])]
        );
        assert_eq!(current.editor.get_text(), "Q: Current UI?\nA: ");
        assert_eq!(
            *current.ui.requests.borrow(),
            [
                json!(["editor", "Q: Current UI?\nA: "]),
                json!([
                    "notify",
                    "Questions loaded. Edit and submit when ready.",
                    "Some(Info)"
                ])
            ]
        );
    });
}
