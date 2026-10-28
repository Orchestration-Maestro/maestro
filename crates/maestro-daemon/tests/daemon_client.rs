//! Retained daemon targets reach their own native socket endpoints, one request per
//! connection, with the shared response shapes and the live status read.
#![cfg(all(test, unix))]

use maestro_daemon::api::client::{ApiClientError, ConnectionTarget, DaemonClient};
use maestro_daemon::api::{RuntimeStatus, read_runtime_status_at};
use maestro_daemon::instances::{Selection, SelectionEnvironment};
use maestro_protocol::{
    rpc_types::{Request, RpcCommand, RpcResponse},
    schema::{ErrorBody, PingParams, ResponseResult, ServerCapabilities},
};
use maestro_test_directory::Scratch;
use std::os::unix::net::UnixListener;
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
};

/// A live selected endpoint returns a distinguishable reply while the default endpoint
/// remains bound; the peer also checks canonical request framing.
fn selected_request(
    root: &Path,
    path: &Path,
    client: &DaemonClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let default_path = root.join("daemon.sock");
    let default = reply(
        UnixListener::bind(&default_path)?,
        b"{\"endpoint\":\"default\"}\n",
    );
    std::fs::create_dir_all(
        path.parent()
            .ok_or_else(|| std::io::Error::other("missing parent"))?,
    )?;
    let selected = reply(UnixListener::bind(path)?, b"{\"endpoint\":\"selected\"}\n");
    let request = Request {
        id: Some("target".into()),
        command: RpcCommand::Ping(PingParams::default()),
    };
    let response = client.request_value(&request)?;
    // Complete the unused peer too, so both threads settle before the assertion.
    let unused = if response["endpoint"] == "selected" {
        &default_path
    } else {
        path
    };
    let cleanup = DaemonClient::for_target(ConnectionTarget::SocketPath(unused.into()));
    let cleaned = cleanup.request_value(&request);
    let selected_request = selected
        .join()
        .map_err(|_| std::io::Error::other("selected peer panicked"))??;
    let default_request = default
        .join()
        .map_err(|_| std::io::Error::other("default peer panicked"))??;
    assert_eq!(selected_request, "{\"id\":\"target\",\"type\":\"ping\"}\n");
    assert_eq!(default_request, "{\"id\":\"target\",\"type\":\"ping\"}\n");
    cleaned?;
    assert_eq!(response["endpoint"], "selected");
    Ok(())
}

#[test]
fn named_daemon_client_uses_the_supplied_path_table() {
    let root = Scratch::new_socket().unwrap();
    let selection = Selection::from_args(
        root.path().to_str().unwrap(),
        &[],
        Some("work"),
        SelectionEnvironment::default(),
    )
    .unwrap();
    let endpoint = std::path::PathBuf::from(&selection.api_socket_path);
    let client = DaemonClient::local(selection);
    assert_eq!(client.socket_path(), endpoint);
    selected_request(root.path(), &endpoint, &client).unwrap();
}

#[test]
fn explicit_client_socket_paths_keep_native_spelling() {
    let root = Scratch::new_socket().unwrap();
    std::fs::create_dir(root.path().join("selected")).unwrap();
    let endpoint = root.path().join("selected/../explicit.sock");
    let client = DaemonClient::for_target(ConnectionTarget::SocketPath(endpoint.clone()));
    assert_eq!(client.socket_path().as_os_str(), endpoint.as_os_str());
    selected_request(root.path(), &endpoint, &client).unwrap();
}

/// Accepts one request and supplies the exact response bytes from a controlled peer.
fn reply(
    listener: UnixListener,
    response: &'static [u8],
) -> std::thread::JoinHandle<std::io::Result<String>> {
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept()?;
        let mut line = String::new();
        BufReader::new(&mut stream).read_line(&mut line)?;
        stream.write_all(response)?;
        stream.flush()?;
        Ok(line)
    })
}

#[test]
fn raw_client_failures_keep_native_causes() {
    use std::error::Error;

    let root = Scratch::new_socket().unwrap();
    let request = Request {
        id: Some("target".into()),
        command: RpcCommand::Ping(PingParams::default()),
    };
    for (name, response) in [
        ("json", b"not json\n".as_slice()),
        ("empty", b"\n"),
        ("eof", b""),
    ] {
        let path = root.path().join(name);
        let peer = reply(UnixListener::bind(&path).unwrap(), response);
        let client = DaemonClient::for_target(ConnectionTarget::SocketPath(path));
        let error = client.request_value(&request).unwrap_err();
        assert_eq!(
            peer.join().unwrap().unwrap(),
            "{\"id\":\"target\",\"type\":\"ping\"}\n"
        );
        if name == "json" {
            let ApiClientError::Json(cause) = &error else {
                panic!("JSON failure required");
            };
            assert!(cause.is_syntax());
            assert!(error.source().unwrap().is::<serde_json::Error>());
            assert_eq!(error.to_string(), cause.to_string());
        } else {
            assert!(matches!(error, ApiClientError::EmptyResponse));
            assert!(error.source().is_none());
            assert_eq!(error.to_string(), "empty api response");
        }
    }
    let client =
        DaemonClient::for_target(ConnectionTarget::SocketPath(root.path().join("missing")));
    let error = client.request_value(&request).unwrap_err();
    let ApiClientError::Io(cause) = &error else {
        panic!("native failure required");
    };
    assert!(cause.raw_os_error().is_some());
    assert!(error.source().unwrap().is::<std::io::Error>());
    assert_eq!(error.to_string(), cause.to_string());
}

#[test]
fn client_request_keeps_large_metadata_sequence() {
    let root = Scratch::new_socket().unwrap();
    let path = root.path().join("sequence.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let peer = reply(listener, b"{\"success\":true}\n");
    let request = Request {
        id: Some("sequence".into()),
        command: RpcCommand::WorkspaceReportMetadata(
            maestro_protocol::schema::WorkspaceReportMetadataParams {
                workspace_id: "workspace".into(),
                source: "hook".into(),
                tokens: std::collections::HashMap::new(),
                seq: Some(9_007_199_254_740_993),
                ttl_ms: None,
            },
        ),
    };
    let client = DaemonClient::for_target(ConnectionTarget::SocketPath(path));
    let response = client.request_value(&request).unwrap();
    let received: serde_json::Value = serde_json::from_str(&peer.join().unwrap().unwrap()).unwrap();
    assert_eq!(response["success"], true);
    assert_eq!(received["seq"], 9_007_199_254_740_993_u64);
}

/// A ping request with the supplied correlation.
fn ping(id: &str) -> Request {
    Request {
        id: Some(id.into()),
        command: RpcCommand::Ping(PingParams::default()),
    }
}

/// A client for a fresh controlled peer that answers one request with `response`.
fn answering(
    root: &Path,
    name: &str,
    response: &'static [u8],
) -> (
    DaemonClient,
    std::thread::JoinHandle<std::io::Result<String>>,
) {
    let path = root.join(name);
    let peer = reply(UnixListener::bind(&path).unwrap(), response);
    (
        DaemonClient::for_target(ConnectionTarget::SocketPath(path)),
        peer,
    )
}

/// The peer's view of one held request: the line it read and the release of its reply.
struct Held {
    /// The request line, sent once it has been read.
    seen: std::sync::mpsc::Receiver<String>,
    /// Releases the reply.
    release: std::sync::mpsc::Sender<()>,
    /// The peer, finishing with its reply's write result.
    peer: std::thread::JoinHandle<std::io::Result<()>>,
}

/// Accepts one request, reports it, then writes `response` only once released.
fn held(listener: UnixListener, response: &'static [u8]) -> Held {
    let (report, seen) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let peer = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept()?;
        let mut line = String::new();
        BufReader::new(&mut stream).read_line(&mut line)?;
        report.send(line).map_err(std::io::Error::other)?;
        released.recv().map_err(std::io::Error::other)?;
        stream.write_all(response)?;
        stream.flush()
    });
    Held {
        seen,
        release,
        peer,
    }
}

/// A successful pong carrying all three status fields.
const PONG: &[u8] =
    b"{\"id\":\"shape\",\"type\":\"response\",\"command\":\"ping\",\"success\":true,\
\"data\":{\"type\":\"pong\",\"version\":\"1.2.3\",\"protocol\":21,\
\"capabilities\":{\"live_handoff\":true,\"detached_server_daemon\":false}}}\n";

/// The decoded form of [`PONG`]'s data.
fn pong() -> ResponseResult {
    ResponseResult::Pong {
        version: "1.2.3".into(),
        protocol: 21,
        capabilities: Some(ServerCapabilities {
            live_handoff: true,
            detached_server_daemon: false,
        }),
    }
}

/// A successful workspace list, which is not a ping result.
const LISTED: &[u8] =
    b"{\"id\":\"shape\",\"type\":\"response\",\"command\":\"ping\",\"success\":true,\
\"data\":{\"type\":\"workspace_list\",\"workspaces\":[]}}\n";

/// The canonical frame of `ping("shape")`.
const SHAPE_FRAME: &str = "{\"id\":\"shape\",\"type\":\"ping\"}\n";

#[test]
fn client_requests_preserve_response_and_error_shapes() {
    let root = Scratch::new_socket().unwrap();
    typed_success_keeps_raw_members(root.path());
    failures_keep_their_correlation(root.path());
    blank_and_undecodable_replies_fail(root.path());
    status_reads_only_a_pong(root.path());
}

/// A typed success decodes the shared envelope; the raw value keeps ignored members.
fn typed_success_keeps_raw_members(root: &Path) {
    let request = ping("shape");
    let (client, peer) = answering(root, "pong", PONG);
    let response = client.request(&request).unwrap();
    assert_eq!(peer.join().unwrap().unwrap(), SHAPE_FRAME);
    assert_eq!(
        response,
        RpcResponse {
            id: Some("shape".into()),
            command: "ping".into(),
            result: Ok(pong()),
        }
    );
    // Success data that looks like a failure is still a success.
    let looks_failed =
        b"{\"id\":\"shape\",\"type\":\"response\",\"command\":\"ping\",\"success\":true,\
\"data\":{\"type\":\"pong\",\"version\":\"1.2.3\",\"protocol\":21,\"error\":\"looks failed\",\
\"code\":\"timeout\",\"capabilities\":{\"live_handoff\":true,\"detached_server_daemon\":false}}}\n";
    let (client, peer) = answering(root, "looks-failed", looks_failed);
    assert_eq!(client.request(&request).unwrap().result, Ok(pong()));
    assert_eq!(peer.join().unwrap().unwrap(), SHAPE_FRAME);
    let (client, peer) = answering(root, "raw", looks_failed);
    let raw = client.request_value(&request).unwrap();
    assert_eq!(peer.join().unwrap().unwrap(), SHAPE_FRAME);
    assert_eq!(raw["data"]["error"], "looks failed");
    assert_eq!(raw["data"]["code"], "timeout");
}

/// A workspace failure keeps its correlation, command, code and message.
fn failures_keep_their_correlation(root: &Path) {
    use std::error::Error;

    let failed = b"{\"id\":\"shape\",\"type\":\"response\",\"command\":\"ping\",\"success\":false,\
\"error\":\"workspace w-1 not found\",\"code\":\"workspace_not_found\"}\n";
    let (client, peer) = answering(root, "failed", failed);
    let error = client.request(&ping("shape")).unwrap_err();
    assert_eq!(peer.join().unwrap().unwrap(), SHAPE_FRAME);
    let ApiClientError::ErrorResponse(response) = &error else {
        panic!("error response required, got {error:?}");
    };
    assert_eq!(
        **response,
        RpcResponse {
            id: Some("shape".into()),
            command: "ping".into(),
            result: Err(ErrorBody {
                code: "workspace_not_found".into(),
                message: "workspace w-1 not found".into(),
            }),
        }
    );
    assert_eq!(error.to_string(), "workspace w-1 not found");
    assert!(error.source().is_none());
}

/// Empty, blank, malformed and foreign replies fail with their own causes.
fn blank_and_undecodable_replies_fail(root: &Path) {
    let request = ping("shape");
    for (name, response) in [
        ("empty", b"\n".as_slice()),
        ("blank", b" \t\r\n"),
        ("eof", b""),
    ] {
        let (client, peer) = answering(root, name, response);
        let error = client.request(&request).unwrap_err();
        assert_eq!(peer.join().unwrap().unwrap(), SHAPE_FRAME);
        assert!(matches!(error, ApiClientError::EmptyResponse), "{name}");
        assert_eq!(error.to_string(), "empty api response");
    }
    for (name, response, syntax) in [
        ("malformed", b"not json\n".as_slice(), true),
        ("foreign", b"{\"endpoint\":\"elsewhere\"}\n", false),
    ] {
        let (client, peer) = answering(root, name, response);
        let error = client.request(&request).unwrap_err();
        assert_eq!(peer.join().unwrap().unwrap(), SHAPE_FRAME);
        let ApiClientError::Json(cause) = &error else {
            panic!("{name}: JSON failure required, got {error:?}");
        };
        assert_eq!(cause.is_syntax(), syntax, "{name}");
        assert_eq!(error.to_string(), cause.to_string());
    }
}

/// Status returns the three pong fields and names any other result.
fn status_reads_only_a_pong(root: &Path) {
    let (client, peer) = answering(root, "status", PONG);
    let status = client.status().unwrap();
    assert_eq!(status, pong_status());
    assert_eq!(
        peer.join().unwrap().unwrap(),
        "{\"id\":\"api-client:status\",\"type\":\"ping\"}\n"
    );
    let (client, peer) = answering(root, "listed", LISTED);
    let error = client.status().unwrap_err();
    let listed = format!(
        "{:?}",
        ResponseResult::WorkspaceList {
            workspaces: Vec::new()
        }
    );
    assert!(matches!(&error, ApiClientError::UnexpectedResult(result) if *result == listed));
    assert_eq!(
        error.to_string(),
        format!("unexpected api result: {listed}")
    );
    peer.join().unwrap().unwrap();
}

/// The status [`PONG`] reports.
fn pong_status() -> RuntimeStatus {
    RuntimeStatus {
        version: Some("1.2.3".into()),
        protocol: Some(21),
        capabilities: Some(ServerCapabilities {
            live_handoff: true,
            detached_server_daemon: false,
        }),
    }
}

/// The per-call timeout the timeout cases use.
const TIMEOUT: std::time::Duration = std::time::Duration::from_millis(50);

#[test]
fn client_timeouts_reach_both_native_operations() {
    let root = Scratch::new_socket().unwrap();
    receive_timeouts_leave_ordinary_requests_waiting(root.path());
    send_timeouts_stop_an_unread_request(root.path());
    zero_timeouts_are_native_invalid_input(root.path());
}

/// A timed request expires on the named daemon's own connection while an ordinary request,
/// which has no timeout, keeps waiting and then reads its late reply.
fn receive_timeouts_leave_ordinary_requests_waiting(root: &Path) {
    let ordinary_path = root.join("ordinary.sock");
    let ordinary = held(
        UnixListener::bind(&ordinary_path).unwrap(),
        b"{\"answer\":\"late\"}\n",
    );
    let client = DaemonClient::for_target(ConnectionTarget::SocketPath(ordinary_path));
    let waiting = std::thread::spawn(move || client.request_value(&ping("ordinary")));
    assert_eq!(
        ordinary.seen.recv().unwrap(),
        "{\"id\":\"ordinary\",\"type\":\"ping\"}\n"
    );

    let selection = Selection::from_args(
        root.to_str().unwrap(),
        &[],
        Some("held"),
        SelectionEnvironment::default(),
    )
    .unwrap();
    let held_path = std::path::PathBuf::from(&selection.api_socket_path);
    std::fs::create_dir_all(held_path.parent().unwrap()).unwrap();
    let receive = held(UnixListener::bind(&held_path).unwrap(), b"{}\n");
    let started = std::time::Instant::now();
    let error = DaemonClient::local(selection)
        .request_value_with_timeout(&ping("held"), TIMEOUT)
        .unwrap_err();
    let elapsed = started.elapsed();
    let ApiClientError::Io(cause) = &error else {
        panic!("native receive timeout required, got {error:?}");
    };
    assert_eq!(cause.kind(), std::io::ErrorKind::WouldBlock);
    assert!(elapsed >= TIMEOUT, "{elapsed:?}");
    assert_eq!(
        receive.seen.recv().unwrap(),
        "{\"id\":\"held\",\"type\":\"ping\"}\n"
    );
    receive.release.send(()).unwrap();
    // The reply may meet a closed connection; only the request mattered.
    let _ = receive.peer.join().unwrap();

    ordinary.release.send(()).unwrap();
    assert_eq!(waiting.join().unwrap().unwrap()["answer"], "late");
    ordinary.peer.join().unwrap().unwrap();
}

/// The send timeout stops a request larger than the socket buffers that the peer never reads.
fn send_timeouts_stop_an_unread_request(root: &Path) {
    let stalled_path = root.join("stalled.sock");
    let listener = UnixListener::bind(&stalled_path).unwrap();
    let unread = std::thread::spawn(move || listener.accept().map(|(stream, _)| stream));
    let large = Request {
        id: Some("large".into()),
        command: RpcCommand::WorkspaceReportMetadata(
            maestro_protocol::schema::WorkspaceReportMetadataParams {
                workspace_id: "workspace".into(),
                source: "hook".into(),
                tokens: std::collections::HashMap::from([(
                    "token".to_owned(),
                    Some("x".repeat(8 << 20)),
                )]),
                seq: None,
                ttl_ms: None,
            },
        ),
    };
    let error = DaemonClient::for_target(ConnectionTarget::SocketPath(stalled_path))
        .request_value_with_timeout(&large, TIMEOUT)
        .unwrap_err();
    let ApiClientError::Io(cause) = &error else {
        panic!("native send timeout required, got {error:?}");
    };
    assert_eq!(cause.kind(), std::io::ErrorKind::WouldBlock);
    drop(unread.join().unwrap().unwrap());
}

/// A zero timeout is the platform's own invalid input, before any request is written.
fn zero_timeouts_are_native_invalid_input(root: &Path) {
    let (client, peer) = answering(root, "zero.sock", b"");
    let error = client
        .request_value_with_timeout(&ping("zero"), std::time::Duration::ZERO)
        .unwrap_err();
    let ApiClientError::Io(cause) = &error else {
        panic!("native invalid input required, got {error:?}");
    };
    assert_eq!(cause.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(peer.join().unwrap().unwrap(), "");
}

#[test]
fn runtime_status_distinguishes_absence_from_failure() {
    let root = Scratch::new_socket().unwrap();
    let path = root.path().join("live.sock");
    let peer = reply(UnixListener::bind(&path).unwrap(), PONG);
    let status = read_runtime_status_at(&path, TIMEOUT).unwrap();
    assert_eq!(status, Some(pong_status()));
    assert_eq!(
        peer.join().unwrap().unwrap(),
        "{\"id\":\"runtime:status\",\"type\":\"ping\"}\n"
    );
    unreachable_daemons_have_no_status(root.path());
    status_failures_keep_their_diagnostics(root.path());
    let mut names: Vec<_> = std::fs::read_dir(root.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    // Only the sockets the test bound exist: the read writes no file.
    assert_eq!(
        names,
        [
            "denied.sock",
            "failed.sock",
            "listed.sock",
            "live.sock",
            "malformed.sock",
            "refused.sock",
            "silent.sock"
        ]
    );
}

/// An absent socket, a refused connection and a daemon that never answers have no status.
fn unreachable_daemons_have_no_status(root: &Path) {
    assert_eq!(
        read_runtime_status_at(&root.join("absent.sock"), TIMEOUT).unwrap(),
        None
    );
    let refused = root.join("refused.sock");
    drop(UnixListener::bind(&refused).unwrap());
    assert_eq!(read_runtime_status_at(&refused, TIMEOUT).unwrap(), None);
    // A live daemon that never answers reaches the Unix receive timeout.
    let silent = root.join("silent.sock");
    let quiet = held(UnixListener::bind(&silent).unwrap(), b"");
    assert_eq!(read_runtime_status_at(&silent, TIMEOUT).unwrap(), None);
    assert_eq!(
        quiet.seen.recv().unwrap(),
        "{\"id\":\"runtime:status\",\"type\":\"ping\"}\n"
    );
    quiet.release.send(()).unwrap();
    let _ = quiet.peer.join().unwrap();
}

/// Refusals, undecodable replies, other results and other native failures propagate.
fn status_failures_keep_their_diagnostics(root: &Path) {
    let failed = b"{\"id\":\"runtime:status\",\"type\":\"response\",\"command\":\"ping\",\
\"success\":false,\"error\":\"server is shutting down\",\"code\":\"server_unavailable\"}\n";
    let malformed = serde_json::from_str::<serde_json::Value>("not json")
        .unwrap_err()
        .to_string();
    let listed = format!(
        "server status request returned unexpected result: {:?}",
        ResponseResult::WorkspaceList {
            workspaces: Vec::new()
        }
    );
    for (name, response, expected) in [
        ("failed.sock", failed.as_slice(), "server is shutting down"),
        ("malformed.sock", b"not json\n", &malformed),
        ("listed.sock", LISTED, &listed),
    ] {
        let path = root.join(name);
        let peer = reply(UnixListener::bind(&path).unwrap(), response);
        let error = read_runtime_status_at(&path, TIMEOUT).unwrap_err();
        peer.join().unwrap().unwrap();
        assert_eq!(error.kind(), std::io::ErrorKind::Other, "{name}");
        assert_eq!(error.to_string(), expected, "{name}");
    }
    let denied = root.join("denied.sock");
    let _listener = UnixListener::bind(&denied).unwrap();
    if let Some(_restore) = maestro_test_directory::deny(&denied, 0).unwrap() {
        let error = read_runtime_status_at(&denied, TIMEOUT).unwrap_err();
        let inner = error
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<ApiClientError>());
        let Some(ApiClientError::Io(cause)) = inner else {
            panic!("native failure required, got {error:?}");
        };
        assert_eq!(cause.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(error.to_string(), cause.to_string());
    }
}

#[test]
fn status_preserves_inaccessible_parent_errors() {
    let root = Scratch::new_socket().unwrap();
    let parent = root.path().join("inaccessible");
    std::fs::create_dir(&parent).unwrap();
    let socket = parent.join("api.sock");
    let _listener = UnixListener::bind(&socket).unwrap();
    let Some(_restore) = maestro_test_directory::deny(&parent, 0).unwrap() else {
        return;
    };
    let native = socket.try_exists().unwrap_err();
    let error = read_runtime_status_at(&socket, TIMEOUT).unwrap_err();
    assert_eq!(error.kind(), native.kind());
    assert_eq!(error.to_string(), native.to_string());
}
