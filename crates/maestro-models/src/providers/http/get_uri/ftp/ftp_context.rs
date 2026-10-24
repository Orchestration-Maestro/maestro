//! The owned FTP connections: the one command sender and the one inactivity clock.

use std::collections::VecDeque;
use std::io;
use std::net::{IpAddr, SocketAddr};

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;

use super::INACTIVITY;
use super::parse_control_response::{Frames, Reply};
use crate::providers::http::get_uri::Failure;

/// How long the context waits for a connection, a reply, a write or a read.
///
/// One inactivity window resets with each completed operation, so a peer that keeps sending or
/// receiving bytes may take longer than the window overall. While admitted transfer data is
/// active the control connection waits without a deadline, as the reference suspends that timer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Deadline {
    /// One inactivity window.
    Inactivity,
    /// No deadline.
    Open,
}

/// The connection whose silence ended a wait.
#[derive(Clone, Copy, Debug)]
enum Socket {
    /// The control connection.
    Control,
    /// The data connection.
    Data,
}

impl Socket {
    /// The name a timeout is reported under.
    fn name(self) -> &'static str {
        match self {
            Self::Control => "control socket",
            Self::Data => "data socket",
        }
    }
}

/// A connection that stayed silent for one inactivity window.
#[derive(Debug)]
struct Silent(Socket);

impl std::fmt::Display for Silent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Timeout ({})", self.0.name())
    }
}

impl std::error::Error for Silent {}

/// A server reply that refuses the command it answers.
#[derive(Debug)]
pub(super) struct FtpError {
    /// The three-digit reply code.
    pub(super) code: u16,
    /// The reply text the server sent.
    pub(super) message: String,
}

impl FtpError {
    /// The failure a refused reply stands for.
    pub(super) fn of(reply: Reply) -> Self {
        Self {
            code: reply.code,
            message: reply.message,
        }
    }
}

impl std::fmt::Display for FtpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for FtpError {}

/// A data connection that could not be prepared, which leaves the session usable.
#[derive(Debug)]
pub(super) struct Unprepared(String);

impl std::fmt::Display for Unprepared {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unprepared {}

/// The failure of a command submitted to a session that has already closed.
fn closed(reason: &str) -> Failure {
    io::Error::other(format!("Client is closed because {reason}")).into()
}

/// Connect a data socket, reporting a preparation failure instead of ending the session.
pub(super) async fn connect_data(address: SocketAddr) -> Result<Data<TcpStream>, Unprepared> {
    match tokio::time::timeout(INACTIVITY, TcpStream::connect(address)).await {
        Ok(Ok(stream)) => Ok(Data::new(stream)),
        Ok(Err(error)) => Err(Unprepared(format!(
            "Can't open data connection in passive mode: {error}"
        ))),
        Err(_) => Err(Unprepared(format!(
            "Timeout when trying to open data connection to {address}"
        ))),
    }
}

/// An installed data connection, or a reader whose inactivity bounds one payload.
pub(super) struct Data<R> {
    /// The reader carrying the transfer.
    reader: R,
}

impl<R> Data<R> {
    /// Wrap a reader that the inactivity window governs.
    pub(super) fn new(reader: R) -> Self {
        Self { reader }
    }
}

impl<R: AsyncRead + Unpin> Data<R> {
    /// Read one chunk, failing when the reader stays silent for one inactivity window.
    ///
    /// # Errors
    /// Returns the reader's own error, or a data-socket timeout.
    pub(super) async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Failure> {
        match tokio::time::timeout(INACTIVITY, self.reader.read(buffer)).await {
            Ok(Ok(read)) => Ok(read),
            Ok(Err(error)) => Err(error.into()),
            Err(_) => Err(Box::new(Silent(Socket::Data))),
        }
    }
}

impl Data<TcpStream> {
    /// Close the write side, which a peer waiting for the end of the transfer observes.
    async fn shutdown(&mut self) {
        self.reader.shutdown().await.ok();
    }
}

/// A payload accumulated under an optional byte limit.
pub(super) struct Payload {
    /// The bytes admitted so far.
    bytes: Vec<u8>,
    /// The most bytes the payload accepts.
    limit: Option<usize>,
}

impl Payload {
    /// An empty payload that admits at most `limit` bytes.
    pub(super) fn bounded(limit: Option<usize>) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }

    /// Admit one chunk, refusing the first byte beyond the limit.
    ///
    /// # Errors
    /// Returns the listing size refusal.
    pub(super) fn extend(&mut self, chunk: &[u8]) -> Result<(), Failure> {
        if self
            .limit
            .is_some_and(|limit| self.bytes.len() + chunk.len() > limit)
        {
            return Err(io::Error::other(format!(
                "The FTP server sent a directory listing larger than {} bytes",
                super::LISTING_LIMIT
            ))
            .into());
        }
        self.bytes.extend_from_slice(chunk);
        Ok(())
    }

    /// The admitted bytes.
    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// The control connection: framing, replies and the frames still queued for their caller.
pub(super) struct Control {
    /// The socket carrying commands and replies.
    stream: TcpStream,
    /// Streaming UTF-8 decoding of the reply bytes.
    decoder: maestro_text::Utf8Stream,
    /// Reply framing across reads.
    frames: Frames,
    /// Replies already framed but not yet consumed.
    replies: VecDeque<Reply>,
    /// The first terminal reason this session stopped answering commands.
    closing: Option<String>,
}

impl Control {
    /// Connect the control socket within one inactivity window.
    async fn connect(host: &str, port: u16) -> Result<Self, Failure> {
        let stream = match tokio::time::timeout(INACTIVITY, TcpStream::connect((host, port))).await
        {
            Ok(Ok(stream)) => stream,
            Ok(Err(error)) => {
                return Err(io::Error::other(format!("{error} (control socket)")).into());
            }
            Err(_) => return Err(Box::new(Silent(Socket::Control))),
        };
        Ok(Self {
            stream,
            decoder: maestro_text::Utf8Stream::default(),
            frames: Frames::default(),
            replies: VecDeque::new(),
            closing: None,
        })
    }

    /// The address of the peer, which an extended passive transfer reuses.
    fn peer(&self) -> Result<IpAddr, Failure> {
        self.stream
            .peer_addr()
            .map(|peer| peer.ip())
            .map_err(|error| {
                io::Error::other(format!(
                    "Control socket is disconnected, can't get remote address: {error}"
                ))
                .into()
            })
    }

    /// Send one command, framed with CRLF, on the one path every command takes.
    async fn send(&mut self, command: &str) -> Result<(), Failure> {
        if let Some(reason) = &self.closing {
            return Err(closed(reason));
        }
        if command
            .bytes()
            .any(|byte| matches!(byte, b'\r' | b'\n' | 0))
        {
            return Err(io::Error::other("Invalid command: Contains control characters.").into());
        }
        let mut framed = String::with_capacity(command.len() + 2);
        framed.push_str(command);
        framed.push_str("\r\n");
        let bytes = framed.as_bytes();
        let mut written = 0;
        while written < bytes.len() {
            match tokio::time::timeout(INACTIVITY, self.stream.write(&bytes[written..])).await {
                Ok(Ok(0)) => {
                    return Err(self.fail(io::Error::other(Self::UNEXPECTED_CLOSE).into()));
                }
                Ok(Ok(count)) => written += count,
                Ok(Err(error)) => return Err(self.fail(error.into())),
                Err(_) => return Err(self.fail(Box::new(Silent(Socket::Control)))),
            }
        }
        Ok(())
    }

    /// Wait for the next complete reply.
    pub(super) async fn reply(&mut self, deadline: Deadline) -> Result<Reply, Failure> {
        if let Some(reason) = &self.closing {
            return Err(closed(reason));
        }
        loop {
            if let Some(reply) = self.replies.pop_front() {
                return Ok(reply);
            }
            if let Err(failure) = self.fill(deadline).await {
                return Err(self.fail(failure));
            }
        }
    }

    /// Read once from the socket and frame whatever replies it completed.
    async fn fill(&mut self, deadline: Deadline) -> Result<(), Failure> {
        let mut chunk = [0u8; 4096];
        let read = match deadline {
            Deadline::Inactivity => {
                match tokio::time::timeout(INACTIVITY, self.stream.read(&mut chunk)).await {
                    Ok(Ok(read)) => read,
                    Ok(Err(error)) => return Err(error.into()),
                    Err(_) => return Err(Box::new(Silent(Socket::Control))),
                }
            }
            Deadline::Open => self.stream.read(&mut chunk).await?,
        };
        if read == 0 {
            return Err(io::Error::other(Self::UNEXPECTED_CLOSE).into());
        }
        let text = self.decoder.push(&chunk[..read]);
        for reply in self.frames.push(&text)? {
            self.replies.push_back(reply);
        }
        Ok(())
    }

    /// Record the first reason this session stopped, and report it.
    fn fail(&mut self, failure: Failure) -> Failure {
        if self.closing.is_none() {
            self.closing = Some(failure.to_string());
        }
        failure
    }

    /// Send the goodbye without waiting for its reply, then drop the socket.
    fn quit(self) {
        // shortcut: a control socket whose send buffer is full loses the goodbye, whose reply the
        // reference ignores as well.
        self.stream.try_write(b"QUIT\r\n").ok();
    }

    /// The message the reference reports for a peer that closed without a reply.
    const UNEXPECTED_CLOSE: &'static str =
        "Server sent FIN packet unexpectedly, closing connection.";
}

/// Both owned connections of one FTP session.
pub(super) struct FtpContext {
    /// The control connection.
    control: Control,
    /// The data connection of the operation in progress.
    data: Option<Data<TcpStream>>,
}

impl FtpContext {
    /// Connect the control socket of a session.
    ///
    /// # Errors
    /// Returns a connection or connect-timeout failure.
    pub(super) async fn open(host: &str, port: u16) -> Result<Self, Failure> {
        Control::connect(host, port).await.map(|control| Self {
            control,
            data: None,
        })
    }

    /// Read the greeting, which must announce a positive completion.
    ///
    /// # Errors
    /// Returns the server's refusal, or a transport failure.
    pub(super) async fn greeting(&mut self) -> Result<(), Failure> {
        let reply = self.control.reply(Deadline::Inactivity).await?;
        if reply.completed() {
            Ok(())
        } else {
            Err(Box::new(FtpError::of(reply)))
        }
    }

    /// Send one command and report a refusal as a failure.
    ///
    /// # Errors
    /// Returns the server's refusal or a transport failure.
    pub(super) async fn request(&mut self, command: &str) -> Result<Reply, Failure> {
        let reply = self.request_ignoring_error(command).await?;
        if reply.refused() {
            return Err(Box::new(FtpError::of(reply)));
        }
        Ok(reply)
    }

    /// Send one command and answer its reply even when the server refuses it.
    ///
    /// # Errors
    /// Returns a transport failure; a refusal is that command's answer.
    pub(super) async fn request_ignoring_error(&mut self, command: &str) -> Result<Reply, Failure> {
        self.control.send(command).await?;
        self.control.reply(Deadline::Inactivity).await
    }

    /// Submit one command whose replies a transfer settles.
    ///
    /// # Errors
    /// Returns a transport failure.
    pub(super) async fn send(&mut self, command: &str) -> Result<(), Failure> {
        self.control.send(command).await
    }

    /// The address of the control peer.
    ///
    /// # Errors
    /// Returns a failure when the peer address is unavailable.
    pub(super) fn peer(&self) -> Result<IpAddr, Failure> {
        self.control.peer()
    }

    /// Install the data connection of the operation in progress.
    pub(super) fn install(&mut self, data: Data<TcpStream>) {
        self.data = Some(data);
    }

    /// The control connection and the installed data connection, borrowed separately.
    pub(super) fn parts(&mut self) -> (&mut Control, Option<&mut Data<TcpStream>>) {
        (&mut self.control, self.data.as_mut())
    }

    /// Close both connections and report `failure` as the terminal reason.
    pub(super) fn fail(&mut self, failure: Failure) -> Failure {
        self.data = None;
        self.control.fail(failure)
    }

    /// Drop the data connection of a finished operation.
    pub(super) fn release(&mut self) {
        self.data = None;
    }

    /// Close the write side of the installed data connection so a waiting peer completes.
    pub(super) async fn shutdown_data(&mut self) {
        if let Some(data) = self.data.as_mut() {
            data.shutdown().await;
        }
    }

    /// Say goodbye and release both owned connections.
    pub(super) fn close(mut self) {
        self.data = None;
        self.control.quit();
    }
}
