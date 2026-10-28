//! A terminal client for streamed RPC events and extension dialogs.

#[cfg(any(unix, windows))]
use serde_json::Value;
use std::io;

/// Reads the asynchronous pipe through the platform's physical-line interface.
#[cfg(any(unix, windows))]
async fn read_events(
    reader: impl tokio::io::AsyncBufRead + Unpin,
    mut receive: impl FnMut(Value),
) -> io::Result<()> {
    use tokio::io::AsyncBufReadExt;
    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await? {
        if let Ok(record) = serde_json::from_str(&line) {
            receive(record);
        }
    }
    Ok(())
}

/// Starts the opt-in terminal demonstration.
#[cfg(any(unix, windows))]
fn main() -> io::Result<std::process::ExitCode> {
    native::main()
}

/// Reports the unavailable native terminal adapter.
#[cfg(not(any(unix, windows)))]
fn main() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "This example needs a native terminal.",
    ))
}

#[cfg(all(test, any(unix, windows)))]
use native::run_client;

#[cfg(any(unix, windows))]
#[path = "sdk/rpc_extension_ui/client.rs"]
mod client;
#[cfg(any(unix, windows))]
#[path = "sdk/rpc_extension_ui/output_log.rs"]
mod output_log;

/// Native terminal composition and the RPC conversation it retains.
#[cfg(any(unix, windows))]
mod native {
    use super::client::Client;
    use maestro_app::{
        CapturedChild, Termination, configuration::config,
        presentation_data::keybindings::KeybindingsManager, rpc_types::RpcExtensionUIResponse,
    };
    use maestro_theme::{LiveTheme, ThemeState};
    use maestro_tui::{TUI, TerminalImage, tui::InputListenerResult};
    use maestro_tui_crossterm::{ProcessTerminal, ProcessTuiRuntime};
    use serde_json::Value;
    use std::{
        cell::RefCell,
        io,
        path::PathBuf,
        process::{Command, ExitCode, ExitStatus},
        rc::Rc,
    };
    use tokio::{
        io::{AsyncReadExt, BufReader},
        process::{ChildStderr, ChildStdin, ChildStdout},
        sync::mpsc,
        task::JoinHandle,
    };

    /// UI callbacks and the pipe reader hand work to the one conversation owner.
    pub(super) enum Action {
        /// A streamed child record.
        Record(Value),
        /// The reader has delivered every stdout record before its end.
        OutputClosed,
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

    /// Prepares the existing brand-backed styles before a child is launched.
    fn theme() -> io::Result<LiveTheme> {
        let themes = Rc::new(ThemeState::new(config::get_theme_directories()?));
        themes.init_theme(None, None).map_err(io::Error::other)?;
        Ok(themes.theme())
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

    /// Leases only the output handling; lifecycle and termination stay with the process owner.
    fn pipes(child: &mut CapturedChild) -> io::Result<(ChildStdin, ChildStdout, ChildStderr)> {
        Ok((
            child
                .take_stdin()
                .ok_or_else(|| io::Error::other("Missing child stdin"))?,
            child
                .take_stdout()
                .ok_or_else(|| io::Error::other("Missing child stdout"))?,
            child
                .take_stderr()
                .ok_or_else(|| io::Error::other("Missing child stderr"))?,
        ))
    }

    /// Hands physical stdout lines to the conversation on the existing local executor.
    fn read_pipe(output: ChildStdout, sender: mpsc::UnboundedSender<Action>) -> JoinHandle<()> {
        tokio::task::spawn_local(async move {
            let records = sender.clone();
            if let Err(error) = super::read_events(BufReader::new(output), |record| {
                let _ = records.send(Action::Record(record));
            })
            .await
            {
                let _ = sender.send(Action::Failed(error));
            }
            let _ = sender.send(Action::OutputClosed);
        })
    }

    /// Collects diagnostics without blocking protocol input or terminal work.
    fn capture_errors(mut errors: ChildStderr) -> JoinHandle<io::Result<Vec<u8>>> {
        tokio::task::spawn_local(async move {
            let mut bytes = Vec::new();
            errors.read_to_end(&mut bytes).await.map(|_| bytes)
        })
    }

    /// Prints every collected diagnostic after raw-terminal cleanup, preserving read failures.
    async fn finish_errors(task: Option<JoinHandle<io::Result<Vec<u8>>>>) -> io::Result<()> {
        let Some(task) = task else {
            return Ok(());
        };
        let bytes = task.await.map_err(io::Error::other)??;
        if !bytes.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&bytes));
        }
        Ok(())
    }

    /// Runs the real terminal widgets against a caller-selected child process.
    pub(super) async fn run_client(tui: TUI, command: Command) -> io::Result<ExitStatus> {
        let theme = theme()?;
        let mut command = tokio::process::Command::from(command);
        command.kill_on_drop(true);
        let mut child = CapturedChild::spawn_piped(&mut command)?;
        let mut reader = None;
        let mut errors = None;
        let (sender, receiver) = mpsc::unbounded_channel();
        let remove = keyboard(&tui, sender.clone());
        let result = async {
            let (input, output, diagnostics) = pipes(&mut child)?;
            reader = Some(read_pipe(output, sender.clone()));
            errors = Some(capture_errors(diagnostics));
            let mut client = Client::new(tui.clone(), input, sender, theme);
            tui.start()?;
            client.run(&mut child, receiver).await
        }
        .await;
        remove();
        let frame = tui.render_pending_frame();
        let stopped = tui.stop();
        tui.set_focus(None);
        tui.clear();
        let terminated = child.terminate(Termination::Terminate).await;
        let reaped = child.wait().await;
        let read = match reader {
            Some(reader) => reader.await.map_err(io::Error::other),
            None => Ok(()),
        };
        let errors = finish_errors(errors).await;
        frame?;
        stopped?;
        terminated?;
        reaped?;
        read?;
        errors?;
        result.map(Option::unwrap_or_default)
    }
    /// Loads the shipped companion artifact and starts a native-only, opt-in RPC session.
    pub(super) fn main() -> io::Result<ExitCode> {
        let root = PathBuf::from(config::get_package_dir()?);
        let binary = root.join(format!(
            "target/debug/maestro{}",
            std::env::consts::EXE_SUFFIX
        ));
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

#[cfg(all(test, any(unix, windows)))]
#[allow(
    dead_code,
    reason = "Shared terminal controls also serve other scenarios."
)]
#[path = "../crates/maestro-test-terminal/src/recording_terminal.rs"]
mod recording_terminal;

#[cfg(test)]
#[cfg(any(unix, windows))]
mod tests {
    use super::*;

    /// A Unicode line separator belongs to the JSON string, not to the wire framing.
    #[test]
    fn streamed_unicode_line_separator_arrives_as_one_event() {
        run_async(async {
            let wire = "{\"type\":\"message_update\",\"assistantMessageEvent\":{\"type\":\"text_delta\",\"delta\":\"first\u{2028}second\"}}\r\n";
            let mut events = Vec::new();
            read_events(tokio::io::BufReader::new(wire.as_bytes()), |event| {
                events.push(event);
            })
            .await
            .unwrap();
            assert_eq!(
                events,
                vec![serde_json::json!({"type":"message_update",
            "assistantMessageEvent":{"type":"text_delta","delta":"first\u{2028}second"}})]
            );
        });
    }

    /// Drives asynchronous cases without macro-owned lint overrides.
    fn run_async(future: impl std::future::Future<Output = ()>) {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future);
    }

    /// Delivers individual key reports as the terminal adapter does.
    fn send_keys(terminal: &super::recording_terminal::RecordingTerminal, keys: &str) {
        for key in keys.chars() {
            terminal.send_input(&key.to_string());
        }
    }

    /// Observations of a controlled peer and actual retained toolkit widgets.
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
    async fn run_case(
        peer: std::process::Command,
        steps: Vec<(&'static str, &'static str)>,
    ) -> Case {
        use maestro_tui::{TUI, TerminalImage};
        use maestro_tui_crossterm::ProcessTuiRuntime;
        use std::{
            cell::{Cell, RefCell},
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
                send_keys(&input, keys);
            }
        });
        let tui = TUI::new(
            Rc::new(RefCell::new(terminal.clone())),
            Rc::new(ProcessTuiRuntime::new(local.clone())),
            TerminalImage::default(),
            None,
        );
        let status = local.run_until(super::run_client(tui, peer)).await.unwrap();
        Case {
            status,
            raw: effects.borrow().join(""),
            steps: stage.get(),
            lifecycle: terminal.events(),
        }
    }

    /// Windows uses the same pipe conversation and retained terminal lifecycle.
    #[cfg(windows)]
    #[test]
    fn windows_rpc_child_keeps_exit_status_and_terminal_lifecycle() {
        run_async(async {
            let mut peer = std::process::Command::new("powershell.exe");
            peer.args(["-NoProfile", "-NonInteractive", "-Command", r#"
[Console]::WriteLine('{"type":"extension_ui_request","id":"ready","method":"notify","message":"Windows peer ready"}')
$line = [Console]::ReadLine()
if ($line -notlike '*"type":"prompt"*' -or $line -notlike '*"message":"hello"*') { exit 12 }
exit 7
"#]);
            let case = run_case(peer, vec![("Windows peer ready", "hello\r")]).await;
            assert_eq!(case.status.code(), Some(7));
            assert!(case.raw.contains("Windows peer ready"));
            assert_eq!(case.steps, 1);
            assert_eq!(case.lifecycle, ["start", "stop"]);
        });
    }

    /// Keeps the existing pipe fixtures as ordinary shell commands.
    #[cfg(unix)]
    fn shell(script: &str) -> std::process::Command {
        let mut command = std::process::Command::new("sh");
        command.args(["-c", script]);
        command
    }

    /// Quitting gives the child its ordinary termination signal before reaping it.
    #[cfg(unix)]
    #[test]
    fn quitting_delivers_ordinary_termination_before_reaping_the_rpc_child() {
        run_async(async {
            let root = maestro_test_directory::Scratch::new().unwrap();
            let marker = root.path().join("terminated");
            let mut peer = std::process::Command::new("python3");
            peer.args(["-c", TERMINATION_PEER]).arg(&marker);
            let case = run_case(peer, vec![("Waiting for termination", "\x04")]).await;
            assert!(case.status.success());
            assert_eq!(case.steps, 1);
            assert_eq!(case.lifecycle, ["start", "stop"]);
            assert_eq!(
                std::fs::read_to_string(marker).ok().as_deref(),
                Some("SIGTERM")
            );
        });
    }

    /// Blocks termination before readiness so even an early signal is consumed by sigwait.
    #[cfg(unix)]
    const TERMINATION_PEER: &str = r#"
import json, signal, sys
signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGTERM})
print(json.dumps({"type":"extension_ui_request", "id":"ready", "method":"notify", "message":"Waiting for termination"}), flush=True)
signal.sigwait({signal.SIGTERM})
with open(sys.argv[1], "w", encoding="utf-8") as marker:
    marker.write("SIGTERM")
"#;

    /// The real toolkit receives keys while a controlled peer checks correlated wire replies.
    #[cfg(unix)]
    #[test]
    fn streamed_rpc_dialogs_keep_replies_and_terminal_lifecycle() {
        run_async(async {
            let case = run_case(
                shell(RPC_PEER),
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
        });
    }

    /// Local dialogs preserve their authored results without emitting RPC commands.
    #[cfg(unix)]
    #[test]
    fn local_dialog_demonstrations_keep_success_and_cancel_results_off_the_wire() {
        run_async(async {
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
                    shell(LOCAL_PEER),
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
                assert!(
                    results.iter().all(|result| output.contains(*result)),
                    "{results:?}: {output}"
                );
            }
        });
    }

    /// Severity remains readable even when terminal colors are removed.
    #[cfg(unix)]
    #[test]
    fn rpc_notifications_retain_their_severity_in_visible_text() {
        run_async(async {
            let case = run_case(shell(NOTIFICATION_PEER), vec![("RPC Chat", "hello\r")]).await;
            assert!(case.status.success());
            assert_eq!(case.steps, 1);
            let output = maestro_app::strip_ansi(&case.raw).unwrap();
            for expected in [
                "Notification: [info] default notice",
                "Notification: [info] information",
                "Notification: [warning] warning",
                "Notification: [error] failure",
            ] {
                assert!(output.contains(expected), "missing {expected}: {output}");
            }
        });
    }

    /// The protocol carries an optional kind, independently of the message wording.
    #[cfg(unix)]
    const NOTIFICATION_PEER: &str = r#"
read -r line || exit 8
printf '%s\n' '{"type":"response","id":"req_1","command":"prompt","success":true}'
printf '%s\n' '{"type":"extension_ui_request","id":"a","method":"notify","message":"default notice"}'
printf '%s\n' '{"type":"extension_ui_request","id":"b","method":"notify","notifyType":"info","message":"information"}'
printf '%s\n' '{"type":"extension_ui_request","id":"c","method":"notify","notifyType":"warning","message":"warning"}'
printf '%s\n' '{"type":"extension_ui_request","id":"d","method":"notify","notifyType":"error","message":"failure"}'
"#;

    /// Selected strings retain whitespace and empty values in correlated replies.
    #[cfg(unix)]
    #[test]
    fn selection_replies_preserve_padded_and_empty_values() {
        run_async(async {
            let case = run_case(
                shell(SELECTION_PEER),
                vec![("Padded choice", "\r"), ("Empty choice", "\r")],
            )
            .await;
            assert!(case.status.success(), "peer status: {}", case.status);
            assert_eq!(case.steps, 2);
        });
    }

    /// A peer validates both the identifier and exact serialized selection value.
    #[cfg(unix)]
    const SELECTION_PEER: &str = r#"
printf '%s\n' '{"type":"extension_ui_request","id":"padded","method":"select","title":"Padded choice","options":[" Allow "]}'
read -r line || exit 11
case "$line" in *'"id":"padded"'*'"value":" Allow "'*) ;; *) exit 12;; esac
printf '%s\n' '{"type":"extension_ui_request","id":"empty","method":"select","title":"Empty choice","options":[""]}'
read -r line || exit 13
case "$line" in *'"id":"empty"'*'"value":""'*) ;; *) exit 14;; esac
case "$line" in *'"cancelled":true'*) exit 15;; esac
"#;

    /// Dialog chrome and option labels cannot execute controls before approval.
    #[cfg(unix)]
    #[test]
    fn dialog_text_cannot_issue_clipboard_commands_before_approval() {
        run_async(async {
            let peer = r#"
printf '%s\n' '{"type":"extension_ui_request","id":"approval","method":"select","title":"Approve sudo echo \u001b]52;c;YWJj\u0007?","options":["Allow \u001b]52;c;YWJj\u0007"]}'
read -r line || exit 11
case "$line" in *'"id":"approval"'*'"value":"Allow \u001b]52;c;YWJj\u0007"'*) ;; *) exit 12;; esac
"#;
            let case = run_case(shell(peer), vec![("Approve sudo echo", "\r")]).await;
            assert!(case.status.success(), "peer status: {}", case.status);
            assert_eq!(case.steps, 1);
            assert!(
                !case.raw.contains("\x1b]52;"),
                "clipboard command reached terminal"
            );
        });
    }

    /// Command approval retains separate command lines and blank lines.
    #[cfg(unix)]
    #[test]
    fn approval_dialog_preserves_multiline_command_text() {
        run_async(async {
            let peer = r#"
printf '%s\n' '{"type":"extension_ui_request","id":"approval","method":"confirm","title":"Dangerous command","message":"sudo echo first\n\necho second"}'
read -r line || exit 11
case "$line" in *'"id":"approval"'*'"confirmed":true'*) ;; *) exit 12;; esac
"#;
            let case = run_case(shell(peer), vec![("Dangerous command", "\r")]).await;
            assert!(case.status.success(), "peer status: {}", case.status);
            assert_eq!(case.steps, 1);
            let output = maestro_app::strip_ansi(&case.raw).unwrap();
            let output = output
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                output.contains("Dangerous command: sudo echo first\n\necho second"),
                "approval lost command line breaks: {output:?}"
            );
        });
    }

    /// Model text, tool names and protocol errors are literal data, not terminal commands.
    #[cfg(unix)]
    #[test]
    fn streamed_literal_text_cannot_issue_terminal_control_commands() {
        run_async(async {
            let case = run_case(shell(LITERAL_PEER), vec![("RPC Chat", "hello\r")]).await;
            assert!(case.status.success());
            assert!(
                !case.raw.contains("\x1b]52;"),
                "untrusted clipboard command reached the terminal"
            );
            let output = maestro_app::strip_ansi(&case.raw).unwrap();
            for expected in ["visible", "second line", "split", "tool", "error"] {
                assert!(output.contains(expected), "missing {expected}: {output}");
            }
        });
    }

    /// Complete and split clipboard controls reach the same literal-text boundary.
    #[cfg(unix)]
    const LITERAL_PEER: &str = r#"
read -r line || exit 8
printf '%s\n' '{"type":"response","id":"req_1","command":"prompt","success":false,"error":"error \u001b]52;c;?\u0007 text"}'
printf '%s\n' '{"type":"agent_start"}'
printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"visible \u001b]52;c;?\u0007 end\nsecond line"}}'
printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":" split \u001b]"}}'
printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"52;c;?\u0007 end"}}'
printf '%s\n' '{"type":"tool_execution_start","toolName":"tool \u001b]52;c;?\u0007 name"}'
printf '%s\n' '{"type":"agent_end","messages":[]}'
"#;

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
