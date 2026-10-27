//! A terminal client for streamed RPC events and extension dialogs.

use serde_json::Value;
use std::io::{self, BufRead};

/// Reads one complete JSON event per physical line without splitting Unicode text.
fn read_events(reader: impl BufRead, mut receive: impl FnMut(Value)) -> io::Result<()> {
    for line in reader.lines() {
        if let Ok(event) = serde_json::from_str(&line?) {
            receive(event);
        }
    }
    Ok(())
}

/// Starts the opt-in terminal demonstration.
fn main() -> io::Result<()> {
    read_events(io::stdin().lock(), |_| {})
}

/// Runs the retained terminal client against its owned RPC child.
#[cfg(all(test, unix))]
fn run_client(
    tui: maestro_tui::TUI,
    command: std::process::Command,
) -> std::future::Ready<io::Result<std::process::ExitStatus>> {
    drop((tui, command));
    std::future::ready(Err(io::Error::other("RPC client is not running")))
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

    /// The real toolkit receives keys while a controlled peer checks correlated wire replies.
    #[cfg(unix)]
    #[tokio::test(flavor = "current_thread")]
    async fn streamed_rpc_dialogs_keep_replies_and_terminal_lifecycle() {
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
            let actions = [
                ("RPC Chat", "hello\r"),
                ("Dangerous command?", "\r"),
                ("Clear session?", "\r"),
                ("Enter a value", "answer\r"),
                ("Edit some text", "\r"),
                ("Abort now", "\x1b"),
            ];
            let Some((needle, keys)) = actions.get(observed.get()) else {
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
        peer.args(["-c", RPC_PEER]);
        let status = local.run_until(super::run_client(tui, peer)).await.unwrap();
        assert_eq!(status.code(), Some(7));
        assert_eq!(stage.get(), 6);
        let raw = effects.borrow().join("");
        assert!(raw.contains("\x1b]0;RPC Demo\x07"));
        let output = maestro_app::strip_ansi(&raw).unwrap();
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
        assert_eq!(terminal.events(), ["start", "stop"]);
    }

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
