//! A one-connection HTTP server on the loopback interface.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::thread::{JoinHandle, sleep};
use std::time::Duration;

use crate::chat::TestResult;

/// What the server read from its client.
pub struct Received {
    /// Method, target and version.
    pub request_line: String,
    /// Header names (lowercase) and values.
    pub headers: Vec<(String, String)>,
    /// Request body.
    pub body: Vec<u8>,
}

impl Received {
    /// First value of a header.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| value.as_str())
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

/// Answer one request with `response_head` and then `pieces` written one at a time, pausing between them.
pub fn serve(
    response_head: &'static str,
    pieces: Vec<Vec<u8>>,
    pause: Duration,
) -> TestResult<Loopback> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://{}", listener.local_addr()?);
    let (sender, received) = channel();
    let worker = std::thread::spawn(move || {
        let (mut connection, _) = listener.accept()?;
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        let head = loop {
            let count = connection.read(&mut buffer)?;
            bytes.extend_from_slice(&buffer[..count]);
            if let Some(end) = header_end(&bytes) {
                break end;
            }
            if count == 0 {
                return Ok(());
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
        let body = bytes[head..].to_vec();
        sender
            .send(Received {
                request_line,
                headers,
                body,
            })
            .ok();
        connection.write_all(response_head.as_bytes())?;
        for piece in pieces {
            connection.write_all(&piece)?;
            connection.flush()?;
            sleep(pause);
        }
        Ok(())
    });
    Ok(Loopback {
        url,
        received,
        worker,
    })
}
