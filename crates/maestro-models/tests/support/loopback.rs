//! A one-connection HTTP server on the loopback interface.

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
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| value.as_str())
    }

    /// The bytes after `name: ` on the header line of that name (lowercase), as received.
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
fn read_request(connection: &mut TcpStream) -> std::io::Result<Option<Received>> {
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
