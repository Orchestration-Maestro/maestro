//! The standard-stream adapter against the application's connection runner over memory,
//! with managed output, workspace commands and framing.
#![cfg(unix)]
use crate::support::{Child, Served, options, runtime, serve};
use maestro_app::rpc_mode::{JsonlLineReader, RpcLine, RpcOutput};
use maestro_app::rpc_types::workspace_schema::{ErrorBody, EventData, ResponseResult};
use maestro_app::{ExtensionFactory, Local, RpcConnectionOutcome, run_rpc_connection};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
use tokio::task::LocalSet;

/// Commands whose answers do not depend on the session's identity, ending with a final
/// line without its LF.
const COMMANDS: &str = concat!(
    "{\"id\":\"a\",\"type\":\"get_last_assistant_text\"}\n",
    "{\"id\":\"b\",\"type\":\"get_fork_messages\"}\n",
    "{\"id\":\"c\",\"type\":\"set_session_name\",\"name\":\" \"}\n",
    "{\"id\":\"d\",\"type\":\"unknown\"}\n",
    "{\n",
    "{\"id\":\"e\",\"type\":\"get_messages\"}",
);

/// Whether this process is a serving child; the child serves and never returns.
fn serve_when_child(served: impl FnOnce() -> Served) {
    if maestro_test_process::selection().is_child() {
        serve(served());
    }
}

/// The lines the connection runner writes for `input` over memory, and how it ended.
fn over_memory(input: &'static str) -> (Vec<String>, RpcConnectionOutcome) {
    let local = Rc::new(LocalSet::new());
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    executor.block_on(local.run_until(async {
        let runtime = runtime(&local, Vec::new()).await;
        let lines = Rc::new(RefCell::new(Vec::new()));
        let written = Rc::clone(&lines);
        let output = RpcOutput::synchronous(Rc::new(move |line: &RpcLine| {
            written.borrow_mut().push(line.as_str().to_owned());
            Ok(())
        }));
        let mut options = options(
            Rc::clone(&local),
            Rc::new(runtime),
            Served::default().workspace,
        );
        options.session.as_mut().unwrap().process = Rc::new(NoSignals);
        let reader = Box::new(JsonlLineReader::new(input.as_bytes()));
        let outcome = run_rpc_connection(reader, output, options).await;
        let lines = lines
            .borrow()
            .iter()
            .map(|line| line.trim_end().to_owned())
            .collect();
        (lines, outcome.unwrap())
    }))
}

/// Signal registration that delivers nothing, for a connection inside the test process.
struct NoSignals;

impl maestro_app::rpc_mode::RpcProcessPort for NoSignals {
    fn register_signals(
        &self,
        _callback: Rc<dyn Fn(maestro_app::TerminationSignal)>,
    ) -> std::io::Result<Box<dyn Fn()>> {
        Ok(Box::new(|| {}))
    }
}

/// The same commands give the same lines over memory and over the standard streams; memory
/// ends with an outcome, and the standard streams exit 0 without a readiness record.
#[test]
fn memory_and_stdio_use_one_dispatch_path() {
    serve_when_child(Served::default);
    let scratch = Scratch::new().unwrap();
    let mut child = Child::start(
        "memory_and_stdio_use_one_dispatch_path",
        "serve",
        scratch.path(),
    );
    child.send(COMMANDS.as_bytes());
    let (status, lines, _) = child.finish();
    assert_eq!(status.code(), Some(0));
    let (expected, outcome) = over_memory(COMMANDS);
    assert_eq!(outcome, RpcConnectionOutcome::Eof);
    assert_eq!(lines, expected);
    let first: Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(
        first,
        json!({"id": "a", "type": "response", "command": "get_last_assistant_text",
            "success": true, "data": {}})
    );
    assert_eq!(lines.len(), 6);
}

/// An extension that writes managed output when the session starts.
fn noisy() -> ExtensionFactory<Local> {
    Rc::new(|api| {
        let started = api.on::<maestro_app::SessionStartEvent, ()>(
            "session_start".into(),
            Rc::new(|_, _| {
                maestro_app::output_guard::write_stdout("managed noise\n").unwrap();
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        Box::pin(std::future::ready(started))
    })
}

/// Managed output goes to standard error, so standard output holds only protocol records.
#[test]
fn managed_stdout_stays_off_protocol_wire() {
    serve_when_child(|| Served {
        factories: vec![noisy()],
        ..Served::default()
    });
    let scratch = Scratch::new().unwrap();
    let mut child = Child::start(
        "managed_stdout_stays_off_protocol_wire",
        "serve",
        scratch.path(),
    );
    child.send(b"{\"id\":\"s\",\"type\":\"get_state\"}\n");
    let state = child.next();
    assert_eq!(
        (&state["id"], &state["success"]),
        (&json!("s"), &json!(true))
    );
    let (status, lines, errors) = child.finish();
    assert_eq!(status.code(), Some(0));
    assert!(lines.is_empty(), "{lines:?}");
    assert_eq!(errors, "managed noise\n");
}

/// Workspace operations answering ping, subscribing with one event, and refusing the rest
/// with a coded error.
fn workspace() -> Served {
    Served {
        workspace: Rc::new(|command, events| {
            let command = serde_json::to_value(command).unwrap();
            match command["type"].as_str().unwrap() {
                "ping" => Ok(serde_json::from_value::<ResponseResult>(
                    json!({"type": "pong", "version": "1.2.3", "protocol": 1}),
                )
                .unwrap()),
                "events.subscribe" => {
                    let event = json!({"type": "workspace_focused", "workspace_id": "w1"});
                    events(serde_json::from_value::<EventData>(event).unwrap()).unwrap();
                    Ok(serde_json::from_value(json!({"type": "subscription_started"})).unwrap())
                }
                _ => Err(ErrorBody {
                    code: "workspace_not_found".into(),
                    message: "no workspace w9".into(),
                }),
            }
        }),
        ..Served::default()
    }
}

/// A workspace command crosses the supplied operations and is answered as a correlated
/// success.
#[test]
fn workspace_commands_cross_application_seam() {
    serve_when_child(workspace);
    let scratch = Scratch::new().unwrap();
    let mut child = Child::start(
        "workspace_commands_cross_application_seam",
        "serve",
        scratch.path(),
    );
    child.send(b"{\"id\":\"p\",\"type\":\"ping\"}\n");
    assert_eq!(
        child.next(),
        json!({"id": "p", "type": "response", "command": "ping", "success": true,
            "data": {"type": "pong", "version": "1.2.3", "protocol": 1, "capabilities": null}})
    );
    let (status, lines, _) = child.finish();
    assert_eq!((status.code(), lines.len()), (Some(0), 0));
}

/// A workspace failure keeps its code, a subscription's event uses the type-tagged format
/// before its answer, and the connection keeps serving; nothing is unsolicited.
#[test]
fn workspace_errors_and_opt_in_events_keep_fields() {
    serve_when_child(workspace);
    let scratch = Scratch::new().unwrap();
    let mut child = Child::start(
        "workspace_errors_and_opt_in_events_keep_fields",
        "serve",
        scratch.path(),
    );
    child.send(b"{\"id\":\"g\",\"type\":\"workspace.get\",\"workspace_id\":\"w9\"}\n");
    assert_eq!(
        child.next(),
        json!({"id": "g", "type": "response", "command": "workspace.get", "success": false,
            "error": "no workspace w9", "code": "workspace_not_found"})
    );
    child.send(b"{\"id\":\"s\",\"type\":\"events.subscribe\",\"subscriptions\":[]}\n");
    assert_eq!(
        child.next(),
        json!({"type": "workspace_focused", "workspace_id": "w1"})
    );
    assert_eq!(
        child.next(),
        json!({"id": "s", "type": "response", "command": "events.subscribe",
            "success": true, "data": {"type": "subscription_started"}})
    );
    child.send(b"{\"id\":\"n\",\"type\":\"get_last_assistant_text\"}\n");
    assert_eq!(child.next()["id"], "n");
    let (status, lines, _) = child.finish();
    assert_eq!((status.code(), lines.len()), (Some(0), 0));
}

/// The standard streams frame records with the shared splitter: a character split across
/// writes, raw line and paragraph separators, CRLF with one CR removed, blank records and
/// a final record without its LF.
#[test]
fn framing_reuse_handles_chunks_blanks_and_detach() {
    serve_when_child(Served::default);
    let scratch = Scratch::new().unwrap();
    let mut child = Child::start(
        "framing_reuse_handles_chunks_blanks_and_detach",
        "serve",
        scratch.path(),
    );
    let renamed =
        "{\"id\":\"n\",\"type\":\"set_session_name\",\"name\":\"\u{e9}\u{2028}x\u{2029}y\"}\n";
    let (head, tail) = renamed
        .as_bytes()
        .split_at(renamed.find('\u{e9}').unwrap() + 1);
    child.send(head);
    child.send(tail);
    assert_eq!(
        child.next(),
        json!({"type": "session_info_changed", "name": "\u{e9}\u{2028}x\u{2029}y"})
    );
    assert_eq!(child.next()["success"], true);
    child.send(b"{\"id\":\"s\",\"type\":\"get_state\"}\r\n");
    let state = child.next_line();
    assert!(
        state.contains("\"sessionName\":\"\u{e9}\u{2028}x\u{2029}y\""),
        "{state}"
    );
    for (blank, column) in [("\r\r\n", 1), ("\n", 0)] {
        child.send(blank.as_bytes());
        assert_eq!(
            child.next()["error"],
            format!("Failed to parse command: EOF while parsing a value at line 1 column {column}")
        );
    }
    child.send(b"{\"id\":\"t\",\"type\":\"get_last_assistant_text\"}");
    let (status, lines, _) = child.finish();
    assert_eq!(status.code(), Some(0));
    assert_eq!(
        lines,
        [
            r#"{"id":"t","type":"response","command":"get_last_assistant_text","success":true,"data":{}}"#
        ]
    );
}

#[test]
fn native_process_reopens_and_shares_termination_delivery() {
    use maestro_app::rpc_mode::RpcProcessPort;
    if !maestro_test_process::selection().is_child() {
        let output = maestro_test_process::TestProcess::current(
            "native_process_reopens_and_shares_termination_delivery",
        )
        .unwrap()
        .case("child")
        .nocapture()
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    LocalSet::new().block_on(&runtime, async {
        let process = maestro_rpc::NativeRpcProcess;
        let old = process.register_signals(Rc::new(|_| {})).unwrap();
        old();
        let (sent, mut received) = tokio::sync::mpsc::unbounded_channel();
        let rpc = process
            .register_signals(Rc::new(move |signal| {
                sent.send(signal).unwrap();
            }))
            .unwrap();
        let (sent, mut print_received) = tokio::sync::mpsc::unbounded_channel();
        let print = maestro_app::register_signals(Rc::new(move |signal| {
            sent.send(signal).unwrap();
        }))
        .unwrap();
        old();
        for signal in [
            maestro_app::TerminationSignal::Terminate,
            maestro_app::TerminationSignal::Hangup,
        ] {
            raise_termination(signal);
            assert_eq!(received.recv().await, Some(signal));
            assert_eq!(print_received.recv().await, Some(signal));
        }
        rpc();
        rpc();
        raise_termination(maestro_app::TerminationSignal::Terminate);
        assert_eq!(
            print_received.recv().await,
            Some(maestro_app::TerminationSignal::Terminate)
        );
        assert!(received.try_recv().is_err());
        print();
    });
}

/// Sends one real native signal to the calling test child.
fn raise_termination(signal: maestro_app::TerminationSignal) {
    assert!(
        std::process::Command::new("kill")
            // `kill -s` takes only names on some hosts; every host takes `-<number>`.
            .args([
                format!("-{}", signal.as_raw()),
                std::process::id().to_string()
            ])
            .status()
            .unwrap()
            .success()
    );
}
