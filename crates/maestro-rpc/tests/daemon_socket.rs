//! The workspace daemon's API socket served in this process by `run_server`, and the daemon
//! request adapter under one connection.
#![cfg(target_os = "linux")]
use maestro_app::daemon::api::client::{ConnectionTarget, DaemonClient};
use maestro_app::daemon::api::{RequestMessage, RuntimeStatus, event_hub::EventHub};
use maestro_app::daemon::app::App;
use maestro_app::daemon::app::construction::{
    ManifestCache, PaneLaunchInputs, ProcessDetectionMode, PtySetupEntry,
};
use maestro_app::daemon::paths::DaemonPaths;
use maestro_app::daemon::render_signal::RenderSignal;
use maestro_app::daemon::settings::model::Config;
use maestro_app::daemon::terminal_theme::TerminalTheme;
use maestro_app::rpc_mode::{
    DaemonWorkspaceOperations, JsonlLineReader, RpcOutput, WorkspaceOperations,
};
use maestro_app::rpc_types::workspace::RpcResponse;
use maestro_app::rpc_types::workspace_schema::{ErrorBody, ResponseResult, ServerCapabilities};
use maestro_app::{RpcConnectionOutcome, RpcModeOptions, run_rpc_connection};
use maestro_rpc::daemon::run_server;
use maestro_test_directory::Scratch;
use maestro_test_process::TestProcess;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::{fs::FileTypeExt, fs::PermissionsExt, net::UnixStream};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::{Arc, RwLock};
use std::task::{Context, Waker};
use std::{future::Future, pin::pin, rc::Rc, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncBufReader, DuplexStream};
use tokio::sync::{Notify, mpsc, oneshot, watch};
use tokio::task::{JoinHandle, LocalSet};

/// The shared protocol version a ping reports.
const PROTOCOL: u32 = 21;

/// The largest request line, in raw bytes before its LF.
const LIMIT: usize = 1024 * 1024;

/// The variable carrying a logged child's root.
const ROOT: &str = "MAESTRO_TEST_DAEMON_ROOT";

/// A daemon without workspaces whose panes would advertise `socket`.
fn daemon(socket: &Path) -> App<fn() -> u128> {
    let (events, receiver) = mpsc::channel(8);
    let launch = PaneLaunchInputs {
        no_session: true,
        setup_entry: PtySetupEntry {
            executable: std::env::current_exe().unwrap(),
            args: Vec::new(),
        },
        socket: socket.to_path_buf(),
        executable: None,
        manifests: Arc::new(RwLock::new(ManifestCache::new(None, None).unwrap())),
        events,
        render_notify: Arc::new(Notify::new()),
        render_dirty: Arc::new(RenderSignal::new()),
        theme: TerminalTheme::default(),
        appearance: None,
        process_detection_mode: ProcessDetectionMode::Native,
    };
    let clock = (|| 0) as fn() -> u128;
    App::new(
        &Config::default(),
        clock,
        launch,
        receiver,
        EventHub::default(),
    )
}

/// Runs `body` on a task set of a current-thread runtime, its clock paused when `paused`.
fn local<T>(paused: bool, body: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(paused)
        .build()
        .unwrap()
        .block_on(LocalSet::new().run_until(body))
}

/// Serves the daemon of `paths` through `run_server` while `client` runs on a blocking
/// thread with the API socket's path; the daemon stops once the client returns.
fn served<T: Send + 'static>(
    paths: &DaemonPaths,
    client: impl FnOnce(PathBuf) -> T + Send + 'static,
) -> T {
    let socket = PathBuf::from(paths.api_socket_path());
    local(false, async {
        let mut app = daemon(&socket);
        let (stop, stopped) = oneshot::channel::<()>();
        let mut server = pin!(run_server(&mut app, paths, async {
            drop(stopped.await);
        }));
        let polled = server
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()));
        assert!(polled.is_pending(), "the server ended at once: {polled:?}");
        let client = tokio::task::spawn_blocking(move || client(socket));
        let value = tokio::select! {
            ended = &mut server => panic!("the server ended early: {ended:?}"),
            value = client => value.unwrap(),
        };
        drop(stop);
        server.await.unwrap();
        value
    })
}

/// Runs a second daemon of `paths` that stops at once, from a thread of its own.
fn start(paths: &DaemonPaths) -> io::Result<()> {
    let socket = PathBuf::from(paths.api_socket_path());
    local(false, async {
        run_server(&mut daemon(&socket), paths, std::future::ready(())).await
    })
}

/// Writes `request` on a fresh connection to `socket`, then returns everything the daemon
/// writes until it closes the connection; `half_close` ends the input after the request.
fn exchange(socket: &Path, request: &[u8], half_close: bool) -> String {
    let mut stream = UnixStream::connect(socket).unwrap();
    // A refused oversized line may close the connection before the whole line is written.
    drop(stream.write_all(request));
    if half_close {
        stream.shutdown(std::net::Shutdown::Write).unwrap();
    }
    let mut reply = String::new();
    drop(stream.read_to_string(&mut reply));
    reply
}

/// Writes `request` on a fresh connection to `socket` and reads its first reply line.
fn first_reply(socket: &Path, request: &[u8]) -> String {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream.write_all(request).unwrap();
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).unwrap();
    line
}

/// The typed response of `id` for `command`.
fn response(id: &str, command: &str, result: Result<ResponseResult, ErrorBody>) -> RpcResponse {
    RpcResponse {
        id: Some(id.into()),
        command: command.into(),
        result,
    }
}

/// The failure of a request the daemon cannot take.
fn unavailable(message: &str) -> Result<ResponseResult, ErrorBody> {
    Err(ErrorBody {
        code: "server_unavailable".into(),
        message: message.into(),
    })
}

/// The answer to a ping reporting `capabilities`.
fn pong(capabilities: Option<ServerCapabilities>) -> ResponseResult {
    ResponseResult::Pong {
        version: env!("CARGO_PKG_VERSION").into(),
        protocol: PROTOCOL,
        capabilities,
    }
}

/// One workspace-only connection served by [`DaemonWorkspaceOperations`], with the test's
/// ends of its streams, the daemon queue and the stop.
struct Adapter {
    /// The connection's input.
    input: DuplexStream,
    /// The connection's output.
    output: AsyncBufReader<DuplexStream>,
    /// The daemon queue.
    requests: mpsc::UnboundedReceiver<RequestMessage>,
    /// Whether a stop was requested.
    stopped: watch::Receiver<bool>,
    /// The connection runner.
    runner: JoinHandle<io::Result<RpcConnectionOutcome>>,
}

impl Adapter {
    /// A connection reporting `capabilities`, whose output holds `capacity` unread bytes.
    fn new(capabilities: Option<ServerCapabilities>, capacity: usize) -> Self {
        let (sender, requests) = mpsc::unbounded_channel();
        let (stop, stopped) = watch::channel(false);
        let operations = DaemonWorkspaceOperations::new(sender, capabilities, stop);
        let (input, server_input) = tokio::io::duplex(4096);
        let (server_output, output) = tokio::io::duplex(capacity);
        let options = RpcModeOptions {
            workspace: Some(Rc::new(operations) as Rc<dyn WorkspaceOperations>),
            session: None,
        };
        let reader = Box::new(JsonlLineReader::new(server_input));
        let output_stream = RpcOutput::new(server_output, None);
        let runner = tokio::task::spawn_local(run_rpc_connection(reader, output_stream, options));
        Self {
            input,
            output: AsyncBufReader::new(output),
            requests,
            stopped,
            runner,
        }
    }

    /// Sends the command of `id` with `command` as its type and `members` as its other text.
    async fn send(&mut self, id: &str, command: &str, members: &str) {
        let line = format!("{{\"id\":\"{id}\",\"type\":\"{command}\"{members}}}\n");
        self.input.write_all(line.as_bytes()).await.unwrap();
    }

    /// The next response.
    async fn reply(&mut self) -> RpcResponse {
        let mut line = String::new();
        self.output.read_line(&mut line).await.unwrap();
        serde_json::from_str(&line).unwrap()
    }
}

/// Where a logged test body runs: a child with its root, or the parent with the root and
/// the child's output.
enum Logged {
    /// The child, running the body with file logging at debug level.
    Child(PathBuf),
    /// The parent, after the child passed.
    Parent(Scratch, Output),
}

/// Runs the body of `test` in a child process of its own, so the child's file logging
/// owns that process, with `MAESTRO_LOG` at debug level and a root the parent keeps.
fn logged(test: &str) -> Logged {
    if maestro_test_process::selection().is_child() {
        return Logged::Child(PathBuf::from(std::env::var_os(ROOT).unwrap()));
    }
    let root = Scratch::new().unwrap();
    let output = TestProcess::current(test)
        .unwrap()
        .case("*")
        .env(ROOT, root.path())
        .env("MAESTRO_LOG", "maestro=debug")
        .nocapture()
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "the child failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Logged::Parent(root, output)
}

/// The default daemon's paths under `root`.
fn root_paths(root: &Path) -> DaemonPaths {
    DaemonPaths::new(root.to_str().unwrap(), None).unwrap()
}

/// The index of the first log line holding every one of `parts`.
fn logged_line(log: &str, parts: &[&str]) -> usize {
    log.lines()
        .position(|line| parts.iter().all(|part| line.contains(part)))
        .unwrap_or_else(|| panic!("no log line holds {parts:?}:\n{log}"))
}

/// A failure with `code` and `message`.
fn failure(code: &str, message: &str) -> Result<ResponseResult, ErrorBody> {
    Err(ErrorBody {
        code: code.into(),
        message: message.into(),
    })
}

/// Answers one `workspace.list` per entry of `results` through the daemon queue, then one
/// whose response cannot be written.
async fn answer_each(results: Vec<(&str, Result<ResponseResult, ErrorBody>)>) {
    let mut adapter = Adapter::new(None, 4096);
    for (id, result) in results {
        adapter.send(id, "workspace.list", "").await;
        let request = adapter.requests.recv().await.unwrap();
        request.respond_to.send(result.clone()).unwrap();
        let answered = response(id, "workspace.list", result);
        assert_eq!(adapter.reply().await, answered);
    }
    adapter.send("unwritten", "workspace.list", "").await;
    let request = adapter.requests.recv().await.unwrap();
    drop(adapter.output);
    request.respond_to.send(Ok(ResponseResult::Ok {})).unwrap();
    request.response_write_complete.await.unwrap();
    let failure = adapter.runner.await.unwrap().unwrap_err();
    assert_eq!(failure.kind(), io::ErrorKind::BrokenPipe);
}

/// Checks that a log line holding every one of `first` comes before one holding `then`.
fn logged_in_order(log: &str, first: &[&str], then: &[&str]) {
    let order = (logged_line(log, first), logged_line(log, then));
    assert!(
        order.0 < order.1,
        "{first:?} is not logged before {then:?}:\n{log}"
    );
}

/// A request's log classifies its result by the response's success and code alone: data
/// that looks like a failure is still ok, the `timeout` code is a timeout, any other code is
/// an error, and a response whose write fails logs that failure.
#[test]
fn response_outcome_uses_the_shared_success_and_code_fields() {
    let Logged::Child(root) = logged("response_outcome_uses_the_shared_success_and_code_fields")
    else {
        return;
    };
    let paths = root_paths(&root);
    maestro_app::daemon::logging::init_file_logging(&paths, "daemon", Some("maestro=debug"));
    let looks_failed = Ok(ResponseResult::Pong {
        version: r#"{"error":{"code":"timeout","message":"failed"}}"#.into(),
        protocol: PROTOCOL,
        capabilities: None,
    });
    local(
        false,
        answer_each(vec![
            ("looks-failed", looks_failed),
            ("late", failure("timeout", "slow")),
            ("refused", failure("busy", "timeout")),
        ]),
    );
    let log = std::fs::read_to_string(paths.log_path("daemon")).unwrap();
    for (id, outcome) in [
        ("looks-failed", "ok"),
        ("late", "timeout"),
        ("refused", "error"),
    ] {
        let request = format!("request_id=\"{id}\"");
        let outcome = format!("outcome=\"{outcome}\"");
        logged_line(&log, &["api request completed", &request, &outcome]);
    }
    let failed = [
        "api request failed",
        "request_id=\"unwritten\"",
        "method=\"workspace.list\"",
    ];
    logged_line(&log, &failed);
}

/// A ping answers with its correlation, the product and protocol versions and both
/// capability flags, whether the daemon queue is busy or gone, and never enters it.
#[test]
fn ping_answers_without_entering_the_daemon_queue() {
    local(false, async {
        let capabilities = ServerCapabilities {
            live_handoff: true,
            detached_server_daemon: false,
        };
        for closed in [false, true] {
            let mut adapter = Adapter::new(Some(capabilities.clone()), 4096);
            if closed {
                adapter.requests.close();
            }
            adapter.send("p", "ping", "").await;
            let expected = response("p", "ping", Ok(pong(Some(capabilities.clone()))));
            assert_eq!(adapter.reply().await, expected);
            assert!(adapter.requests.try_recv().is_err(), "the ping was queued");
        }
    });
}

/// A stop answers while the daemon is busy with another request, wakes the server's
/// shutdown and refuses every later request but a ping.
#[test]
fn stop_bypasses_busy_dispatch_and_rejects_later_mutations() {
    local(false, async {
        let mut adapter = Adapter::new(None, 4096);
        adapter.send("busy", "workspace.list", "").await;
        let busy = adapter.requests.recv().await.unwrap();
        adapter.send("stop", "server.stop", "").await;
        let stopped = response("stop", "server.stop", Ok(ResponseResult::Ok {}));
        assert_eq!(adapter.reply().await, stopped);
        adapter.stopped.wait_for(|stopped| *stopped).await.unwrap();
        let rename = r#","workspace_id":"w","label":"l""#;
        adapter.send("late", "workspace.rename", rename).await;
        let refused = unavailable("server is shutting down");
        assert_eq!(
            adapter.reply().await,
            response("late", "workspace.rename", refused)
        );
        adapter.send("p", "ping", "").await;
        assert_eq!(adapter.reply().await, response("p", "ping", Ok(pong(None))));
        assert!(
            adapter.requests.try_recv().is_err(),
            "a later request was queued"
        );
        drop(busy);
    });
}

/// The daemon learns that a response's write was attempted only once that response's whole
/// frame was written and flushed, never at dispatch or admission; a failed attempt also
/// counts as attempted.
#[test]
fn dispatch_completion_follows_the_physical_response_write() {
    local(false, async {
        let mut adapter = Adapter::new(None, 16);
        adapter.send("a", "workspace.list", "").await;
        adapter.send("b", "tab.list", "").await;
        let first = adapter.requests.recv().await.unwrap();
        let second = adapter.requests.recv().await.unwrap();
        let (mut first_written, mut second_written) = (
            first.response_write_complete,
            second.response_write_complete,
        );
        first.respond_to.send(Ok(ResponseResult::Ok {})).unwrap();
        let mut start = [0; 1];
        tokio::io::AsyncReadExt::read_exact(&mut adapter.output, &mut start)
            .await
            .unwrap();
        assert_eq!(
            first_written.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        );
        assert_eq!(
            second_written.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        );
        let mut rest = String::new();
        adapter.output.read_line(&mut rest).await.unwrap();
        let line = format!("{}{rest}", char::from(start[0]));
        let written: RpcResponse = serde_json::from_str(&line).unwrap();
        assert_eq!(
            written,
            response("a", "workspace.list", Ok(ResponseResult::Ok {}))
        );
        first_written.await.unwrap();
        assert_eq!(
            second_written.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        );
        drop(adapter.output);
        second.respond_to.send(Ok(ResponseResult::Ok {})).unwrap();
        second_written.await.unwrap();
        let failure = adapter.runner.await.unwrap().unwrap_err();
        assert_eq!(failure.kind(), io::ErrorKind::BrokenPipe);
    });
}

/// An ordinary request may wait on the daemon longer than 5 s while other commands answer;
/// a closed queue or a dropped answer fails with its own diagnostic, and the input's end
/// drops the connection's wait without taking back a request the daemon already holds.
#[test]
fn ordinary_dispatch_has_no_five_second_deadline() {
    local(true, async {
        let mut adapter = Adapter::new(None, 4096);
        adapter.send("held", "workspace.list", "").await;
        let held = adapter.requests.recv().await.unwrap();
        let start = tokio::time::Instant::now();
        tokio::time::sleep(Duration::from_secs(6)).await;
        adapter.send("p", "ping", "").await;
        assert_eq!(adapter.reply().await, response("p", "ping", Ok(pong(None))));
        held.respond_to.send(Ok(ResponseResult::Ok {})).unwrap();
        let answered = response("held", "workspace.list", Ok(ResponseResult::Ok {}));
        assert_eq!(adapter.reply().await, answered);
        assert!(start.elapsed() > Duration::from_secs(5));

        adapter.send("dropped", "tab.list", "").await;
        drop(adapter.requests.recv().await.unwrap());
        let dropped = unavailable("request handling failed: receiving on a closed channel");
        assert_eq!(
            adapter.reply().await,
            response("dropped", "tab.list", dropped)
        );
        adapter.requests.close();
        adapter.send("closed", "tab.list", "").await;
        let closed = unavailable("failed to dispatch request: channel closed");
        assert_eq!(
            adapter.reply().await,
            response("closed", "tab.list", closed)
        );

        let mut adapter = Adapter::new(None, 4096);
        adapter.send("committed", "workspace.list", "").await;
        let committed = adapter.requests.recv().await.unwrap();
        drop(adapter.input);
        assert_eq!(
            adapter.runner.await.unwrap().unwrap(),
            RpcConnectionOutcome::Eof
        );
        assert!(
            committed.respond_to.is_closed(),
            "the connection stopped waiting"
        );
        assert!(matches!(
            committed.command,
            maestro_app::rpc_types::workspace::RpcCommand::WorkspaceList(_)
        ));
    });
}

/// Through `run_server`, a ping's first reply line reports the product version, the shared
/// protocol and the daemon's capabilities, with no startup record before it; the client's
/// status reads the same.
#[test]
fn socket_ping_reports_product_and_shared_protocol_versions() {
    let root = Scratch::new().unwrap();
    let paths = root_paths(root.path());
    let (line, status) = served(&paths, |socket| {
        let line = first_reply(&socket, b"{\"id\":\"p\",\"type\":\"ping\"}\n");
        let client = DaemonClient::for_target(ConnectionTarget::SocketPath(socket));
        (line, client.status().unwrap())
    });
    let capabilities = ServerCapabilities {
        live_handoff: false,
        detached_server_daemon:
            maestro_app::daemon::platform::current_process_is_detached_server_daemon(),
    };
    let reply: RpcResponse = serde_json::from_str(&line).unwrap();
    assert_eq!(
        reply,
        response("p", "ping", Ok(pong(Some(capabilities.clone()))))
    );
    let expected = RuntimeStatus {
        version: Some(env!("CARGO_PKG_VERSION").into()),
        protocol: Some(PROTOCOL),
        capabilities: Some(capabilities),
    };
    assert_eq!(status, expected);
}

/// A connection's first line decides it: an unterminated or empty input, a blank line,
/// invalid UTF-8 and a line over 1 MiB close it without a reply; CRLF, padding up to the
/// limit and U+0085 at the edges still answer, and U+FEFF reaches the parser.
#[test]
fn socket_first_lines_close_or_answer_by_their_framing() {
    let root = Scratch::new().unwrap();
    let paths = root_paths(root.path());
    let ping = r#"{"id":"p","type":"ping"}"#;
    let padded = move |extra: usize, end: &str| {
        let padding = " ".repeat(LIMIT + extra - ping.len() - end.len());
        format!("{ping}{padding}{end}\n").into_bytes()
    };
    let replies = served(&paths, move |socket| {
        let cases: Vec<(Vec<u8>, bool)> = vec![
            (b"{\"id\":".to_vec(), true),
            (Vec::new(), true),
            (b" \t \n".to_vec(), false),
            (b"\xff\n".to_vec(), false),
            (padded(1, ""), false),
            (padded(1, "\r"), false),
            (format!("{ping}\r\n").into_bytes(), true),
            (padded(0, ""), true),
            (padded(0, "\r"), true),
            (format!("\u{85}{ping}\u{85}\n").into_bytes(), true),
            (format!("\u{feff}{ping}\n").into_bytes(), true),
        ];
        cases
            .into_iter()
            .map(|(request, half_close)| exchange(&socket, &request, half_close))
            .collect::<Vec<_>>()
    });
    let served_capabilities = ServerCapabilities {
        live_handoff: false,
        detached_server_daemon:
            maestro_app::daemon::platform::current_process_is_detached_server_daemon(),
    };
    let answered = response("p", "ping", Ok(pong(Some(served_capabilities))));
    let answered = format!("{}\n", serde_json::to_string(&answered).unwrap());
    let unparsed = concat!(
        r#"{"type":"response","command":"parse","success":false,"#,
        r#""error":"Failed to parse command: expected value at line 1 column 1"}"#,
        "\n"
    );
    let expected = [
        "", "", "", "", "", "", &answered, &answered, &answered, &answered, unparsed,
    ];
    assert_eq!(replies, expected);
}

/// The listener's socket is private to its owner and removed when the daemon stops, unless
/// another file replaced it meanwhile; a live daemon makes a second start fail as busy, a
/// stale socket is reclaimed and a file that is not a socket is kept.
#[test]
fn socket_shutdown_keeps_replacement_files() {
    let root = Scratch::new().unwrap();
    let paths = root_paths(root.path());
    let socket = PathBuf::from(paths.api_socket_path());
    let second = paths.clone();
    let (mode, socket_file, busy) = served(&paths, move |socket| {
        let metadata = std::fs::metadata(&socket).unwrap();
        let busy = start(&second).unwrap_err();
        (
            metadata.permissions().mode() & 0o777,
            metadata.file_type().is_socket(),
            busy,
        )
    });
    assert_eq!((mode, socket_file), (0o600, true));
    assert_eq!(busy.kind(), io::ErrorKind::AddrInUse);
    let busy_text = format!(
        "maestro is already running (socket busy at {})",
        socket.display()
    );
    assert_eq!(busy.to_string(), busy_text);
    assert!(!socket.exists(), "the daemon removed its socket");

    served(&paths, |socket| {
        std::fs::remove_file(&socket).unwrap();
        std::fs::write(&socket, "replacement").unwrap();
    });
    assert_eq!(std::fs::read_to_string(&socket).unwrap(), "replacement");
    let refused = start(&paths).unwrap_err();
    assert_eq!(refused.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read_to_string(&socket).unwrap(), "replacement");

    std::fs::remove_file(&socket).unwrap();
    drop(std::os::unix::net::UnixListener::bind(&socket).unwrap());
    let reply = served(&paths, |socket| {
        first_reply(&socket, b"{\"id\":\"p\",\"type\":\"ping\"}\n")
    });
    assert!(reply.contains("\"success\":true"), "{reply}");
    assert!(!socket.exists());
}

/// The server's log records its listener, each request's start and completion with its
/// correlation, method, outcome and interface impact, a failed connection's cause and a
/// failed socket removal.
#[test]
fn socket_lifecycle_logs_keep_typed_outcomes() {
    let Logged::Child(root) = logged("socket_lifecycle_logs_keep_typed_outcomes") else {
        return;
    };
    let paths = root_paths(&root);
    let socket = PathBuf::from(paths.api_socket_path());
    let restore = served(&paths, move |socket| {
        first_reply(&socket, b"{\"id\":\"p\",\"type\":\"ping\"}\n");
        let rename = br#"{"id":"r","type":"workspace.rename","workspace_id":"w","label":"l"}"#;
        first_reply(&socket, &[rename.as_slice(), b"\n"].concat());
        exchange(&socket, &vec![b'a'; LIMIT + 1], false);
        maestro_test_directory::deny(&root, 0o500).unwrap()
    });
    let removal_refused = restore.is_some();
    drop(restore);
    let log = std::fs::read_to_string(paths.log_path("daemon")).unwrap();
    let path = format!("path={}", socket.display());
    logged_line(&log, &["api server listening", &path]);
    let started = format!("api_socket={}", socket.display());
    logged_line(&log, &["maestro server started", &started]);
    let ping = ["request_id=\"p\"", "method=\"ping\""];
    logged_in_order(
        &log,
        &["api request received", ping[0], ping[1], "changes_ui=false"],
        &["api request completed", ping[0], ping[1], "outcome=\"ok\""],
    );
    let rename = ["request_id=\"r\"", "method=\"workspace.rename\""];
    logged_in_order(
        &log,
        &[
            "INFO",
            "api request received",
            rename[0],
            rename[1],
            "changes_ui=true",
        ],
        &[
            "INFO",
            "api request completed",
            rename[0],
            rename[1],
            "outcome=\"error\"",
        ],
    );
    let oversized = [
        "WARN",
        "api connection failed",
        "api request line is too large",
    ];
    logged_line(&log, &oversized);
    if removal_refused {
        logged_line(
            &log,
            &["WARN", "failed to remove api socket on shutdown", &path],
        );
    }
}

/// A started server announces its ready line, its bound API socket and its log, and logs
/// its start once; a second start against a live daemon announces only that the server is
/// running and fails as busy. Nothing else is written beside the log.
#[test]
fn server_startup_announces_only_bound_resources() {
    match logged("server_startup_announces_only_bound_resources") {
        Logged::Child(root) => {
            let paths = root_paths(&root);
            let second = paths.clone();
            let busy = served(&paths, move |socket| {
                let busy = start(&second).unwrap_err();
                first_reply(&socket, b"{\"id\":\"p\",\"type\":\"ping\"}\n");
                busy.kind()
            });
            assert_eq!(busy, io::ErrorKind::AddrInUse);
            let log = std::fs::read_to_string(paths.log_path("daemon")).unwrap();
            assert_eq!(log.matches("maestro server started").count(), 1, "{log}");
            let mut entries: Vec<_> = std::fs::read_dir(&root)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            entries.sort();
            assert_eq!(entries, ["maestro-daemon.log"]);
        }
        Logged::Parent(root, output) => {
            let paths = root_paths(root.path());
            let socket = paths.api_socket_path();
            let expected = [
                "maestro server running; you can use any maestro CLI command in another terminal."
                    .to_owned(),
                format!("api socket: {socket}"),
                format!("logs: {}", paths.log_path("daemon")),
                "error: maestro server is already running".to_owned(),
                format!("api socket: {socket}"),
            ];
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert_eq!(stderr.lines().collect::<Vec<_>>(), expected);
        }
    }
}
