//! The shipped summary completes through the native provider and retained Markdown widgets.
#![cfg(test)]
use maestro_extensions::{
    EventError, ExtensionContextActions, ExtensionExecution, ExtensionRunner, ExtensionUIContext,
    Local,
    types::{
        CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions, ExtensionUIContextPort,
        ExtensionUIFuture, NotificationType, ProviderActions,
    },
};
use maestro_models::*;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    io::{self, Write},
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};

/// Original provider operands, observed at native dispatch rather than a second stream route.
#[derive(Default)]
struct Recording(Mutex<Vec<Value>>);
impl<E: ProviderExecution, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>>
    for Recording
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        let Some(StreamOptionsInput::Raw(options)) = call.options.map(Into::into) else {
            panic!("summary uses raw completion options");
        };
        self.0.lock().unwrap().push(json!({"model":call.model,"context":call.context,"common":options.common,"extra":options.extra}));
        let stream = AssistantMessageEventStream::new();
        let message = faux_assistant_message(
            vec![
                AssistantContent::Thinking(faux_thinking("private reasoning")),
                AssistantContent::Text(faux_text("## Goals")),
                AssistantContent::ToolCall(faux_tool_call(
                    "ignored",
                    json!({}),
                    FauxToolCallOptions {
                        id: Some("ignored".into()),
                    },
                )),
                AssistantContent::Text(faux_text("Resume later.")),
            ],
            FauxAssistantMessageOptions {
                timestamp: Some(12.0),
                ..Default::default()
            },
        );
        stream.push(AssistantMessageEvent::Done {
            reason: DoneReason::Stop,
            message: Arc::new(RwLock::new(message)),
        });
        Ok(stream)
    }
}

/// Guest stdout captured independently of the host's notifications and view calls.
#[derive(Clone, Default)]
struct Output(Arc<Mutex<Vec<u8>>>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A controlled UI using the real widgets, theme and terminal owner.
struct Ui {
    /// Host terminal supplied to the custom factory.
    tui: maestro_tui::TUI,
    /// Notifications in call order.
    notifications: RefCell<Vec<Value>>,
    /// Terminal key that closes the view.
    close: &'static str,
    /// Number of completed custom views.
    views: Cell<usize>,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        theme()
    }
    fn notify(&self, text: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        self.notifications.borrow_mut().push(json!([text, kind]));
        Ok(())
    }
    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        _: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        self.views.set(self.views.get() + 1);
        let closed = Rc::new(Cell::new(false));
        let done = closed.clone();
        let keys = maestro_extensions::widgets::ExtensionKeybindings {
            manager: maestro_tui::KeybindingsManager::new(Vec::new(), Vec::new()),
            reload: None,
        };
        let built = factory.call((
            self.tui.clone(),
            self.theme(),
            keys,
            Rc::new(move || Ok(!done.replace(true))),
        ));
        let close = self.close;
        Box::pin(async move {
            let CustomFactoryOutcome::Ready(built) = built? else {
                panic!("summary factory is ready")
            };
            assert_eq!(built.width, None);
            check_frame(&built.component.try_render(80).unwrap());
            built.component.invalidate();
            check_frame(&built.component.try_render(80).unwrap());
            let input = built.component.input_handler().unwrap();
            input.try_handle_input("x").unwrap();
            assert!(!closed.get(), "ordinary input does not close");
            input.try_handle_input(close).unwrap();
            assert!(closed.get(), "Enter or Escape closes");
            Ok(())
        })
    }
}

/// A prepared shipped palette, shared by the widget owner and custom factory.
fn theme() -> maestro_theme::LiveTheme {
    let theme = maestro_theme::load_theme_from_path(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../maestro-theme/assets/theme/dark.json"
        ),
        Some(maestro_theme::ColorMode::Truecolor),
    )
    .unwrap();
    maestro_theme::LiveTheme::from_theme(Rc::new(theme))
}

/// Checks ordered headings, text, hint and full-width borders in the real Markdown output.
fn check_frame(rows: &[String]) {
    assert!(rows.first().unwrap().contains(&"─".repeat(80)));
    assert!(rows.last().unwrap().contains(&"─".repeat(80)));
    assert!(rows.iter().all(|row| maestro_tui::visible_width(row) == 80));
    let text = rows.join("\n");
    let title = text.find("Conversation Summary").unwrap();
    let goal = text.find("Goals").unwrap();
    let body = text.find("Resume later.").unwrap();
    let hint = text.find("Press Enter or Esc to close").unwrap();
    assert!(title < goal && goal < body && body < hint);
    for ignored in ["##", "private reasoning", "ignored"] {
        assert!(!text.contains(ignored));
    }
}

/// Installs a non-network descriptor and literal key/headers in the original catalog.
fn configure(session: &maestro_extensions::runner::ExtensionRunnerSession, api: &str) {
    let config = serde_json::from_value(json!({
        "api":api,"baseUrl":"https://example.invalid","apiKey":"controlled-key","headers":{"x-summary":"literal"},
        "models":[{"id":"gpt-5.2","name":"Controlled summary","reasoning":true,"input":["text"],
            "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":128,"maxTokens":32}]
    })).unwrap();
    session
        .model_registry
        .register_provider("openai", SharedHandle::new(RwLock::new(config)))
        .unwrap();
}

/// Current-branch entries include ignored roles, images, blank text and tool-only messages.
fn history(history: &Rc<RefCell<maestro_session::SessionManager>>) {
    let mut h = history.borrow_mut();
    for message in [
        json!({"role":"user","content":" \u{feff}Start Ω\u{feff} ","timestamp":1}),
        json!({"role":"user","content":[{"type":"text","text":"first"},{"type":"image","data":"eA==","mimeType":"image/png"},{"type":"text","text":"second"}],"timestamp":2}),
        json!({"role":"assistant","content":[{"type":"thinking","thinking":"secret"},{"type":"toolCall","id":"one","name":"read","arguments":{"path":"a","n":-0.0,"10":1e21,"2":false}},{"type":"text","text":" \nanswer\n "},{"type":"toolCall","id":"two","name":"bash","arguments":{}}],"api":"faux","provider":"faux","model":"faux-1","usage":Usage::default(),"stopReason":"toolUse","timestamp":3}),
        json!({"role":"toolResult","toolCallId":"one","toolName":"read","content":[{"type":"text","text":"ignored result"}],"isError":false,"timestamp":4}),
        json!({"role":"user","content":"\t\u{a0}\u{feff}\n","timestamp":5}),
        json!({"role":"user","content":"\u{85}keep\u{85}","timestamp":6}),
        json!({"role":"assistant","content":[{"type":"toolCall","id":"three","name":"empty","arguments":{}}],"api":"faux","provider":"faux","model":"faux-1","usage":Usage::default(),"stopReason":"toolUse","timestamp":7}),
    ] {
        h.append_message(message).unwrap();
    }
    h.append_custom_entry(
        "ignored",
        maestro_session::Presence::Present(json!({"text":"not a message"})),
    )
    .unwrap();
}

/// Literal request text from the reference, independent of the example's extraction code.
const PROMPT: &str = "Summarize this conversation so I can resume it later.\nInclude goals, key decisions, progress, open questions, and next steps.\nKeep it concise and structured with headings.\n\n<conversation>\nUser: Start Ω\n\nUser: first\nsecond\n\nAssistant: answer\nTool read was called with args {\"2\":false,\"10\":1e+21,\"path\":\"a\",\"n\":0}\nTool bash was called with args {}\n\nUser: \u{85}keep\u{85}\n\nTool empty was called with args {}\n</conversation>";

/// Loads only the shipped component, with no author fixture's registrations.
async fn run(close: Option<&'static str>) {
    let api = if close.is_some() {
        "summary-ui"
    } else {
        "summary-headless"
    };
    let (session, entries) = super::session::runner_session("/summary");
    configure(&session, api);
    history(&entries);
    let recording = Arc::new(Recording::default());
    register_api_provider(
        ApiProvider {
            api: api.into(),
            stream: recording.clone(),
            stream_simple: recording.clone(),
        },
        Some(api.into()),
    );
    let output = Output::default();
    let (runner, bus) = load(session, output.clone()).await;
    let ui = close.map(|close| {
        Rc::new(Ui {
            tui: super::canonical_ui::screen().0,
            close,
            notifications: RefCell::default(),
            views: Cell::new(0),
        })
    });
    if let Some(ui) = &ui {
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
    }
    let command = runner.get_command("summarize").unwrap().command;
    assert_eq!(
        command.description.as_deref(),
        Some("Summarize the current conversation in a custom UI")
    );
    (command.handler)("ignored".into(), runner.create_command_context())
        .await
        .unwrap();
    assert!(
        output.0.lock().unwrap().is_empty(),
        "headless and UI runs print nothing"
    );
    if let Some(ui) = ui {
        assert_eq!(ui.views.get(), 1);
        assert_eq!(
            *ui.notifications.borrow(),
            [json!(["Preparing summary...", "info"])]
        );
    }
    check_request(&recording);
    unregister_api_providers(api);
    bus.clear();
}

/// Loads the shipped component against the existing production host.
async fn load(
    session: maestro_extensions::runner::ExtensionRunnerSession,
    output: Output,
) -> (
    ExtensionRunner<Local>,
    maestro_extensions::EventBusController<Local>,
) {
    let bus = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no errors")),
    );
    let runtime = maestro_extensions::create_extension_runtime::<Local>();
    let path = std::env::var("MAESTRO_SUMMARIZE_COMPONENT").expect("nextest prepares summarize");
    let factory = maestro_extensions_wasmtime::artifact_factory_with_stdout(
        path.clone(),
        super::support::widgets_over(theme()),
        maestro_extensions_wasmtime::Installation {
            version: "summary-host".into(),
            agent_dir: "/controlled-agent".into(),
        },
        Box::new(output),
        false,
    );
    let owner = maestro_extensions::load_extension_from_factory(
        factory,
        "/summary".into(),
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
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions::default(),
            ProviderActions::default(),
        )
        .unwrap();
    (runner, bus)
}

/// Checks the literal prompt, fixed model and raw authentication options at dispatch.
fn check_request(recording: &Recording) {
    let calls = recording.0.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["model"]["provider"], "openai");
    assert_eq!(calls[0]["model"]["id"], "gpt-5.2");
    assert_eq!(
        calls[0]["context"]["messages"][0]["content"],
        json!([{"type":"text","text":PROMPT}])
    );
    assert_eq!(calls[0]["extra"], json!({"reasoningEffort":"high"}));
    assert_eq!(calls[0]["common"]["apiKey"], "controlled-key");
    assert_eq!(calls[0]["common"]["headers"]["x-summary"], "literal");
}

#[test]
fn summarize_completion_extracts_branch_and_stays_silent_headless() {
    super::components::local(run(None));
}

#[test]
fn summarize_markdown_view_closes_on_enter_and_escape() {
    super::components::local(async {
        for close in ["\r", "\x1b"] {
            run(Some(close)).await;
        }
    });
}
