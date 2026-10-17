//! A one-connection HTTP server on the loopback interface.

use std::fmt::Write as _;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, channel};
use std::thread::JoinHandle;

/// Fallible test body result.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// What the server read from its client.
pub struct Received {
    /// Method, target and version.
    pub request_line: String,
    /// Header names (lowercase) and values.
    pub headers: Vec<(String, String)>,
    /// Request body.
    pub body: Vec<u8>,
    /// The request line and header block exactly as received.
    pub head: Vec<u8>,
}

impl Received {
    /// First value of a header.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| value.as_str())
    }

    /// The bytes after `name: ` on the header line of that name (lowercase), as received.
    #[must_use]
    pub fn header_octets(&self, name: &str) -> Option<&[u8]> {
        let prefix = format!("{name}: ").into_bytes();
        self.head
            .split(|byte| *byte == b'\n')
            .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
            .find_map(|line| {
                let start = line.get(..prefix.len())?;
                start
                    .eq_ignore_ascii_case(&prefix)
                    .then(|| &line[prefix.len()..])
            })
    }
}

/// A running server and the request it received.
pub struct Loopback {
    /// Base URL of the server.
    pub url: String,
    received: Receiver<Received>,
    worker: JoinHandle<std::io::Result<()>>,
}

impl Loopback {
    /// Wait for the exchange to finish and return what the client sent.
    ///
    /// # Errors
    /// Returns worker failures, panics or a missing request.
    pub fn finish(self) -> TestResult<Received> {
        self.worker.join().map_err(|_| "server thread panicked")??;
        Ok(self.received.recv()?)
    }
}

/// Position of the end of the header block.
fn header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|at| at + 4)
}

/// Status line and headers of an ordinary event-stream answer.
pub const EVENT_STREAM_HEAD: &str =
    "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n";

/// One action of the server after it has answered the request head.
pub enum Step {
    /// Write and flush these bytes.
    Write(Vec<u8>),
    /// Hold the response until the test sends on, or drops, the paired sender.
    Wait(Receiver<()>),
}

impl Step {
    /// A write step for each piece, in order.
    pub fn writes(pieces: Vec<Vec<u8>>) -> Vec<Self> {
        pieces.into_iter().map(Self::Write).collect()
    }
}

/// Whether the client closed the connection, which ends the response early without failing it.
fn client_closed(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::BrokenPipe | ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted
    )
}

/// Read one request, or `None` when the client closes before finishing its head; a body cut
/// short by the client is returned as received.
///
/// # Errors
/// Returns socket read failures.
pub fn read_request(connection: &mut impl Read) -> std::io::Result<Option<Received>> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let head = loop {
        let count = connection.read(&mut buffer)?;
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(end) = header_end(&bytes) {
            break end;
        }
        if count == 0 {
            return Ok(None);
        }
    };
    let text = String::from_utf8_lossy(&bytes[..head]).into_owned();
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or_default().to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let length: usize = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or(0);
    while bytes.len() < head + length {
        let count = connection.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    Ok(Some(Received {
        request_line,
        headers,
        body: bytes[head..].to_vec(),
        head: bytes[..head].to_vec(),
    }))
}

/// Answer one request with `response_head` and then carry out `steps` in order.
///
/// # Errors
/// Returns listener setup failures.
pub fn serve(response_head: &str, steps: Vec<Step>) -> TestResult<Loopback> {
    let response_head = response_head.to_owned();
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://{}", listener.local_addr()?);
    let (sender, received) = channel();
    let worker = std::thread::spawn(move || {
        let (mut connection, _) = listener.accept()?;
        let Some(request) = read_request(&mut connection)? else {
            return Ok(());
        };
        sender.send(request).ok();
        connection.write_all(response_head.as_bytes())?;
        for step in steps {
            let written = match step {
                Step::Write(piece) => connection
                    .write_all(&piece)
                    .and_then(|()| connection.flush()),
                Step::Wait(gate) => {
                    gate.recv().ok();
                    Ok(())
                }
            };
            match written {
                Err(error) if client_closed(&error) => break,
                other => other?,
            }
        }
        Ok(())
    });
    Ok(Loopback {
        url,
        received,
        worker,
    })
}

/// What a routed server answers to one request.
pub struct Reply {
    /// Status code.
    pub status: u16,
    /// Content type.
    pub content_type: &'static str,
    /// Extra response headers.
    pub headers: &'static [(&'static str, &'static str)],
    /// Response body.
    pub body: Vec<u8>,
    /// Whether to close the connection without answering.
    pub close: bool,
}

/// A loopback server that answers every request through one handler until it is finished or
/// dropped.
pub struct Routed {
    /// Base URL of the server.
    pub url: String,
    address: std::net::SocketAddr,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    worker: Option<JoinHandle<std::io::Result<()>>>,
}

impl Routed {
    /// Wake the accept loop; refusal or peer closure means the listener already stopped.
    fn stop_accepting(&self) -> std::io::Result<()> {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        match TcpStream::connect(self.address) {
            Err(error) if error.kind() == ErrorKind::ConnectionRefused || client_closed(&error) => {
                Ok(())
            }
            other => other.map(drop),
        }
    }

    /// Stop accepting connections and wait until every admitted request was answered.
    ///
    /// # Errors
    /// Returns stop, worker or join failures after waiting for admitted connections.
    pub fn finish(mut self) -> TestResult {
        self.stop_accepting()?;
        let worker = self.worker.take().ok_or("server already finished")?;
        worker.join().map_err(|_| "server thread panicked")??;
        Ok(())
    }
}

impl Drop for Routed {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            self.stop_accepting().ok();
            worker.join().ok();
        }
    }
}

/// Read one request from `connection`, answer it with `handler` and close.
///
/// # Errors
/// Returns request read or response write failures.
pub fn answer_connection(
    mut connection: impl Read + Write,
    handler: &dyn Fn(&Received) -> Reply,
) -> std::io::Result<()> {
    let Some(request) = read_request(&mut connection)? else {
        return Ok(());
    };
    let reply = handler(&request);
    answer_connection_reply(connection, &reply)
}

/// Write an already-selected response after a caller-owned handshake or observation.
///
/// # Errors
/// Returns response write failures.
pub fn answer_connection_reply(mut connection: impl Write, reply: &Reply) -> std::io::Result<()> {
    if reply.close {
        return Ok(());
    }
    let mut head = format!(
        "HTTP/1.1 {} Controlled\r\ncontent-type: {}\r\ncontent-length: {}\r\nconnection: close\r\n",
        reply.status,
        reply.content_type,
        reply.body.len()
    );
    for (name, value) in reply.headers {
        let _ = write!(head, "{name}: {value}\r\n");
    }
    head.push_str("\r\n");
    connection.write_all(head.as_bytes())?;
    connection.write_all(&reply.body)?;
    connection.flush()
}

/// One admitted connection, handled before the accept owner joins it.
type Handler = std::sync::Arc<dyn Fn(TcpStream) -> std::io::Result<()> + Send + Sync>;

/// Accept connections until `stopped` is set, then wait for every admitted request; the result
/// is the first failure, after all of them were waited for.
fn serve_until_stopped(
    listener: &TcpListener,
    stopped: &std::sync::atomic::AtomicBool,
    handler: &Handler,
) -> std::io::Result<()> {
    let mut connections = Vec::new();
    let mut accepted = Ok(());
    while !stopped.load(std::sync::atomic::Ordering::SeqCst) {
        let connection = match listener.accept() {
            Ok((connection, _)) => connection,
            Err(error) => {
                accepted = Err(error);
                break;
            }
        };
        if stopped.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        let handler = std::sync::Arc::clone(handler);
        connections.push(std::thread::spawn(move || handler(connection)));
    }
    connections
        .into_iter()
        .map(|connection| {
            connection
                .join()
                .unwrap_or_else(|_| Err(std::io::Error::other("connection thread panicked")))
        })
        .fold(accepted, Result::and)
}

/// Answer each request on its own connection with `handler`, and close the connection after it.
///
/// # Errors
/// Returns listener setup failures.
pub fn serve_routed(
    handler: impl Fn(&Received) -> Reply + Send + Sync + 'static,
) -> TestResult<Routed> {
    start_routed(std::sync::Arc::new(move |connection| {
        answer_connection(connection, &handler)
    }))
}

/// Close each admitted connection before HTTP parsing, recording its admission.
///
/// # Errors
/// Returns listener setup failures.
pub fn serve_closed(on_accept: impl Fn() + Send + Sync + 'static) -> TestResult<Routed> {
    start_routed(std::sync::Arc::new(move |connection| {
        on_accept();
        connection.shutdown(std::net::Shutdown::Both)
    }))
}

/// Start the shared accept/join owner with a connection handler.
///
/// # Errors
/// Returns listener setup failures.
pub fn start_routed(handler: Handler) -> TestResult<Routed> {
    start_routed_at("127.0.0.1:0", handler)
}

/// Start the shared owner on an explicitly selected loopback address family.
///
/// # Errors
/// Returns bind or local-address lookup failures.
pub fn start_routed_at(address: &str, handler: Handler) -> TestResult<Routed> {
    let listener = TcpListener::bind(address)?;
    let address = listener.local_addr()?;
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stopped = std::sync::Arc::clone(&stop);
    let worker = std::thread::spawn(move || serve_until_stopped(&listener, &stopped, &handler));
    Ok(Routed {
        url: format!("http://{address}"),
        address,
        stop,
        worker: Some(worker),
    })
}
