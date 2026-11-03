//! Explicit command execution through the authored component and production factory.
#![cfg(test)]
use super::example_process::{install, run};
use maestro_extensions::{
    Cancellation, EventBus, ExtensionRunner, Local, create_event_bus_for, types::ExtensionHandle,
};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The native programs of one test: the disposable directory that owns them and the program
/// already installed for each scenario.
struct Programs {
    /// Deletes the installed programs when the test ends.
    directory: Scratch,
    /// The path of the program installed for each scenario record.
    installed: RefCell<HashMap<String, String>>,
}

impl Programs {
    /// An empty set of programs.
    fn new() -> Self {
        Self {
            directory: Scratch::new().unwrap(),
            installed: RefCell::default(),
        }
    }

    /// The path of a program playing `scenario` of the component execution family.
    fn add(&self, scenario: &Value) -> String {
        self.add_in("component-exec", scenario)
    }

    /// The path of a program playing `scenario` of `family`, installed on first use.
    fn add_in(&self, family: &str, scenario: &Value) -> String {
        let key = format!("{family}\n{scenario}");
        let mut installed = self.installed.borrow_mut();
        let name = format!("program-{}", installed.len());
        installed
            .entry(key)
            .or_insert_with(|| {
                install(self.directory.path(), &name, family, scenario)
                    .to_str()
                    .unwrap()
                    .to_owned()
            })
            .clone()
    }

    /// The path of a program that ends with `code` after writing the given stream bytes.
    fn emitting(&self, stdout: &[u8], stderr: &[u8], code: u8) -> String {
        self.add(&json!({"scenario":"emit", "stdout":stdout, "stderr":stderr, "code":code}))
    }
}

/// The exit code a killed child reports: Windows ends it with code 1, while a child a
/// signal ended on Unix has no code and reports 0.
const KILLED: i32 = if cfg!(windows) { 1 } else { 0 };

/// Whether a read saw the child's end of a socket close. Windows resets the connection of
/// a killed process instead of closing it.
fn closed(read: &std::io::Result<usize>) -> bool {
    match read {
        Ok(read) => *read == 0,
        Err(error) => error.kind() == std::io::ErrorKind::ConnectionReset,
    }
}

/// Uses the existing fixture's production artifact loader.
async fn loaded() -> ExtensionRunner<Local> {
    observed().await.1
}

/// Loads the fixture keeping its owner and bus, so tests can observe abort notifications
/// and released captures.
async fn observed() -> (
    ExtensionHandle<Local>,
    ExtensionRunner<Local>,
    EventBus<Local>,
) {
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no errors")),
    );
    super::retained_components::loaded_on(bus.for_extension()).await
}

/// Sends literal operands through the guest facade, observing the fixture's result text.
async fn execute(runner: &ExtensionRunner<Local>, request: Value) -> Value {
    execute_on(runner, runner.create_command_context(), request).await
}

/// Invokes the same captured facade through a caller-selected later context.
async fn execute_on(
    runner: &ExtensionRunner<Local>,
    context: maestro_extensions::ExtensionCommandContext,
    request: Value,
) -> Value {
    let outcome = outcome(runner, context, request).await;
    serde_json::from_str(&outcome)
        .unwrap_or_else(|_| panic!("explicit execution unavailable: {outcome}"))
}

/// The fixture's raw reported text: result JSON or the execution's error.
async fn outcome(
    runner: &ExtensionRunner<Local>,
    context: maestro_extensions::ExtensionCommandContext,
    request: Value,
) -> String {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    command(format!("exec imports:{request}"), context)
        .await
        .unwrap_err()
        .to_string()
}

/// A request for a child that writes its first two operands to its streams, reports READY on
/// the test's socket, named by its third operand, then waits for one release byte, with
/// `fields` merged in and `port` as the third operand, when it has one.
fn pending_request(programs: &Programs, fields: &Value, port: u16) -> Value {
    let command = programs.add(&json!({"scenario":"pending"}));
    let mut request = json!({"command":command, "args":["", "", ""], "options":{"cwd":""}});
    for (key, value) in fields.as_object().unwrap() {
        request[key] = value.clone();
    }
    if let Some(slot) = request["args"].get_mut(2) {
        *slot = port.to_string().into();
    }
    request
}

/// Accepts the child's connection and its READY announcement.
async fn ready(listener: &tokio::net::TcpListener) -> tokio::net::TcpStream {
    let (mut socket, _) = listener.accept().await.unwrap();
    let mut ready = [0; 5];
    socket.read_exact(&mut ready).await.unwrap();
    assert_eq!(&ready, b"READY");
    socket
}

/// Runs a [`pending_request`]; once READY arrives, `on_ready` either releases the
/// child or keeps its socket open until settlement.
async fn pending(
    runner: &ExtensionRunner<Local>,
    programs: &Programs,
    fields: Value,
    on_ready: impl FnOnce(tokio::net::TcpStream) -> Option<tokio::net::TcpStream>,
) -> String {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let request = pending_request(programs, &fields, listener.local_addr().unwrap().port());
    let execution = outcome(runner, runner.create_command_context(), request);
    tokio::pin!(execution);
    let _held = tokio::select! {
        result = &mut execution => return result,
        socket = ready(&listener) => on_ready(socket),
    };
    execution.await
}

/// Starts a [`pending_request`] on its own local task once its child reports READY.
async fn started(
    runner: &ExtensionRunner<Local>,
    programs: &Programs,
    fields: &Value,
) -> (tokio::task::JoinHandle<Value>, tokio::net::TcpStream) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let request = pending_request(programs, fields, listener.local_addr().unwrap().port());
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let execution = command(
        format!("exec imports:{request}"),
        runner.create_command_context(),
    );
    let task = tokio::task::spawn_local(async move {
        serde_json::from_str(&execution.await.unwrap_err().to_string()).unwrap()
    });
    (task, ready(&listener).await)
}

/// Advances the paused clock to just before `boundary`, then checks that the execution
/// has not settled and the child has not closed its socket.
async fn assert_running_until(
    execution: &tokio::task::JoinHandle<Value>,
    socket: &tokio::net::TcpStream,
    boundary: tokio::time::Instant,
) {
    let before = boundary - std::time::Duration::from_millis(1);
    tokio::time::advance(before.saturating_duration_since(tokio::time::Instant::now())).await;
    tokio::task::yield_now().await;
    assert!(!execution.is_finished(), "settled before the boundary");
    let open = socket.try_read(&mut [0; 1]).unwrap_err().kind();
    assert_eq!(
        open,
        std::io::ErrorKind::WouldBlock,
        "ended before the boundary"
    );
}

/// Advances the paused clock just past `boundary`, then waits for the child to close its
/// socket. The blocking read holds the clock still, so only a deadline already passed can
/// end the child; a later one never fires.
async fn assert_ended_by(socket: tokio::net::TcpStream, boundary: tokio::time::Instant) {
    let after = boundary + std::time::Duration::from_millis(1);
    tokio::time::advance(after.saturating_duration_since(tokio::time::Instant::now())).await;
    let mut socket = socket.into_std().unwrap();
    socket.set_nonblocking(false).unwrap();
    let read = tokio::task::spawn_blocking(move || std::io::Read::read(&mut socket, &mut [0; 1]))
        .await
        .unwrap();
    assert!(
        closed(&read),
        "the child ended once the boundary passed: {read:?}"
    );
}

/// Lets a pending child finish normally.
fn release(socket: tokio::net::TcpStream) -> Option<tokio::net::TcpStream> {
    assert_eq!(socket.try_write(b"g").unwrap(), 1);
    drop(socket);
    None
}

#[test]
fn exec_literal_arguments_and_directory_through_host() {
    run(async {
        let scratch = Scratch::new().unwrap();
        let cwd = scratch.path().join("cwd with spaces");
        std::fs::create_dir(&cwd).unwrap();
        let runner = loaded().await;
        let programs = Programs::new();
        let literal = ["a b", "'quoted'", "\"double\"", "*", "$HOME", ";", ""];
        let report = programs.add(&json!({"scenario":"report"}));
        let result = execute(
            &runner,
            json!({"command":report, "args":literal, "options":{"cwd":cwd}}),
        )
        .await;
        let stdout: Value = serde_json::from_str(result["stdout"].as_str().unwrap()).unwrap();
        assert_eq!(stdout, json!([literal, cwd, 0]));
        assert_eq!(result["stderr"], "");
        assert_eq!(result["code"], 0);
        assert_eq!(result["killed"], false);
    });
}

#[test]
fn exec_absent_and_empty_directory_choose_distinct_sources() {
    run(async {
        let scratch = Scratch::new().unwrap();
        let override_cwd = scratch.path().join("overridden cwd");
        std::fs::create_dir(&override_cwd).unwrap();
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no errors")),
        );
        let (_owner, runner, _events) = super::retained_components::loaded_at(
            bus.for_extension(),
            super::session::runner_session("/initial-context").0,
            scratch.path().to_str().unwrap(),
        )
        .await;
        let inherited = std::env::current_dir().unwrap();
        let programs = Programs::new();
        let request = json!({"command":programs.add(&json!({"scenario":"cwd"})), "args":[]});
        for (options, expected) in [
            (None, scratch.path()),
            (Some(Value::Null), scratch.path()),
            (Some(json!({})), scratch.path()),
            (Some(json!({"cwd":null})), scratch.path()),
            (Some(json!({"cwd":override_cwd})), override_cwd.as_path()),
            (Some(json!({"cwd":""})), inherited.as_path()),
        ] {
            let mut request = request.clone();
            if let Some(options) = options {
                request["options"] = options;
            }
            let result = execute(&runner, request).await;
            assert_eq!(
                result,
                json!({"stdout":expected, "stderr":"", "code":0, "killed":false})
            );
        }
        let later = loaded().await;
        let context = later.create_command_context();
        assert_eq!(context.cwd().unwrap(), "/render-domain");
        let result = execute_on(&runner, context, request).await;
        assert_eq!(result["stdout"], json!(scratch.path()));
    });
}

#[test]
fn exec_preaborted_signal_is_forwarded() {
    run(async {
        let runner = loaded().await;
        let programs = Programs::new();
        let result = pending(&runner, &programs, json!({"signal":"preaborted"}), release).await;
        assert_eq!(
            serde_json::from_str::<Value>(&result).unwrap(),
            json!({"stdout":"", "stderr":"", "code":KILLED, "killed":true})
        );
    });
}

#[test]
#[cfg(unix)]
fn exec_unrequested_signal_exit_is_not_killed() {
    run(async {
        let runner = loaded().await;
        let programs = Programs::new();
        let terminating = programs.add_in(
            "command",
            &json!({"scenario":"output", "stdout":b"before", "stderr":b"diagnostic", "code":0, "signal":"SIGTERM"}),
        );
        let result = execute(
            &runner,
            json!({"command":terminating, "args":[], "options":{"cwd":""}}),
        )
        .await;
        assert_eq!(
            result,
            json!({"stdout":"before", "stderr":"diagnostic", "code":0, "killed":false})
        );
    });
}

#[test]
fn exec_output_keeps_streams_and_status_separate() {
    run(async {
        let runner = loaded().await;
        let programs = Programs::new();
        for (code, full) in [(0, false), (7, false), (0, true), (7, true)] {
            let program = if full {
                programs.emitting(
                    "\x1b[31mred\x1b[0m\r\n\0雪🙂".as_bytes(),
                    "\x1b[2mdiag\x1b[0m\n\r\0Ω".as_bytes(),
                    code,
                )
            } else {
                programs.emitting(b"", b"", code)
            };
            let result = execute(
                &runner,
                json!({"command":program, "args":[], "options":{"cwd":""}}),
            )
            .await;
            assert_eq!(
                result["stdout"],
                if full {
                    "\x1b[31mred\x1b[0m\r\n\0雪🙂"
                } else {
                    ""
                }
            );
            assert_eq!(
                result["stderr"],
                if full {
                    "\x1b[2mdiag\x1b[0m\n\r\0Ω"
                } else {
                    ""
                }
            );
            assert_eq!(result["code"], code);
            assert_eq!(result["killed"], false);
        }
    });
}

#[test]
fn exec_caller_signal_cancels_pending_child() {
    run(async {
        let (_owner, runner, events) = observed().await;
        let (observations, _seen) = super::abort_observers::observe(&events);
        let signal = Cancellation::new();
        super::abort_observers::bind(&runner, &signal);
        let fields = json!({"args":["partial-out", "partial-err", ""], "signal":"relay"});
        let programs = Programs::new();
        let result = pending(&runner, &programs, fields, |socket| {
            assert!(!signal.is_aborted());
            signal.abort();
            assert_eq!(super::abort_observers::labels(&observations), ["relay"]);
            Some(socket)
        })
        .await;
        assert_eq!(
            serde_json::from_str::<Value>(&result).unwrap(),
            json!({"stdout":"partial-out", "stderr":"partial-err", "code":KILLED, "killed":true, "before":false, "notified":1, "after":true})
        );
    });
}

#[test]
fn finished_executions_release_their_context_signals() {
    run(async {
        let runner = loaded().await;
        let programs = Programs::new();
        let quiet = programs.emitting(b"", b"", 0);
        let mut releases = Vec::new();
        for _ in 0..3 {
            let signal = Cancellation::new();
            releases.push(super::abort_observers::signal_release(&signal));
            super::abort_observers::bind(&runner, &signal);
            let result = execute(
                &runner,
                json!({"command":quiet, "args":[], "options":{"cwd":""}, "signal":"relay"}),
            )
            .await;
            assert_eq!(
                result,
                json!({"stdout":"", "stderr":"", "code":0, "killed":false, "before":false, "notified":0, "after":false})
            );
        }
        super::abort_observers::bind(&runner, &Cancellation::new());
        super::abort_observers::signal_command(&runner, "barrier").await;
        assert_eq!(
            super::abort_observers::retained(&mut releases),
            0,
            "finished executions kept their context signals"
        );
    });
}

/// Runs a pending child with `options` that stays alive until its deadline ends it.
async fn held_with(runner: &ExtensionRunner<Local>, programs: &Programs, options: Value) -> String {
    pending(runner, programs, json!({"options":options}), Some).await
}

/// Runs a pending child with `options`, releasing the child if it reports READY.
async fn released_with(
    runner: &ExtensionRunner<Local>,
    programs: &Programs,
    options: Value,
) -> String {
    pending(runner, programs, json!({"options":options}), release).await
}

#[test]
fn exec_deadline_is_optional_and_milliseconds() {
    run(async {
        let (_owner, runner, events) = observed().await;
        let programs = Programs::new();
        let finished = json!({"stdout":"", "stderr":"", "code":0, "killed":false});
        let expired = json!({"stdout":"", "stderr":"", "code":KILLED, "killed":true});
        for options in [
            json!({"cwd":""}),
            json!({"cwd":"", "timeout":null}),
            json!({"cwd":"", "timeout":0}),
            json!({"cwd":"", "timeout":-0.0}),
            json!({"cwd":"", "timeout":-1}),
        ] {
            let result = released_with(&runner, &programs, options.clone()).await;
            assert_eq!(
                serde_json::from_str::<Value>(&result).unwrap(),
                finished,
                "{options}"
            );
        }
        let begun = tokio::time::Instant::now();
        let (held, socket) = started(
            &runner,
            &programs,
            &json!({"options":{"cwd":"", "timeout":2500}}),
        )
        .await;
        // The deadline starts between `begun` and READY; the clock stays at each side of it.
        let boundary = (begun, tokio::time::Instant::now());
        tokio::time::pause();
        let deadline = std::time::Duration::from_millis(2500);
        assert_running_until(&held, &socket, boundary.0 + deadline).await;
        assert_ended_by(socket, boundary.1 + deadline).await;
        assert_eq!(held.await.unwrap(), expired);
        tokio::time::resume();
        let fractional = held_with(&runner, &programs, json!({"cwd":"", "timeout":0.5})).await;
        assert_eq!(serde_json::from_str::<Value>(&fractional).unwrap(), expired);

        let huge = released_with(&runner, &programs, json!({"cwd":"", "timeout":1e300})).await;
        let tiny = held_with(&runner, &programs, json!({"cwd":"", "timeout":1e-10})).await;
        assert_eq!(
            (huge.as_str(), tiny.as_str()),
            (finished.to_string().as_str(), expired.to_string().as_str())
        );

        let (observations, _seen) = super::abort_observers::observe(&events);
        let signal = Cancellation::new();
        super::abort_observers::bind(&runner, &signal);
        super::abort_observers::signal_command(&runner, "subscribe control").await;
        let timed = pending(
            &runner,
            &programs,
            json!({"options":{"cwd":"", "timeout":0.5}, "signal":"relay"}),
            Some,
        )
        .await;
        assert_eq!(
            serde_json::from_str::<Value>(&timed).unwrap()["killed"],
            true
        );
        signal.abort();
        assert_eq!(super::abort_observers::labels(&observations), ["control"]);
    });
}

#[test]
#[cfg(unix)]
fn exec_spawn_failures_keep_result_channel() {
    run(async {
        let scratch = Scratch::new().unwrap();
        let denied = scratch.path().join("not executable");
        std::fs::write(&denied, b"bytes that are not a program\n").unwrap();
        let missing = scratch.path().join("missing");
        let programs = Programs::new();
        let quiet = programs.emitting(b"", b"", 0);
        let runner = loaded().await;
        let failed = json!({"stdout":"", "stderr":"", "code":1, "killed":false});
        for request in [
            json!({"command":missing, "args":[], "options":{"cwd":""}}),
            json!({"command":quiet, "args":[], "options":{"cwd":missing}}),
            json!({"command":denied, "args":[], "options":{"cwd":""}}),
        ] {
            assert_eq!(execute(&runner, request.clone()).await, failed, "{request}");
        }
        let preaborted = execute(
            &runner,
            json!({"command":missing, "args":[], "options":{"cwd":""}, "signal":"preaborted"}),
        )
        .await;
        assert_eq!(
            preaborted,
            json!({"stdout":"", "stderr":"", "code":1, "killed":true})
        );
        let unsuccessful = execute(
            &runner,
            json!({"command":programs.emitting(b"", b"", 3), "args":[], "options":{"cwd":""}}),
        )
        .await;
        assert_eq!(
            unsuccessful,
            json!({"stdout":"", "stderr":"", "code":3, "killed":false})
        );
    });
}

#[test]
fn exec_invalid_command_fails_before_signal_admission() {
    run(async {
        let (_owner, runner, events) = observed().await;
        let (observations, _seen) = super::abort_observers::observe(&events);
        let signal = Cancellation::new();
        super::abort_observers::bind(&runner, &signal);
        super::abort_observers::signal_command(&runner, "subscribe control").await;
        let programs = Programs::new();
        let quiet = programs.emitting(b"", b"", 0);
        let nul = std::process::Command::new("nul\0byte")
            .spawn()
            .unwrap_err()
            .to_string();
        for (command, args, expected) in [
            ("", vec![], "empty command"),
            ("nul\0byte", vec![], nul.as_str()),
            (quiet.as_str(), vec!["nul\0byte"], nul.as_str()),
        ] {
            let request =
                json!({"command":command, "args":args, "options":{"cwd":""}, "signal":"relay"});
            let context = runner.create_command_context();
            assert_eq!(outcome(&runner, context, request).await, expected);
        }
        signal.abort();
        assert_eq!(super::abort_observers::labels(&observations), ["control"]);
    });
}

#[test]
fn exec_settlement_removes_only_its_observer() {
    run(async {
        let (owner, runner, events) = observed().await;
        let programs = Programs::new();
        let (observations, _seen) = super::abort_observers::observe(&events);
        let missing = json!({"command":"missing-program-for-settlement", "args":[]});
        let empty = json!({"command":"", "args":[]});
        let cases: [(&str, Value, bool); 5] = [
            ("success", json!({}), false),
            ("spawn", missing, false),
            ("cancel", json!({}), true),
            (
                "timeout",
                json!({"options":{"cwd":"", "timeout":0.5}}),
                false,
            ),
            ("import", empty, false),
        ];
        let mut expected = Vec::new();
        for (label, mut fields, cancel) in cases {
            let signal = Cancellation::new();
            super::abort_observers::bind(&runner, &signal);
            let control = format!("control-{label}");
            super::abort_observers::signal_command(&runner, &format!("subscribe {control}")).await;
            fields["signal"] = "relay".into();
            fields["label"] = label.into();
            let hold = |socket| {
                signal.abort();
                Some(socket)
            };
            if cancel {
                pending(&runner, &programs, fields, hold).await;
            } else {
                pending(&runner, &programs, fields, release).await;
            }
            super::abort_observers::signal_command(&runner, "barrier").await;
            assert!(super::abort_observers::released(&owner, label), "{label}");
            assert_eq!(
                super::abort_observers::released(&owner, &control),
                cancel,
                "{label}: a notified listener runs once and is released"
            );
            signal.abort();
            expected.push(control);
            if cancel {
                expected.push("relay".to_owned());
            }
            assert_eq!(super::abort_observers::labels(&observations), expected);
        }
    });
}

#[test]
fn signal_aliases_and_overlaps_keep_other_calls_alive() {
    run(async {
        let (owner, runner, events) = observed().await;
        let programs = Programs::new();
        let (observations, _seen) = super::abort_observers::observe(&events);
        let released = |label| super::abort_observers::released(&owner, label);
        let relay = |label: &str| json!({"signal":"relay", "label":label});
        let finished = json!({"stdout":"", "stderr":"", "code":0, "killed":false, "before":false, "notified":0, "after":false});
        let shared = Cancellation::new();
        super::abort_observers::bind(&runner, &shared);
        let (older, older_socket) = started(&runner, &programs, &relay("older")).await;
        let (newer, _newer_socket) = started(&runner, &programs, &relay("newer")).await;
        release(older_socket);
        assert_eq!(older.await.unwrap(), finished);
        super::abort_observers::signal_command(&runner, "barrier").await;
        assert!(released("older") && !released("newer"));
        shared.abort();
        assert_eq!(super::abort_observers::labels(&observations), ["relay"]);
        assert_eq!(
            newer.await.unwrap(),
            json!({"stdout":"", "stderr":"", "code":KILLED, "killed":true, "before":false, "notified":1, "after":true})
        );
        super::abort_observers::signal_command(&runner, "barrier").await;
        assert!(released("newer"));

        let (first, second) = (Cancellation::new(), Cancellation::new());
        super::abort_observers::bind(&runner, &first);
        let (sibling, sibling_socket) = started(&runner, &programs, &relay("sibling")).await;
        super::abort_observers::bind(&runner, &second);
        let (cancelled, _cancelled_socket) = started(&runner, &programs, &relay("cancelled")).await;
        second.abort();
        assert_eq!(cancelled.await.unwrap()["killed"], true);
        assert!(!first.is_aborted());
        release(sibling_socket);
        assert_eq!(sibling.await.unwrap(), finished);
        super::abort_observers::signal_command(&runner, "barrier").await;
        assert!(released("sibling") && released("cancelled"));
        assert_eq!(
            super::abort_observers::labels(&observations),
            ["relay", "relay"]
        );
    });
}

#[test]
fn exec_utf8_pipe_fragments_remain_valid() {
    run(async {
        let runner = loaded().await;
        let programs = Programs::new();
        let data = "🙂".as_bytes();
        let mut chunks = Vec::new();
        for stream in ["stdout", "stderr"] {
            for split in 1..4 {
                chunks.push(json!({"stream":stream, "bytes":&data[..split]}));
                chunks.push(json!({"stream":stream, "bytes":&data[split..]}));
            }
            chunks.push(json!({"stream":stream, "bytes":&data[..2]}));
        }
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let writes = chunks.len();
        let program = programs.add(&json!({"scenario":"chunks", "chunks":chunks}));
        // The child asks before each write, so each fragment reaches its pipe on its own.
        let grants = tokio::task::spawn_local(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            for _ in 0..writes {
                let mut request = [0];
                socket.read_exact(&mut request).await.unwrap();
                assert_eq!(&request, b"R");
                socket.write_all(b"x").await.unwrap();
            }
        });
        let result = execute(
            &runner,
            json!({"command":program, "args":[port.to_string()], "options":{"cwd":""}}),
        )
        .await;
        grants.await.unwrap();
        let expected = format!("🙂🙂🙂{}", String::from_utf8_lossy(&"🙂".as_bytes()[..2]));
        assert_eq!(
            result,
            json!({"stdout":expected, "stderr":expected, "code":0, "killed":false})
        );
    });
}

#[test]
#[cfg(unix)]
fn exec_term_resistance_uses_existing_escalation() {
    run(async {
        let runner = loaded().await;
        let programs = Programs::new();
        let signal = Cancellation::new();
        super::abort_observers::bind(&runner, &signal);
        // The child writes its partial streams, reports READY and answers each termination
        // request with `T` on the socket instead of ending.
        let fields = json!({
            "command":programs.add(&json!({"scenario":"stubborn"})),
            "args":["partial-out", "partial-err", ""],
            "signal":"relay",
        });
        let (execution, mut socket) = started(&runner, &programs, &fields).await;
        tokio::time::pause();
        let aborted = tokio::time::Instant::now();
        signal.abort();
        let mut answer = [0; 1];
        // Keep virtual time still until the real child acknowledges TERM.
        loop {
            tokio::select! {
                biased;
                result = socket.read_exact(&mut answer) => {
                    result.unwrap();
                    break;
                }
                () = tokio::task::yield_now() => {}
            }
        }
        assert_eq!(&answer, b"T", "terminated, still running");
        // The clock stood still from the abort, so the escalation ends exactly five seconds later.
        let escalation = aborted + std::time::Duration::from_secs(5);
        assert_running_until(&execution, &socket, escalation).await;
        assert_ended_by(socket, escalation).await;
        let result = execution.await.unwrap();
        assert_eq!(
            result,
            json!({"stdout":"partial-out", "stderr":"partial-err", "code":KILLED, "killed":true, "before":false, "notified":1, "after":true})
        );
    });
}

#[test]
fn exec_failures_preserve_messages_and_resource_phase() {
    run(async {
        let scratch = Scratch::new().unwrap();
        let marker = scratch.path().join("started");
        let (_owner, runner, events) = observed().await;
        let (observations, _seen) = super::abort_observers::observe(&events);
        let signal = Cancellation::new();
        super::abort_observers::bind(&runner, &signal);
        super::abort_observers::signal_command(&runner, "subscribe control").await;
        let programs = Programs::new();
        let touch = programs.add(&json!({"scenario":"touch"}));
        let acquire =
            json!({"command":touch, "args":[marker], "options":{"cwd":""}, "signal":"relay"});

        let mut infinite = acquire.clone();
        infinite["infinite"] = true.into();
        let context = runner.create_command_context();
        assert_eq!(
            outcome(&runner, context, infinite).await,
            "extension wrote a non-finite number (Infinity or NaN)"
        );

        let (entered, mut admitted) = tokio::sync::mpsc::unbounded_channel();
        runner.bind_command_context(Some(
            maestro_extensions::ExtensionCommandContextActions::new(
                maestro_extensions::CommandContextOperations {
                    wait_for_idle: Rc::new(move || {
                        let (resume, done) = tokio::sync::oneshot::channel::<()>();
                        entered.send(resume).unwrap();
                        Box::pin(async move { Ok(done.await?) })
                    }),
                    ..super::context_actions::silent_command_operations()
                },
            ),
        ));
        let mut stale = acquire;
        stale["idle"] = true.into();
        let context = runner.create_command_context();
        let (rejected, ()) = futures_util::future::join(outcome(&runner, context, stale), async {
            let resume = admitted.recv().await.unwrap();
            runner.invalidate(None);
            resume.send(()).unwrap();
        })
        .await;
        assert_eq!(rejected, super::retained_contexts::STALE);
        assert!(!marker.exists(), "rejected before the process started");
        signal.abort();
        assert_eq!(super::abort_observers::labels(&observations), ["control"]);
    });
}

/// Registers the component's shell, whose commands run as the final operand of a child that
/// writes the partial streams `partial-out` and `partial-err` before it reports READY. The
/// child's program lives as long as the returned programs.
async fn register_shell(runner: &ExtensionRunner<Local>) -> Programs {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let programs = Programs::new();
    let pending = programs.add(&json!({"scenario":"pending"}));
    let shell = json!({"program":pending, "operands":["partial-out", "partial-err"]});
    let registered = command(
        format!("exec shell:{shell}"),
        runner.create_command_context(),
    )
    .await;
    assert_eq!(registered.unwrap_err().to_string(), "shell registered");
    programs
}

#[test]
fn returned_shell_abort_survives_event_handler_return() {
    run(async {
        let (owner, runner, _events) = observed().await;
        let _programs = register_shell(&runner).await;
        let mut event = maestro_extensions::UserBashEvent {
            command: "remote".into(),
            cwd: "/event".into(),
            exclude_from_context: false,
        };
        let returned = runner
            .emit_user_bash::<Rc<dyn maestro_extensions::BashOperations<Local>>, maestro_extensions::BashResult>(&mut event)
            .await
            .unwrap()
            .unwrap();
        let operations = returned.operations.unwrap();
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let script = port.to_string();
        let signal = Cancellation::new();
        let mut chunks = Vec::new();
        let mut sink = |bytes: &[u8]| chunks.push(bytes.to_vec());
        let (error, mut held) = {
            let execution = operations.exec(
                &script,
                "",
                maestro_extensions::BashExecOptions {
                    on_data: &mut sink,
                    signal: Some(&signal),
                    timeout: None,
                    env: None,
                },
            );
            tokio::pin!(execution);
            let held = tokio::select! {
                result = &mut execution => panic!("settled before READY: {:?}", result.map(|result| result.exit_code)),
                socket = ready(&listener) => socket,
            };
            signal.abort();
            (execution.await.unwrap_err().to_string(), held)
        };
        assert_eq!(error, "aborted");
        let read = held.read(&mut [0; 1]).await;
        assert!(closed(&read), "the child ended: {read:?}");
        assert_eq!(chunks, [b"partial-out".to_vec(), b"partial-err".to_vec()]);
        super::abort_observers::signal_command(&runner, "barrier").await;
        assert!(!super::abort_observers::released(&owner, "shell"));
        drop(operations);
        super::abort_observers::signal_command(&runner, "barrier").await;
        assert!(super::abort_observers::released(&owner, "shell"));
    });
}
