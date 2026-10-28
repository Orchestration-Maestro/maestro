//! A graceful stop waits for the selected sockets to disappear within one deadline.
use super::*;
use crate::instances::SelectionEnvironment;
use maestro_test_directory::Scratch;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

/// The canonical stop request line.
const STOP_LINE: &str = "{\"id\":\"cli:daemon:stop\",\"type\":\"server.stop\"}\n";

/// A successful stop reply.
const STOPPED: &[u8] =
    b"{\"id\":\"cli:daemon:stop\",\"type\":\"response\",\"command\":\"server.stop\",\
\"success\":true,\"data\":{\"type\":\"ok\"}}\n";

/// A failed stop reply.
const REFUSED: &[u8] =
    b"{\"id\":\"cli:daemon:stop\",\"type\":\"response\",\"command\":\"server.stop\",\
\"success\":false,\"error\":\"server is shutting down\",\"code\":\"server_unavailable\"}\n";

/// The stop budget the short-deadline cases use.
const SHORT: Duration = Duration::from_millis(75);

/// How a controlled API endpoint treats the stop connection.
enum Answer {
    /// Reads the request, then writes these bytes and closes.
    Bytes(&'static [u8]),
    /// Reads the request and keeps the connection open without replying.
    Hold,
    /// Closes the connection without reading the request.
    Close,
}

/// What an endpoint observed.
#[derive(Debug)]
struct Seen {
    /// The stop request line, empty when none was read.
    request: String,
    /// Reachability probes accepted after the stop connection.
    probes: usize,
}

/// A controlled endpoint that disappears after a number of probes or stays until finished.
struct Endpoint {
    /// The bound socket path.
    path: PathBuf,
    /// Tells a lasting endpoint to end at its next connection.
    released: Arc<AtomicBool>,
    /// The serving thread.
    peer: JoinHandle<std::io::Result<Seen>>,
}

impl Endpoint {
    /// Binds `path`. `answer` handles the stop connection of an API endpoint; `None` makes a
    /// client endpoint that only receives probes. After `probes` probes the socket file is
    /// removed; `None` keeps it until [`Endpoint::finish`].
    fn bind(path: &Path, answer: Option<Answer>, probes: Option<usize>) -> Self {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let listener = UnixListener::bind(path).unwrap();
        let released = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&released);
        let owned = path.to_owned();
        let peer = std::thread::spawn(move || serve(&listener, &owned, answer, probes, &flag));
        Self {
            path: path.to_owned(),
            released,
            peer,
        }
    }

    /// Ends the endpoint, waking a lasting one with one last connection.
    fn finish(self) -> Seen {
        self.released.store(true, Ordering::SeqCst);
        if self.path.exists() {
            drop(UnixStream::connect(&self.path));
        }
        self.peer.join().unwrap().unwrap()
    }
}

/// Serves one endpoint until it disappears or is released.
fn serve(
    listener: &UnixListener,
    path: &Path,
    answer: Option<Answer>,
    probes: Option<usize>,
    released: &AtomicBool,
) -> std::io::Result<Seen> {
    let mut request = String::new();
    let mut held = None;
    if let Some(answer) = answer {
        let (mut stream, _) = listener.accept()?;
        match answer {
            Answer::Bytes(bytes) => {
                BufReader::new(&mut stream).read_line(&mut request)?;
                stream.write_all(bytes)?;
            }
            Answer::Hold => {
                BufReader::new(&mut stream).read_line(&mut request)?;
                held = Some(stream);
            }
            Answer::Close => drop(stream),
        }
    }
    let mut count = 0;
    while probes != Some(count) {
        listener.accept()?;
        if released.load(Ordering::SeqCst) {
            return Ok(Seen {
                request,
                probes: count,
            });
        }
        count += 1;
    }
    std::fs::remove_file(path)?;
    drop(held);
    Ok(Seen {
        request,
        probes: count,
    })
}

/// An explicit named selection under `root`.
fn named(root: &Scratch, name: &str) -> Selection {
    Selection::from_args(
        root.path().to_str().unwrap(),
        &[],
        Some(name),
        SelectionEnvironment::default(),
    )
    .unwrap()
}

/// The deadline diagnostic naming the still-reachable sockets.
fn still_reachable(label: &str, paths: &[&Path]) -> String {
    let paths: Vec<_> = paths
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    format!(
        "daemon {label} did not stop within 75ms; sockets are still reachable at {}",
        paths.join(", ")
    )
}

#[test]
fn graceful_stop_retains_the_full_shutdown_budget() {
    assert_eq!(STOP_WAIT_TIMEOUT, Duration::from_secs(15));
    let root = Scratch::new().unwrap();
    let selection = named(&root, "slow");
    let api = Endpoint::bind(
        Path::new(&selection.api_socket_path),
        Some(Answer::Bytes(STOPPED)),
        Some(40),
    );
    let info = stop_session(&selection).unwrap();
    assert_eq!(info.name, "slow");
    assert!(!info.running);
    let seen = api.finish();
    assert_eq!(seen.request, STOP_LINE);
    assert_eq!(seen.probes, 40);
}

#[test]
fn retryable_stop_failures_defer_to_socket_reachability() {
    for kind in [
        io::ErrorKind::BrokenPipe,
        io::ErrorKind::ConnectionReset,
        io::ErrorKind::UnexpectedEof,
        io::ErrorKind::NotConnected,
        io::ErrorKind::TimedOut,
        io::ErrorKind::WouldBlock,
    ] {
        assert!(stop_request_error_allows_wait(&kind.into()), "{kind:?}");
    }
    for kind in [io::ErrorKind::PermissionDenied, io::ErrorKind::InvalidData] {
        assert!(!stop_request_error_allows_wait(&kind.into()), "{kind:?}");
    }
    // A connection closed before its request is read still waits for the socket itself.
    let root = Scratch::new().unwrap();
    let selection = named(&root, "closing");
    let api = Endpoint::bind(
        Path::new(&selection.api_socket_path),
        Some(Answer::Close),
        Some(3),
    );
    assert_eq!(
        stop_with_timeout(&selection, Duration::from_secs(5)),
        Ok(())
    );
    assert_eq!(api.finish().probes, 3);
}

#[test]
fn unavailable_stop_timeouts_still_observe_socket_shutdown() {
    assert!(stop_timeout_error_allows_wait(
        &io::ErrorKind::InvalidInput.into()
    ));
    assert!(!stop_timeout_error_allows_wait(
        &io::ErrorKind::PermissionDenied.into()
    ));
}

#[test]
fn unsupported_stop_timeouts_keep_platform_policy() {
    assert_eq!(
        stop_timeout_error_allows_wait(&io::ErrorKind::Unsupported.into()),
        cfg!(windows)
    );
}

#[test]
fn stop_socket_timeouts_keep_a_positive_minimum() {
    assert_eq!(socket_timeout_from_remaining(Duration::ZERO), None);
    assert_eq!(
        socket_timeout_from_remaining(Duration::from_nanos(1)),
        Some(Duration::from_millis(1))
    );
    assert_eq!(
        socket_timeout_from_remaining(Duration::from_millis(1)),
        Some(Duration::from_millis(1))
    );
    assert_eq!(
        socket_timeout_from_remaining(Duration::from_millis(10)),
        Some(Duration::from_millis(10))
    );
}

#[test]
fn an_empty_stop_response_defers_to_socket_shutdown() {
    let root = Scratch::new().unwrap();
    let selection = named(&root, "empty");
    let api = Endpoint::bind(
        Path::new(&selection.api_socket_path),
        Some(Answer::Bytes(b"")),
        Some(2),
    );
    let client = Endpoint::bind(&selection.client_socket_path, None, Some(5));
    assert_eq!(
        stop_with_timeout(&selection, Duration::from_secs(5)),
        Ok(())
    );
    let api = api.finish();
    assert_eq!(api.request, STOP_LINE);
    assert_eq!(api.probes, 2);
    assert_eq!(client.finish().probes, 5);
}

#[test]
fn a_silent_reachable_daemon_exhausts_the_stop_budget() {
    let root = Scratch::new().unwrap();
    let selection = named(&root, "silent");
    let path = PathBuf::from(&selection.api_socket_path);
    let api = Endpoint::bind(&path, Some(Answer::Hold), None);
    assert_eq!(
        stop_with_timeout(&selection, SHORT),
        Err(still_reachable("silent", &[&path]))
    );
    assert_eq!(api.finish().request, STOP_LINE);
}

#[test]
fn a_responding_reachable_daemon_still_exhausts_the_stop_budget() {
    let root = Scratch::new().unwrap();
    let selection = named(&root, "lingering");
    let path = PathBuf::from(&selection.api_socket_path);
    let api = Endpoint::bind(&path, Some(Answer::Bytes(STOPPED)), None);
    assert_eq!(
        stop_with_timeout(&selection, SHORT),
        Err(still_reachable("lingering", &[&path]))
    );
    assert_eq!(api.finish().request, STOP_LINE);
}

#[test]
fn stop_errors_and_both_endpoints_preserve_the_deadline() {
    let root = Scratch::new().unwrap();
    let selection = named(&root, "work");
    let api_path = PathBuf::from(&selection.api_socket_path);
    let client_path = selection.client_socket_path.clone();

    let native = maestro_ipc::local_socket::connect_local_stream(&api_path).unwrap_err();
    assert_eq!(
        stop_with_timeout(&selection, SHORT),
        Err(format!(
            "daemon work is not running or cannot be reached at {}: {native}",
            api_path.display()
        ))
    );

    let api = Endpoint::bind(&api_path, Some(Answer::Bytes(REFUSED)), None);
    assert_eq!(
        stop_with_timeout(&selection, SHORT),
        Err("server is shutting down".to_owned())
    );
    assert_eq!(api.finish().request, STOP_LINE);
    std::fs::remove_file(&api_path).unwrap();

    let malformed = serde_json::from_str::<serde_json::Value>("not json\n")
        .unwrap_err()
        .to_string();
    let api = Endpoint::bind(&api_path, Some(Answer::Bytes(b"not json\n")), None);
    assert_eq!(stop_with_timeout(&selection, SHORT), Err(malformed));
    assert_eq!(api.finish().request, STOP_LINE);
    std::fs::remove_file(&api_path).unwrap();

    // A transport failure defers to the sockets, and both remain reachable.
    let api = Endpoint::bind(&api_path, Some(Answer::Close), None);
    let client = Endpoint::bind(&client_path, None, None);
    assert_eq!(
        stop_with_timeout(&selection, SHORT),
        Err(still_reachable("work", &[&api_path, &client_path]))
    );
    api.finish();
    client.finish();
    std::fs::remove_file(&api_path).unwrap();
    std::fs::remove_file(&client_path).unwrap();

    // The API socket disappears but the client socket stays.
    let api = Endpoint::bind(&api_path, Some(Answer::Bytes(STOPPED)), Some(0));
    let client = Endpoint::bind(&client_path, None, None);
    assert_eq!(
        stop_with_timeout(&selection, SHORT),
        Err(still_reachable("work", &[&client_path]))
    );
    assert_eq!(api.finish().request, STOP_LINE);
    client.finish();
}
