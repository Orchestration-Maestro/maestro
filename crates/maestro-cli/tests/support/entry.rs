//! The startup entry over controlled package, presentation, input and mode effects.
#![cfg(unix)]

use super::mode_dispatch::{Recorder, launchers};
use super::package_commands::Harness as Commands;
use super::presenters;
use maestro_app::presentation_data::input_selectors::{
    ExtensionSelectorFactory, SessionListProgress, SessionSelectorFactory,
};
use maestro_cli::input::config_selector::ConfigPresentation;
use maestro_cli::input::file_processor::NativeFileArgumentOperations;
use maestro_cli::input::session_picker::SessionPresentation;
use maestro_cli::startup::main::presentation::MissingSessionCwdPresentation;
use maestro_cli::startup::main::{MainExport, MainHost, MainOptions, main};
use maestro_test_process::{TestProcess, selection};
use maestro_theme::ThemeState;
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::error::Error;
use std::future::Future;
use std::io::{Read, Write};
use std::rc::Rc;
use tokio::task::LocalSet;

/// Run `body` once per case, each in a cleared child process with the case's variables.
pub fn cases<F: Future<Output = ()>>(
    name: &str,
    cases: &[(&'static str, &[(&str, &str)])],
    body: impl FnOnce(Rc<LocalSet>, &'static str) -> F,
) {
    if selection().is_child() {
        let (case, _) = cases
            .iter()
            .find(|(case, _)| selection().runs_case(case))
            .expect("a selected case");
        let local = Rc::new(LocalSet::new());
        let runtime = super::startup::runtime();
        runtime.block_on(local.run_until(body(Rc::clone(&local), case)));
        return;
    }
    for (case, variables) in cases {
        let root = maestro_test_directory::Scratch::new().unwrap();
        let mut command: TestProcess = super::missing_cwd::command(name, case, root.path());
        for (key, value) in *variables {
            command = command.env(key, value);
        }
        let mut child = command.stdin(std::process::Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(b"k").unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{case}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Everything one startup run is given, and what it observably did.
pub struct Entry {
    /// Package context, settings files and the captured text streams.
    pub commands: Commands,
    /// Presentation, mode and effect calls in order.
    pub recorder: Rc<Recorder>,
    /// Standard input bytes.
    pub stdin: RefCell<Vec<u8>>,
    /// Terminal observations of standard input in order; the last one repeats.
    pub tty: Rc<RefCell<VecDeque<bool>>>,
    /// The export operation's result.
    pub export: RefCell<Result<String, String>>,
    /// The theme owner shared by every presentation.
    pub themes: Rc<ThemeState>,
    /// The frame writer of presented selectors.
    pub screen: presenters::Harness,
    /// The resume selector's choice; absent cancels, and an error refuses presentation.
    pub resume: RefCell<Result<Option<String>, String>>,
    /// The missing-directory dialog's choice.
    pub missing: Cell<&'static str>,
    /// The extension module adapter supplied to the entry.
    pub module_loader: maestro_app::ModuleLoader<maestro_app::Local>,
    /// The last clock reading.
    pub clock: Cell<f64>,
    /// A controlled effect at each supplied clock reading.
    pub clock_hook: Rc<dyn Fn(f64)>,
    /// Whether output is styled.
    pub styles: Cell<bool>,
    /// The ordinary output handle the host receives.
    pub out: Rc<RefCell<dyn Write>>,
    /// The diagnostic output handle the host receives.
    pub err: Rc<RefCell<dyn Write>>,
}

impl Entry {
    /// A startup over fresh directories, with no settings and an interactive terminal.
    pub fn new() -> Self {
        let commands = Commands::new(None, None);
        let themes = presenters::theme(std::path::Path::new(&commands.cwd).parent().unwrap());
        let (out, err) = (commands.stdout.handle(), commands.stderr.handle());
        Self {
            out,
            err,
            commands,
            recorder: Rc::new(Recorder::default()),
            stdin: RefCell::default(),
            tty: Rc::new(RefCell::new(VecDeque::from([true]))),
            export: RefCell::new(Ok("exported.html".into())),
            themes,
            screen: presenters::Harness::new(),
            resume: RefCell::new(Ok(None)),
            missing: Cell::new("Continue"),
            module_loader: Rc::new(|_| Box::pin(std::future::ready(Ok(None)))),
            clock: Cell::new(0.0),
            clock_hook: Rc::new(|_| {}),
            styles: Cell::new(false),
        }
    }

    /// Write the global and project settings documents.
    pub fn settings(&self, global: Option<Value>, project: Option<Value>) {
        self.commands.write_settings(global, project);
    }

    /// Configure provider `startup` at an address no run reaches, for runs that send no request.
    pub fn models(&self, ids: &[&str]) {
        self.served_models("https://example.test/v1", ids);
    }

    /// Record one call into the shared log.
    pub fn note(&self, entry: impl Into<String>) {
        self.recorder.log.borrow_mut().push(entry.into());
    }

    /// The recorded calls.
    pub fn log(&self) -> Vec<String> {
        self.recorder.log.borrow().clone()
    }

    /// Everything written to ordinary output.
    pub fn stdout(&self) -> String {
        self.commands.stdout.text()
    }

    /// Everything written to diagnostic output.
    pub fn stderr(&self) -> String {
        self.commands.stderr.text()
    }

    /// Run the public entry over `args` and this harness's effects.
    pub async fn run(
        &self,
        local: &Rc<LocalSet>,
        args: &[&str],
        options: MainOptions,
    ) -> Result<i32, Box<dyn Error>> {
        let args = args.iter().map(|arg| (*arg).to_owned()).collect();
        let bytes = self.stdin.borrow().clone();
        let length = bytes.len();
        let mut stdin = std::io::Cursor::new(bytes);
        let tty = || {
            let mut observations = self.tty.borrow_mut();
            if observations.len() > 1 {
                observations.pop_front().unwrap()
            } else {
                observations[0]
            }
        };
        let now = || {
            self.clock.set(self.clock.get() + 1.0);
            (self.clock_hook)(self.clock.get());
            self.clock.get()
        };
        let files = NativeFileArgumentOperations::new(false);
        self.commands
            .with_context(async |package| {
                let package = maestro_cli::packages::PackageCommandContext {
                    styles_enabled: self.styles.get(),
                    stdout: Rc::clone(&self.out),
                    stderr: Rc::clone(&self.err),
                    ..package
                };
                let host = MainHost {
                    package,
                    apply_startup: self.apply_startup(),
                    config: self.config(),
                    sessions: self.sessions(),
                    missing_cwd: self.missing_cwd(),
                    local: Rc::clone(local),
                    module_loader: Rc::clone(&self.module_loader),
                    modes: launchers(&self.recorder),
                    stdin: &mut stdin,
                    stdin_is_tty: &tty,
                    now: &now,
                    files: &files,
                    export_from_file: self.export_operation(),
                };
                let status = Box::pin(main(args, options, host)).await;
                let unread = length - usize::try_from(stdin.position()).unwrap();
                self.note(format!("unread:{unread}"));
                status
            })
            .await
    }

    /// Record the startup offline selection.
    fn apply_startup(&self) -> Rc<dyn Fn(bool)> {
        let recorder = Rc::clone(&self.recorder);
        let state = Rc::clone(&self.commands.state);
        Rc::new(move |offline| {
            state.offline.set(Some(offline));
            recorder.log.borrow_mut().push(format!("apply:{offline}"));
        })
    }

    /// The configuration presentation over the shared theme and recording watch.
    fn config(&self) -> ConfigPresentation {
        let log = Rc::clone(&self.recorder.log);
        ConfigPresentation {
            host: self.screen.host(Rc::new(|_| {})),
            theme: Rc::clone(&self.themes),
            watcher: Rc::new(presenters::RecordingWatch { log }),
            presenter: Rc::new(|_, _| Err(std::io::Error::other("no configuration view"))),
        }
    }

    /// The resume selector: it lists both scopes, then settles with the configured choice.
    fn sessions(&self) -> SessionPresentation {
        let recorder = Rc::clone(&self.recorder);
        let choice = self.resume.borrow().clone();
        let presenter: SessionSelectorFactory = Rc::new(move |request, callbacks| {
            recorder.log.borrow_mut().push("resume-presented".into());
            let choice = choice.clone().map_err(std::io::Error::other)?;
            let log = Rc::clone(&recorder.log);
            let notes = Rc::clone(&log);
            let progress: SessionListProgress = Rc::new(move |loaded, total| {
                notes
                    .borrow_mut()
                    .push(format!("progress:{loaded}/{total}"));
            });
            tokio::task::spawn_local(async move {
                let current = (request.current_sessions_loader)(Some(progress)).await;
                let all = (request.all_sessions_loader)(None).await;
                log.borrow_mut().push(format!(
                    "listed:{}/{}",
                    current.unwrap().len(),
                    all.unwrap().len()
                ));
                match choice {
                    Some(path) => (callbacks.on_select)(&path),
                    None => (callbacks.on_cancel)(),
                }
            });
            Ok(presenters::view(&Rc::default()))
        });
        SessionPresentation {
            host: self.screen.host(Rc::new(|_| {})),
            keybindings: None,
            presenter,
        }
    }

    /// The missing-directory dialog: it settles with the configured choice.
    fn missing_cwd(&self) -> MissingSessionCwdPresentation {
        let recorder = Rc::clone(&self.recorder);
        let choice = self.missing.get();
        let presenter: ExtensionSelectorFactory = Rc::new(move |request| {
            recorder
                .log
                .borrow_mut()
                .push(format!("missing-presented:{}", request.options.join("|")));
            tokio::task::spawn_local(async move {
                (request.on_select)(choice);
                (request.on_select)(choice);
            });
            Ok(presenters::view(&Rc::default()))
        });
        MissingSessionCwdPresentation {
            create_ui: self.screen.create_ui(),
            theme: Rc::clone(&self.themes),
            presenter,
        }
    }

    /// The export operation, recording its operands.
    fn export_operation(&self) -> MainExport {
        let recorder = Rc::clone(&self.recorder);
        let result = self.export.borrow().clone();
        Rc::new(move |source, destination| {
            recorder
                .log
                .borrow_mut()
                .push(format!("export:{source}:{destination:?}"));
            result.clone().map_err(Into::into)
        })
    }
}

/// A loopback chat-completions server answering each request in order.
pub struct Provider {
    /// The server's base address.
    pub base_url: String,
    /// The serving thread, returning each request body.
    thread: std::thread::JoinHandle<Vec<Value>>,
}

impl Provider {
    /// Serve one reply per supplied status: 200 streams `answer`, any other status fails.
    pub fn start(statuses: Vec<u16>) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            statuses
                .into_iter()
                .map(|status| {
                    let (mut stream, _) = listener.accept().unwrap();
                    let body = read_request(&mut stream).unwrap();
                    let reply = response(status);
                    stream.write_all(reply.as_bytes()).unwrap();
                    body
                })
                .collect()
        });
        Self { base_url, thread }
    }

    /// Wait until every reply was served and return the request bodies.
    pub fn join(self) -> Vec<Value> {
        self.thread.join().unwrap()
    }
}

/// A controlled streaming answer or a failed HTTP response.
fn response(status: u16) -> String {
    if status != 200 {
        return format!(
            "HTTP/1.1 {status} Failed\r\ncontent-length: 6\r\nconnection: close\r\n\r\nfailed"
        );
    }
    let chunk = json!({"id": "c", "choices": [{"index": 0,
        "delta": {"content": "answer"}, "finish_reason": "stop"}]});
    let events = format!("data: {chunk}\n\ndata: [DONE]\n\n");
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{events}",
        events.len()
    )
}

/// Read one request's headers and its declared body, failing when the peer closes first.
fn read_request(mut stream: impl Read) -> std::io::Result<Value> {
    let mut received = Vec::new();
    let mut chunk = [0; 4096];
    let (head, length) = loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        received.extend_from_slice(&chunk[..read]);
        if let Some(end) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&received[..end]).to_lowercase();
            let length = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .map_or(0, |value| value.trim().parse::<usize>().unwrap());
            break (end + 4, length);
        }
    };
    let mut body = received.split_off(head);
    let have = body.len().min(length);
    body.resize(length, 0);
    stream.read_exact(&mut body[have..])?;
    Ok(serde_json::from_slice(&body)?)
}

/// A peer that sends `sent`, then closes; reading on after the close fails the test.
struct ClosingPeer {
    /// The bytes not yet read.
    sent: &'static [u8],
    /// Whether a read already reported the close.
    closed: bool,
}

impl Read for ClosingPeer {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        assert!(!self.closed, "read again after the peer closed");
        let read = self.sent.read(buffer)?;
        self.closed = read == 0;
        Ok(read)
    }
}

#[test]
fn request_reader_fails_when_the_peer_closes_early() {
    for sent in [
        &b"POST /v1/chat/completions HTTP/1.1\r\ncontent-len"[..],
        b"POST /v1/chat/completions HTTP/1.1\r\ncontent-length: 9\r\n\r\n{}",
    ] {
        let error = read_request(ClosingPeer {
            sent,
            closed: false,
        })
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
    }
}

/// Every user text of one chat-completions request body, in order.
pub fn user_texts(body: &Value) -> Vec<String> {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["role"] == "user")
        .flat_map(|message| match &message["content"] {
            Value::String(text) => vec![text.clone()],
            Value::Array(parts) => parts
                .iter()
                .filter_map(|part| part["text"].as_str().map(str::to_owned))
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

impl Entry {
    /// Write provider `startup` over `base_url` with the supplied model identifiers.
    pub fn served_models(&self, base_url: &str, ids: &[&str]) {
        let models: Vec<Value> = ids.iter().map(|id| json!({"id": id})).collect();
        let document = json!({"providers": {"startup": {
            "baseUrl": base_url, "apiKey": "controlled",
            "api": "openai-completions", "models": models,
        }}});
        std::fs::write(
            format!("{}/models.json", self.commands.agent_dir),
            document.to_string(),
        )
        .unwrap();
    }
}
