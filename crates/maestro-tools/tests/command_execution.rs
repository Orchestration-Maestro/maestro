//! Explicit command execution observed through its public operation.
#![cfg(test)]
#![cfg(unix)]
use crate::process_support;

use maestro_models::Cancellation;
use maestro_tools::{ExecOptions, ExecResult, exec_command};
use process_support::{complete, forward, poll_once, poll_through, polled_to_completion};
use std::future::Future;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// A single-threaded runtime with I/O and timers.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// A single-threaded runtime whose clock starts paused.
fn paused_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
}

/// Reports `READY`, then waits for one byte and prints `bye`.
const HOLDER: &str = r"
import os, socket, sys
s = socket.create_connection(('127.0.0.1', int(sys.argv[1])))
s.sendall(b'READY\n')
s.recv(1)
os.write(1, b'bye')
";

/// Reports `READY` and exits on any byte; each termination signal it receives prints
/// `bye`, is answered with `T` and is otherwise ignored.
const STUBBORN: &str = r"
import os, signal, socket, sys
s = socket.create_connection(('127.0.0.1', int(sys.argv[1])))
def term(*_): os.write(1, b'bye'); s.sendall(b'T')
signal.signal(signal.SIGTERM, term)
s.sendall(b'READY\n')
s.recv(1)
";

/// Leaves a descendant holding both pipes after the direct child exits; the
/// descendant answers each `p` with `E` and finishes on any other byte.
const DESCENDANT: &str = r"
import os, socket, sys
os.write(1, b'early'); os.write(2, b'diag')
if os.fork() == 0:
    s = socket.create_connection(('127.0.0.1', int(sys.argv[1])))
    s.sendall(b'READY\n')
    while s.recv(1) == b'p': s.sendall(b'E')
    os.write(1, b'late'); os._exit(0)
os._exit(0)
";

/// The temporary directory, as the positional working directory.
fn cwd() -> String {
    std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned()
}

/// Owned argument vector.
fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// Runs `command` with `values` and `options` in the temporary directory; the operands
/// are copied, so the future borrows only the options.
fn run<'a>(
    command: &str,
    values: &[&str],
    options: ExecOptions<'a>,
) -> impl Future<Output = ExecResult> + use<'a> {
    let (command, values) = (command.to_owned(), args(values));
    async move {
        exec_command(&command, &values, &cwd(), options)
            .await
            .unwrap()
    }
}

/// Runs `python3 -c script` with `values` appended.
fn python<'a>(
    script: &str,
    values: &[&str],
    options: ExecOptions<'a>,
) -> impl Future<Output = ExecResult> + use<'a> {
    let mut all = vec!["-c", script];
    all.extend_from_slice(values);
    run("python3", &all, options)
}

/// A held child's readiness socket.
struct Held {
    /// The connection the child waits on.
    socket: TcpStream,
}

/// Binds a readiness listener and returns it with its port.
async fn listen() -> (TcpListener, String) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    (listener, port)
}

/// Accepts the held child and reads its `READY` line.
async fn ready(listener: &TcpListener) -> Held {
    let (mut socket, _) = listener.accept().await.unwrap();
    let mut line = Vec::new();
    while !line.ends_with(b"\n") {
        line.push(socket.read_u8().await.unwrap());
    }
    assert_eq!(line, b"READY\n");
    Held { socket }
}

/// Lets the held child continue and returns what it sent until it closed the socket.
async fn release(mut held: Held) -> Vec<u8> {
    let _ = held.socket.write_all(b"g").await;
    let mut rest = Vec::new();
    let _ = held.socket.read_to_end(&mut rest).await;
    rest
}

#[test]
fn exec_preserves_literal_operands_and_positional_directory() {
    runtime().block_on(async {
        let script = "import os, sys\nassert sys.stdin.buffer.read() == b''\nsys.stdout.write(sys.argv[1] + '|' + os.getcwd() + '|' + os.environ['HOME'])";
        let result = python(script, &["a b*$HOME 'q'"], ExecOptions::default()).await;
        let home = std::env::var("HOME").unwrap();
        assert_eq!(result.stdout, format!("a b*$HOME 'q'|{}|{home}", cwd()));
        assert_eq!(
            (result.stderr.as_str(), result.code, result.killed),
            ("", 0, false)
        );
    });
}

#[test]
fn exec_separates_unbounded_unsanitized_streams() {
    runtime().block_on(async {
        let script = r"
import sys
sys.stdout.buffer.write(b'\x1b[31mred\x1b[0m\r\0' + b'o' * 300000)
sys.stderr.buffer.write(b'e' * 300000 + b'\r\n')
";
        let result = python(script, &[], ExecOptions::default()).await;
        assert_eq!(
            result.stdout,
            format!("\u{1b}[31mred\u{1b}[0m\r\0{}", "o".repeat(300_000))
        );
        assert_eq!(result.stderr, format!("{}\r\n", "e".repeat(300_000)));
        let empty = run("true", &[], ExecOptions::default()).await;
        assert_eq!((empty.stdout.as_str(), empty.stderr.as_str()), ("", ""));
    });
}

#[test]
fn exec_preserves_utf8_across_pipe_fragments() {
    runtime().block_on(async {
        let script = r"
import os, socket, sys
split = int(sys.argv[2])
for fd in (1, 2): os.write(fd, b'\xf0\x90\x8d\x88'[:split])
s = socket.create_connection(('127.0.0.1', int(sys.argv[1])))
s.sendall(b'READY\n')
s.recv(1)
for fd in (1, 2): os.write(fd, b'\xf0\x90\x8d\x88'[split:])
";
        for split in ["1", "2", "3"] {
            let (listener, port) = listen().await;
            let exec = python(script, &[&port, split], ExecOptions::default());
            let released = async { release(ready(&listener).await).await };
            let (result, _) = tokio::join!(exec, released);
            assert_eq!(
                (result.stdout.as_str(), result.stderr.as_str()),
                ("\u{10348}", "\u{10348}"),
                "split {split}"
            );
        }
        let malformed =
            r"import sys; sys.stdout.buffer.write(b'a\xffb'); sys.stderr.buffer.write(b'\xf0\x90')";
        let result = python(malformed, &[], ExecOptions::default()).await;
        assert_eq!(
            (result.stdout.as_str(), result.stderr.as_str()),
            ("a\u{fffd}b", "\u{fffd}")
        );
    });
}

#[test]
fn exec_preserves_status_and_maps_signal_exit_to_zero() {
    runtime().block_on(async {
        for (script, code) in [("exit 0", 0), ("exit 7", 7), ("kill -9 $$", 0)] {
            let result = run("sh", &["-c", script], ExecOptions::default()).await;
            assert_eq!((result.code, result.killed), (code, false), "{script}");
        }
    });
}

#[test]
fn exec_spawn_errors_preserve_phase_and_preabort_state() {
    runtime().block_on(async {
        let temp = cwd();
        let cases = [
            ("maestro-missing-executable", cwd()),
            ("true", "/maestro-missing-directory".to_owned()),
            (temp.as_str(), cwd()),
        ];
        for (command, dir) in cases {
            let result = exec_command(command, &[], &dir, ExecOptions::default())
                .await
                .unwrap();
            assert_eq!(
                (
                    result.stdout.as_str(),
                    result.stderr.as_str(),
                    result.code,
                    result.killed
                ),
                ("", "", 1, false),
                "{command} in {dir}"
            );
        }
        let aborted = Cancellation::new();
        aborted.abort();
        let options = ExecOptions {
            signal: Some(&aborted),
            timeout: None,
        };
        let result = run("maestro-missing-executable", &[], options).await;
        assert_eq!((result.code, result.killed), (1, true));
        let error = exec_command(
            "true",
            &args(&["bad\0operand"]),
            &cwd(),
            ExecOptions::default(),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    });
}

#[test]
fn exec_empty_cwd_runs_in_the_inherited_directory() {
    runtime().block_on(async {
        let result = exec_command(
            "python3",
            &args(&["-c", "import os; print(os.getcwd(), end='')"]),
            "",
            ExecOptions::default(),
        )
        .await
        .unwrap();
        let inherited = std::env::current_dir().unwrap();
        assert_eq!(
            (result.stdout.as_str(), result.code),
            (inherited.to_str().unwrap(), 0)
        );
    });
}

#[test]
fn exec_rejects_an_empty_command_before_observing_the_signal() {
    runtime().block_on(async {
        let aborted = Cancellation::new();
        aborted.abort();
        for signal in [None, Some(&aborted)] {
            let options = ExecOptions {
                signal,
                timeout: None,
            };
            let error = exec_command("", &[], &cwd(), options).await.err().unwrap();
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        }
    });
}

#[test]
fn exec_spawns_before_observing_preaborted_signal() {
    runtime().block_on(async {
        let aborted = Cancellation::new();
        aborted.abort();
        let options = || ExecOptions {
            signal: Some(&aborted),
            timeout: None,
        };
        let error = exec_command("true", &args(&["bad\0operand"]), &cwd(), options())
            .await
            .err()
            .unwrap();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        let result = run("sleep", &["60"], options()).await;
        assert_eq!(
            (result.code, result.killed),
            (0, true),
            "started, then terminated"
        );
        let result = run("true", &[], options()).await;
        assert!(
            result.killed,
            "a command that finishes at once still sees the abort"
        );
    });
}

/// Runs `exec` against a plain holder: waits for its readiness, releases it and
/// returns the result; fails when the execution ends first.
async fn run_released(timeout: Option<Duration>) -> ExecResult {
    let (listener, port) = listen().await;
    let exec = python(
        HOLDER,
        &[&port],
        ExecOptions {
            signal: None,
            timeout,
        },
    );
    tokio::pin!(exec);
    let held = tokio::select! {
        result = &mut exec => panic!("ended before the child was ready: {:?}", result.stdout),
        held = ready(&listener) => held,
    };
    tokio::join!(exec, release(held)).0
}

#[test]
fn exec_timeout_is_opt_in_and_released_after_completion() {
    runtime().block_on(async {
        for timeout in [None, Some(Duration::ZERO), Some(Duration::from_secs(600))] {
            let result = run_released(timeout).await;
            assert_eq!(
                (result.stdout.as_str(), result.killed),
                ("bye", false),
                "{timeout:?}"
            );
        }
        let options = ExecOptions {
            signal: None,
            timeout: Some(Duration::from_millis(50)),
        };
        let result = run("sleep", &["60"], options).await;
        assert_eq!(
            (result.code, result.killed),
            (0, true),
            "positive timeout terminates"
        );
    });
}

/// One way to request termination.
#[derive(Clone, Copy, Debug)]
enum Trigger {
    /// Abort the signal.
    Abort,
    /// Let the timeout elapse.
    Timeout,
}

#[test]
fn exec_terminates_on_abort_timeout_or_both() {
    paused_runtime().block_on(async {
        let timeout = Duration::from_secs(1);
        let cases: [&[Trigger]; 4] = [
            &[Trigger::Abort],
            &[Trigger::Timeout],
            &[Trigger::Abort, Trigger::Timeout],
            &[Trigger::Timeout, Trigger::Abort],
        ];
        for triggers in cases {
            let (listener, port) = blocking_listener();
            let signal = Cancellation::new();
            let options = ExecOptions {
                signal: Some(&signal),
                timeout: Some(timeout),
            };
            let exec = python(STUBBORN, &[&port], options);
            tokio::pin!(exec);
            assert!(
                poll_once(&mut exec).await.is_pending(),
                "spawned and waiting"
            );
            let mut socket = accept_ready(&listener);
            for trigger in triggers {
                match trigger {
                    Trigger::Abort => signal.abort(),
                    Trigger::Timeout => tokio::time::advance(timeout).await,
                }
                assert!(poll_once(&mut exec).await.is_pending(), "{triggers:?}");
            }
            read_signal_answer(&mut socket);
            std::io::Write::write_all(&mut socket, b"g").unwrap();
            let result = exec.await;
            assert_eq!(
                (result.stdout.as_str(), result.killed),
                ("bye", true),
                "{triggers:?}"
            );
        }
    });
}

/// A blocking readiness listener and its port.
fn blocking_listener() -> (std::net::TcpListener, String) {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    (listener, port)
}

/// Accepts the child and reads its `READY` line from a blocking socket.
fn accept_ready(listener: &std::net::TcpListener) -> std::net::TcpStream {
    let (mut socket, _) = listener.accept().unwrap();
    let mut line = [0; 6];
    std::io::Read::read_exact(&mut socket, &mut line).unwrap();
    assert_eq!(&line, b"READY\n");
    socket
}

/// Blocks until a [`STUBBORN`] child answers a termination signal it received.
fn read_signal_answer(socket: &mut std::net::TcpStream) {
    let mut answer = [0; 1];
    std::io::Read::read_exact(socket, &mut answer).unwrap();
    assert_eq!(&answer, b"T");
}

#[test]
fn exec_kills_and_reaps_a_term_resistant_child_after_the_escalation_delay() {
    paused_runtime().block_on(async {
        let (listener, port) = blocking_listener();
        let signal = Cancellation::new();
        let options = ExecOptions {
            signal: Some(&signal),
            timeout: None,
        };
        let exec = python(STUBBORN, &[&port], options);
        tokio::pin!(exec);
        assert!(
            poll_once(&mut exec).await.is_pending(),
            "spawned and waiting"
        );
        let mut socket = accept_ready(&listener);
        signal.abort();
        let aborted = tokio::time::Instant::now();
        assert!(
            poll_once(&mut exec).await.is_pending(),
            "terminated, not yet killed"
        );
        read_signal_answer(&mut socket);
        let result = exec.await;
        assert_eq!((result.code, result.killed), (0, true));
        assert_eq!(aborted.elapsed(), Duration::from_secs(5));
        assert_eq!(
            std::io::Read::read(&mut socket, &mut [0; 1]).unwrap(),
            0,
            "killed: the child's end closed"
        );
    });
}

#[test]
fn exec_finishes_after_inherited_pipe_grace() {
    runtime().block_on(async {
        let (listener, port) = listen().await;
        let exec = python(DESCENDANT, &[&port], ExecOptions::default());
        let (result, held) = tokio::join!(exec, ready(&listener));
        assert_eq!(
            (
                result.stdout.as_str(),
                result.stderr.as_str(),
                result.code,
                result.killed
            ),
            ("early", "diag", 0, false)
        );
        let mut held = held;
        held.socket.write_all(b"p").await.unwrap();
        assert_eq!(
            held.socket.read_u8().await.unwrap(),
            b'E',
            "the descendant was still holding the pipes"
        );
        release(held).await;
    });
}

#[test]
fn exec_settlement_releases_cancellation_observation() {
    runtime().block_on(async {
        let signal = Cancellation::new();
        let options = ExecOptions {
            signal: Some(&signal),
            timeout: Some(Duration::from_secs(600)),
        };
        let (result, forward) =
            polled_to_completion(exec_command("true", &[], &cwd(), options)).await;
        assert!(!result.unwrap().killed);
        assert_eq!(Arc::strong_count(&forward), 1);
        signal.abort();
        assert_eq!(Arc::strong_count(&forward), 1);

        let options = ExecOptions {
            signal: Some(&signal),
            timeout: None,
        };
        let (result, forward) =
            polled_to_completion(exec_command("sleep", &args(&["60"]), &cwd(), options)).await;
        assert!(result.unwrap().killed, "already aborted: terminated");
        assert_eq!(Arc::strong_count(&forward), 1);
    });
}

/// Reports `READY` with the default termination action restored and answers each `p`
/// with `E`.
const YIELDING: &str = r"
import signal, socket, sys
signal.signal(signal.SIGTERM, signal.SIG_DFL)
s = socket.create_connection(('127.0.0.1', int(sys.argv[1])))
s.sendall(b'READY\n')
while s.recv(1) == b'p': s.sendall(b'E')
";

/// Asks a [`YIELDING`] child to answer: `true` when it answers `E`, `false` when it closed
/// its end. A closed end fails the question as a broken pipe or reset, or ends the answer
/// as end of file or a reset (a child that dies with the question unread resets the
/// connection); any other byte or I/O error fails the test.
///
/// A termination signal sent before the question is fatal before the child can
/// answer it.
fn answers(socket: &mut std::net::TcpStream) -> bool {
    use std::io::{ErrorKind, Read, Write};
    let closed = |error: &std::io::Error| {
        matches!(
            error.kind(),
            ErrorKind::ConnectionReset | ErrorKind::BrokenPipe
        )
    };
    if let Err(error) = socket.write_all(b"p") {
        assert!(closed(&error), "asking failed: {error}");
        return false;
    }
    let mut answer = [0; 1];
    match socket.read(&mut answer) {
        Ok(0) => false,
        Ok(_) => {
            assert_eq!(&answer, b"E", "unexpected answer");
            true
        }
        Err(error) => {
            assert_eq!(error.kind(), ErrorKind::ConnectionReset, "{error}");
            false
        }
    }
}

#[test]
fn exec_timeout_requests_termination_exactly_at_the_deadline() {
    paused_runtime().block_on(async {
        let timeout = Duration::from_secs(1);
        let (listener, port) = blocking_listener();
        let options = ExecOptions {
            signal: None,
            timeout: Some(timeout),
        };
        let exec = python(YIELDING, &[&port], options);
        tokio::pin!(exec);
        assert!(poll_once(&mut exec).await.is_pending(), "spawned");
        let mut socket = accept_ready(&listener);
        tokio::time::advance(timeout.checked_sub(Duration::from_millis(1)).unwrap()).await;
        assert!(poll_once(&mut exec).await.is_pending());
        assert!(answers(&mut socket), "alive 1 ms before the deadline");
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(poll_once(&mut exec).await.is_pending());
        assert!(!answers(&mut socket), "terminated at the deadline");
        let result = exec.await;
        assert_eq!((result.code, result.killed), (0, true));
    });
}

#[test]
fn exec_registers_a_cancellation_observer_until_it_settles() {
    runtime().block_on(async {
        let (listener, port) = listen().await;
        let signal = Cancellation::new();
        let options = ExecOptions {
            signal: Some(&signal),
            timeout: Some(Duration::from_secs(600)),
        };
        let owner = forward();
        let mut exec = Box::pin(python(HOLDER, &[&port], options));
        let pending = std::future::poll_fn(|cx| {
            Poll::Ready(poll_through(&owner, &mut exec, cx).is_pending())
        });
        assert!(pending.await);
        assert!(
            Arc::strong_count(&owner) > 1,
            "the running execution holds a waker"
        );
        let held = ready(&listener).await;
        let (result, _) = tokio::join!(complete(&owner, exec), release(held));
        assert_eq!(result.stdout, "bye");
        assert_eq!(Arc::strong_count(&owner), 1);
    });
}
