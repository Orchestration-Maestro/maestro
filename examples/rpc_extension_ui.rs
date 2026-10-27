//! A terminal client for streamed RPC events and extension dialogs.

#[cfg(any(unix, test))]
use serde_json::Value;
use std::io;
#[cfg(any(unix, test))]
use std::io::BufRead;

/// Reads one complete JSON event per physical line without splitting Unicode text.
#[cfg(any(unix, test))]
fn read_events(reader: impl BufRead, mut receive: impl FnMut(Value)) -> io::Result<()> {
    for line in reader.lines() {
        if let Ok(event) = serde_json::from_str(&line?) {
            receive(event);
        }
    }
    Ok(())
}

/// Starts the opt-in terminal demonstration.
#[cfg(unix)]
fn main() -> io::Result<std::process::ExitCode> {
    native::main()
}

/// Reports the unavailable native terminal adapter.
#[cfg(not(unix))]
fn main() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "This example needs a Unix terminal.",
    ))
}

#[cfg(all(test, unix))]
use native::run_client;

#[cfg(unix)]
#[path = "sdk/rpc_extension_ui/client.rs"]
mod client;
#[cfg(unix)]
#[path = "sdk/rpc_extension_ui/output_log.rs"]
mod output_log;

/// Native terminal composition and the RPC conversation it retains.
#[cfg(unix)]
mod native {
    use super::client::Client;
    use maestro_app::{
        configuration::config, presentation_data::keybindings::KeybindingsManager,
        rpc_types::RpcExtensionUIResponse,
    };
    use maestro_tui::tui::InputListenerResult;
    use maestro_tui::{TUI, TerminalImage};
    use maestro_tui_crossterm::{ProcessTerminal, ProcessTuiRuntime};
    use serde_json::Value;
    use std::{
        cell::RefCell,
        io::{self, BufReader, Read},
        os::unix::process::ExitStatusExt,
        path::PathBuf,
        process::{Child, Command, ExitCode, ExitStatus, Stdio},
        rc::Rc,
        thread,
    };
    use tokio::sync::mpsc;

    /// UI callbacks and the pipe reader hand work to the one conversation owner.
    pub(super) enum Action {
        /// A streamed child record.
        Record(Value),
        /// An ordinary prompt or local demonstration command.
        Submit(String),
        /// The active dialog's answer, paired with its original request.
        Answer(RpcExtensionUIResponse),
        /// Abort streaming, dismiss a dialog, or leave an idle client.
        Escape,
        /// Leave the client regardless of its current input.
        Quit,
        /// A pipe read failed.
        Failed(io::Error),
    }

    /// The owned child is killed and reaped even when setup or terminal output fails.
    struct Agent {
        /// The process behind the RPC pipes.
        child: Child,
    }
    impl Agent {
        /// Terminates a still-running child and always reaps its status.
        fn stop(&mut self) -> io::Result<()> {
            if self.child.try_wait()?.is_none() {
                self.child.kill()?;
            }
            self.child.wait().map(drop)
        }
    }
    impl Drop for Agent {
        fn drop(&mut self) {
            let _ = self.stop();
        }
    }

    /// Routes registered exit and interrupt actions before the focused toolkit editor.
    fn keyboard(tui: &TUI, sender: mpsc::UnboundedSender<Action>) -> Box<dyn Fn()> {
        let keys =
            KeybindingsManager::new(maestro_tui::get_keybindings().get_user_bindings(), None);
        tui.add_input_listener(Rc::new(move |data| {
            let action = if keys.matches(data, "app.exit") {
                Some(Action::Quit)
            } else if keys.matches(data, "app.interrupt") {
                Some(Action::Escape)
            } else {
                None
            };
            if let Some(action) = action {
                let _ = sender.send(action);
                InputListenerResult::Consume
            } else {
                InputListenerResult::Pass
            }
        }))
    }

    /// Hands physical stdout lines to the conversation without blocking terminal input.
    fn read_pipe(
        output: std::process::ChildStdout,
        sender: mpsc::UnboundedSender<Action>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let records = sender.clone();
            if let Err(error) = super::read_events(BufReader::new(output), |record| {
                let _ = records.send(Action::Record(record));
            }) {
                let _ = sender.send(Action::Failed(error));
            }
        })
    }

    /// Runs the real terminal widgets against a caller-selected child process.
    pub(super) async fn run_client(tui: TUI, mut command: Command) -> io::Result<ExitStatus> {
        let mut agent = Agent {
            child: command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?,
        };
        let input = agent
            .child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("Missing child stdin"))?;
        let output = agent
            .child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("Missing child stdout"))?;
        let mut errors = agent
            .child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("Missing child stderr"))?;
        let (sender, receiver) = mpsc::unbounded_channel();
        let mut client = Client::new(tui.clone(), input, sender.clone())?;
        let remove = keyboard(&tui, sender.clone());
        let reader = read_pipe(output, sender);
        let diagnostics = thread::spawn(move || {
            let mut bytes = Vec::new();
            errors.read_to_end(&mut bytes).map(|_| bytes)
        });
        let result = async {
            tui.start()?;
            client.run(&mut agent.child, receiver).await
        }
        .await;
        remove();
        let stopped = tui.stop();
        tui.set_focus(None);
        tui.clear();
        drop(client);
        let reaped = agent.stop();
        let read = reader
            .join()
            .map_err(|_| io::Error::other("RPC reader failed"));
        let errors = diagnostics
            .join()
            .map_err(|_| io::Error::other("RPC stderr reader failed"))?;
        if let Ok(bytes) = &errors
            && !bytes.is_empty()
        {
            eprint!("{}", String::from_utf8_lossy(bytes));
        }
        stopped?;
        reaped?;
        read?;
        errors?;
        result.map(|status| status.unwrap_or_else(|| ExitStatus::from_raw(0)))
    }
    /// Loads the shipped companion artifact and starts a native-only, opt-in RPC session.
    pub(super) fn main() -> io::Result<ExitCode> {
        let root = PathBuf::from(config::get_package_dir()?);
        let binary = root.join("target/debug/maestro");
        let companion = root.join("target/wasm32-wasip2/release/examples/rpc_demo.wasm");
        if !binary.is_file() || !companion.is_file() {
            return Err(io::Error::other(
                "Build maestro and the rpc_demo component first; see examples/README.md",
            ));
        }
        let mut command = Command::new(binary);
        command
            .args([
                "--mode",
                "rpc",
                "--no-session",
                "--no-extensions",
                "--extension",
            ])
            .arg(companion);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let local = Rc::new(tokio::task::LocalSet::new());
        let tui = TUI::new(
            Rc::new(RefCell::new(ProcessTerminal::new(local.clone()))),
            Rc::new(ProcessTuiRuntime::new(local.clone())),
            TerminalImage::default(),
            None,
        );
        runtime
            .block_on(local.run_until(run_client(tui, command)))
            .map(|status| {
                ExitCode::from(
                    status
                        .code()
                        .and_then(|code| u8::try_from(code).ok())
                        .unwrap_or(1),
                )
            })
    }
}

#[cfg(all(test, unix))]
#[allow(
    dead_code,
    reason = "Shared terminal controls also serve other scenarios."
)]
#[path = "../crates/maestro-test-terminal/src/recording_terminal.rs"]
mod recording_terminal;

#[cfg(test)]
mod tests {
    use super::*;

    /// A Unicode line separator belongs to the JSON string, not to the wire framing.
    #[test]
    fn streamed_unicode_line_separator_arrives_as_one_event() {
        let wire = "{\"type\":\"message_update\",\"assistantMessageEvent\":{\"type\":\"text_delta\",\"delta\":\"first\u{2028}second\"}}\r\n";
        let mut events = Vec::new();
        read_events(wire.as_bytes(), |event| events.push(event)).unwrap();
        assert_eq!(
            events,
            vec![serde_json::json!({"type":"message_update",
            "assistantMessageEvent":{"type":"text_delta","delta":"first\u{2028}second"}})]
        );
    }

    /// Observations of a controlled peer and actual retained toolkit widgets.
    #[cfg(unix)]
    struct Case {
        /// The child process's propagated status.
        status: std::process::ExitStatus,
        /// Every terminal effect delivered through the existing recording tap.
        raw: String,
        /// Number of completion-driven input transitions.
        steps: usize,
        /// Native terminal lifecycle effects.
        lifecycle: Vec<&'static str>,
    }

    /// Runs one pipe peer while rendered prompts drive deterministic input.
    #[cfg(unix)]
    async fn run_case(script: &str, steps: Vec<(&'static str, &'static str)>) -> Case {
        use maestro_tui::{TUI, TerminalImage};
        use maestro_tui_crossterm::ProcessTuiRuntime;
        use std::{
            cell::{Cell, RefCell},
            process::Command,
            rc::Rc,
        };

        let local = Rc::new(tokio::task::LocalSet::new());
        let terminal = super::recording_terminal::RecordingTerminal::new(80, 24);
        let stage = Rc::new(Cell::new(0));
        let observed = stage.clone();
        let input = terminal.clone();
        let effects = Rc::new(RefCell::new(Vec::new()));
        let captured = effects.clone();
        terminal.set_tap(move |frame| {
            captured.borrow_mut().push(frame.to_owned());
            let Some((needle, keys)) = steps.get(observed.get()) else {
                return;
            };
            if frame.contains(needle) {
                observed.set(observed.get() + 1);
                for key in keys.chars() {
                    input.send_input(&key.to_string());
                }
            }
        });
        let tui = TUI::new(
            Rc::new(RefCell::new(terminal.clone())),
            Rc::new(ProcessTuiRuntime::new(local.clone())),
            TerminalImage::default(),
            None,
        );
        let mut peer = Command::new("sh");
        peer.args(["-c", script]);
        let status = local.run_until(super::run_client(tui, peer)).await.unwrap();
        Case {
            status,
            raw: effects.borrow().join(""),
            steps: stage.get(),
            lifecycle: terminal.events(),
        }
    }

    /// The real toolkit receives keys while a controlled peer checks correlated wire replies.
    #[cfg(unix)]
    #[tokio::test(flavor = "current_thread")]
    async fn streamed_rpc_dialogs_keep_replies_and_terminal_lifecycle() {
        let case = run_case(
            RPC_PEER,
            vec![
                ("RPC Chat", "hello\r"),
                ("Dangerous command?", "\r"),
                ("Clear session?", "\r"),
                ("Enter a value", "answer\r"),
                ("Edit some text", "\r"),
                ("Abort now", "\x1b"),
            ],
        )
        .await;
        assert_eq!(case.status.code(), Some(7));
        assert_eq!(case.steps, 6);
        assert!(case.raw.contains("\x1b]0;RPC Demo\x07"));
        let output = maestro_app::strip_ansi(&case.raw).unwrap();
        for expected in [
            "first\u{2028}second",
            "controlled refusal",
            "Turns: 1",
            "Loaded and ready.",
            "Editor prefilled",
            "preset",
        ] {
            assert!(output.contains(expected), "missing {expected}: {output}");
        }
        assert_eq!(case.lifecycle, ["start", "stop"]);
    }

    /// Local dialogs preserve their authored results without emitting RPC commands.
    #[cfg(unix)]
    #[tokio::test(flavor = "current_thread")]
    async fn local_dialog_demonstrations_keep_success_and_cancel_results_off_the_wire() {
        for (select, confirm, input, editor, results) in [
            (
                "\r",
                "\r",
                "Maestro\r",
                "\r",
                [
                    "You picked: Red",
                    "Confirmed: true",
                    "You entered: Maestro",
                    "Submitted: Hello, world!",
                ],
            ),
            (
                "\x1b",
                "\x1b",
                "\x1b",
                "\x1b",
                [
                    "Selection cancelled",
                    "Confirmed: false",
                    "Input cancelled",
                    "Editor cancelled",
                ],
            ),
        ] {
            let case = run_case(
                LOCAL_PEER,
                vec![
                    ("RPC Chat", "/select\r"),
                    ("Pick a color", select),
                    (results[0], "/confirm\r"),
                    ("Are you sure?", confirm),
                    (results[1], "/input\r"),
                    ("Enter your name", input),
                    (results[2], "/editor\r"),
                    ("Edit text", editor),
                    (results[3], "after local\r"),
                ],
            )
            .await;
            assert_eq!(case.status.code(), Some(0));
            assert_eq!(case.steps, 9);
            assert_eq!(case.lifecycle, ["start", "stop"]);
            let output = maestro_app::strip_ansi(&case.raw).unwrap();
            assert!(!output.contains("Are you sure?:"), "{output}");
            for result in results {
                assert!(output.contains(result), "{output}");
            }
        }
    }

    /// The peer must receive only the final ordinary prompt with the first identifier.
    #[cfg(unix)]
    const LOCAL_PEER: &str = r#"
read -r line || exit 8
case "$line" in *'"message":"after local"'*) ;; *) exit 9;; esac
case "$line" in *'"id":"req_1"'*) ;; *) exit 10;; esac
printf '%s\n' '{"type":"response","id":"req_1","command":"prompt","success":true}'
exit 0
"#;

    /// A shell peer uses only pipes and completion signals, without a model or a clock.
    #[cfg(unix)]
    const RPC_PEER: &str = r#"
read -r line || exit 91
case "$line" in *'"id":"req_1"'*) ;; *) exit 92;; esac
case "$line" in *'"message":"hello"'*) ;; *) exit 93;; esac
printf '%s\n' '{"type":"response","id":"req_1","command":"prompt","success":false,"error":"controlled refusal"}'
printf '%s\n' '{"type":"agent_start"}' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"first\u2028second"}}'
printf '%s\n' '{"type":"extension_ui_request","id":"s","method":"select","title":"Dangerous command?","options":["Allow","Block"]}'
read -r line || exit 94
case "$line" in *'"id":"s"'*) ;; *) exit 95;; esac
case "$line" in *'"value":"Allow"'*) ;; *) exit 96;; esac
printf '%s\n' '{"type":"extension_ui_request","id":"c","method":"confirm","title":"Clear session?","message":"All messages will be lost."}'
read -r line || exit 97
case "$line" in *'"id":"c"'*) ;; *) exit 98;; esac
case "$line" in *'"confirmed":true'*) ;; *) exit 99;; esac
printf '%s\n' '{"type":"extension_ui_request","id":"i","method":"input","title":"Enter a value","placeholder":"type something..."}'
read -r line || exit 100
case "$line" in *'"id":"i"'*) ;; *) exit 101;; esac
case "$line" in *'"value":"answer"'*) ;; *) exit 102;; esac
printf '%s\n' '{"type":"extension_ui_request","id":"e","method":"editor","title":"Edit some text","prefill":"Line 1\nLine 2\nLine 3"}'
read -r line || exit 103
case "$line" in *'"id":"e"'*) ;; *) exit 104;; esac
case "$line" in *'"value":"Line 1\nLine 2\nLine 3"'*) ;; *) exit 105;; esac
printf '%s\n' '{"type":"extension_ui_request","id":"n","method":"notify","message":"Editor prefilled"}'
printf '%s\n' '{"type":"extension_ui_request","id":"t","method":"setTitle","title":"RPC Demo"}'
printf '%s\n' '{"type":"extension_ui_request","id":"st","method":"setStatus","statusKey":"rpc-demo","statusText":"Turns: 1"}'
printf '%s\n' '{"type":"extension_ui_request","id":"w","method":"setWidget","widgetKey":"rpc-demo","widgetLines":["Loaded and ready."]}'
printf '%s\n' '{"type":"extension_ui_request","id":"p","method":"set_editor_text","text":"preset"}'
printf '%s\n' '{"type":"extension_ui_request","id":"a","method":"notify","message":"Abort now"}'
read -r line || exit 106
case "$line" in *'"id":"req_2"'*) ;; *) exit 107;; esac
case "$line" in *'"type":"abort"'*) ;; *) exit 108;; esac
printf '%s\n' '{"type":"response","id":"req_2","command":"abort","success":true}' '{"type":"agent_end","messages":[]}'
printf '%s\n' 'controlled peer exited' >&2
exit 7
"#;
}
