#![cfg(unix)]
//! The interactive mode as its users reach it: built through its constructor over the
//! recording terminal and the manual frame host, with a session whose provider the test
//! scripts. Tests type through the terminal and read its frames and the mode's view.
pub mod corpus;
pub mod isolation;
pub mod transcript_corpus;

use crate::footer_support::harness;
use crate::manual_runtime::ManualRuntime;
use crate::recording_terminal::RecordingTerminal;
use maestro_app::presentation_data::status_line::NativeFooterOperations;
use maestro_app::{AgentSessionRuntime, PromptOptions};
use maestro_chat::{InteractiveMode, InteractiveModeOptions, Region};
use maestro_models::{
    AssistantMessage, AssistantMessageEventStream, Cancellation, SharedAssistantMessage,
};
use maestro_settings::SettingsManager;
use maestro_theme::{ThemeDirectories, ThemeState};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// The terminal effects a tap collected: titles and progress requests, not frame writes.
pub type Effects = Rc<RefCell<Vec<String>>>;

/// A mode and the controlled ends it was built over.
pub struct World {
    /// The mode under test.
    pub mode: Rc<InteractiveMode>,
    /// The terminal keys are typed into and frames are written to.
    pub terminal: RecordingTerminal,
    /// The host that runs scheduled frames and holds the work the mode hands over.
    pub runtime: ManualRuntime,
    /// The theme registry the mode selected from.
    pub themes: Rc<ThemeState>,
    /// The runtime the mode reads its session from.
    pub host: Rc<AgentSessionRuntime>,
}

/// The inputs of one mode.
pub struct Setup {
    /// The terminal's width, which is the width every region is read at.
    pub columns: usize,
    /// Changes the settings before construction.
    pub configure: Box<dyn FnOnce(&mut SettingsManager)>,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            columns: 80,
            configure: Box::new(|_| {}),
        }
    }
}

impl Setup {
    /// Constructs the mode over a memory session in `directory`, without starting it. Its
    /// themes are the shipped ones, and the dark one is selected.
    pub fn build(self, directory: &Path) -> World {
        let cwd = directory.to_str().expect("a text path");
        let result = harness::session(cwd, None, |inner| inner);
        let settings = Rc::clone(result.session.settings_manager());
        settings.borrow_mut().set_theme("dark".to_owned());
        (self.configure)(&mut settings.borrow_mut());
        let services = Rc::clone(&result.services);
        let host = Rc::new(AgentSessionRuntime::new(
            result,
            Rc::new(move |options| {
                let replacement = harness::over(&options.cwd, &services);
                Box::pin(async { Ok(replacement) })
            }),
        ));
        let terminal = RecordingTerminal::new(self.columns, 24);
        let runtime = ManualRuntime::new();
        let themes = Rc::new(ThemeState::new(ThemeDirectories {
            themes_dir: concat!(env!("CARGO_MANIFEST_DIR"), "/../maestro-theme/assets/theme")
                .into(),
            custom_themes_dir: String::new(),
        }));
        let effects = NativeFooterOperations::new(Rc::new(tokio::task::LocalSet::new()));
        let mode = InteractiveMode::new(
            Rc::clone(&host),
            InteractiveModeOptions::default(),
            (terminal.handle(), runtime.handle()),
            Rc::clone(&themes),
            Rc::new(effects),
        )
        .expect("the mode is built");
        World {
            mode,
            terminal,
            runtime,
            themes,
            host,
        }
    }
}

/// Builds a default mode in `directory` and starts it, settling the first frame.
pub async fn started(directory: &Path) -> World {
    let world = Setup::default().build(directory);
    world.start().await;
    world
}

/// A response the scripted provider left open.
pub struct Held {
    /// The open response.
    pub stream: AssistantMessageEventStream,
    /// The turn's cancellation.
    signal: Cancellation,
}

impl Held {
    /// Whether the turn was stopped.
    pub fn is_aborted(&self) -> bool {
        self.signal.is_aborted()
    }
}

/// The stored record of an assistant message with `content` and its stop reason.
pub fn assistant_record(content: &Value, stop: &str) -> Value {
    json!({
        "role": "assistant", "content": content, "api": "unknown", "provider": "unknown",
        "model": "unknown", "usage": maestro_models::test_support::zero_usage(),
        "stopReason": stop, "timestamp": 0
    })
}

/// An assistant message of the scripted provider with `content` and its stop reason.
pub fn assistant(content: &Value, stop: &str) -> SharedAssistantMessage {
    let message: AssistantMessage =
        serde_json::from_value(assistant_record(content, stop)).expect("an assistant message");
    Arc::new(RwLock::new(message))
}

impl World {
    /// Starts the mode and draws its first frame.
    pub async fn start(&self) {
        self.mode.init().await.expect("the mode starts");
        self.runtime.settle().expect("the first frame is drawn");
    }

    /// The rows `region` shows at the terminal's width.
    pub fn region(&self, region: Region) -> Vec<String> {
        self.mode.view().region(region)
    }

    /// Collects the effects the terminal is asked to emulate from now on.
    pub fn effects(&self) -> Effects {
        let effects = Effects::default();
        let sink = Rc::clone(&effects);
        self.terminal
            .set_tap(move |effect| sink.borrow_mut().push(effect.to_owned()));
        effects
    }

    /// Types `text` into the terminal, one key at a time.
    pub fn type_text(&self, text: &str) {
        let mut key = [0; 4];
        for character in text.chars() {
            self.terminal.send_input(character.encode_utf8(&mut key));
        }
    }

    /// Answers every later provider request with `respond`, which is given the request's
    /// position and the response to fill.
    pub fn script(&self, respond: impl Fn(usize, &AssistantMessageEventStream) + 'static) {
        self.host
            .services()
            .auth_storage
            .set_runtime_api_key("unknown", "key");
        let requests = Cell::new(0);
        let session = self.host.session();
        *session
            .agent()
            .stream_fn
            .write()
            .expect("the provider slot") = Some(Rc::new(move |_, _, _, _| {
            let stream = AssistantMessageEventStream::new();
            respond(requests.get(), &stream);
            requests.set(requests.get() + 1);
            Box::pin(std::future::ready(Ok(stream)))
        }));
    }

    /// Prompts the session with `text` and waits for the turn the scripted provider answers.
    pub async fn prompt(&self, text: &str) {
        let session = self.host.session();
        let turn = session.prompt(text, PromptOptions::default()).await;
        turn.expect("the prompt settles");
    }

    /// Prompts `text` and runs `body` while the session streams the response the scripted
    /// provider holds open,
    /// then ends that response and waits for the prompt to settle.
    pub async fn held<T>(&self, text: &str, body: impl AsyncFnOnce(&Held) -> T) -> T {
        let session = self.host.session();
        self.host
            .services()
            .auth_storage
            .set_runtime_api_key("unknown", "key");
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let sender = RefCell::new(Some(sender));
        *session
            .agent()
            .stream_fn
            .write()
            .expect("the provider slot") = Some(Rc::new(move |_, _, options, _| {
            let stream = AssistantMessageEventStream::new();
            if let Some(sender) = sender.borrow_mut().take() {
                let signal = options.common.signal.expect("a turn cancellation");
                let held = Held {
                    stream: stream.clone(),
                    signal,
                };
                drop(sender.send(held));
            } else {
                stream.end(Some(assistant(&json!([]), "stop")));
            }
            Box::pin(std::future::ready(Ok(stream)))
        }));
        let (prompted, text) = (Rc::clone(&session), text.to_owned());
        let run = tokio::task::spawn_local(async move {
            prompted.prompt(&text, PromptOptions::default()).await
        });
        let held = receiver.await.expect("the provider is asked");
        let value = body(&held).await;
        held.stream.end(Some(assistant(&json!([]), "stop")));
        run.await
            .expect("the prompt task finishes")
            .expect("the prompt settles");
        value
    }
}
