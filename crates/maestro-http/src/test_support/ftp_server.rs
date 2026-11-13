//! Scripted FTP server peer: a control connection answered from a scenario, with a passive data
//! connection for each listing and transfer.

#![cfg(not(target_arch = "wasm32"))]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;

use tokio::sync::watch;

/// The scripted answers of one control connection.
mod scenario;

pub use scenario::{
    Dribble, Features, Held, Listing, Modification, Passive, STAGE, Scenario, Silence, Transfer,
    machine,
};

/// Fallible test body result.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// What the server observed about its connections when it stopped.
pub struct Closed {
    /// Whether the client closed the control connection.
    pub control: bool,
    /// Whether the client closed a data connection.
    pub data: bool,
}

/// A running scripted server.
pub struct Peer {
    /// The loopback address the server listens on.
    address: SocketAddr,
    /// The listening socket, shared with the worker and held until the peer is dropped, so the
    /// wake-up connection reaches it even when the worker already stopped.
    _listener: Arc<TcpListener>,
    /// Observations shared with the server worker.
    pub(super) observed: Arc<Observed>,
    /// Tells the worker to stop accepting connections.
    stop: Arc<AtomicBool>,
    /// The server worker, joined when the peer finishes.
    worker: Option<JoinHandle<std::io::Result<()>>>,
}

impl Peer {
    /// The loopback address the server listens on.
    #[must_use]
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Commands the server has read so far.
    #[must_use]
    pub fn commands(&self) -> Vec<String> {
        self.observed.commands.borrow().clone()
    }

    /// Stop accepting connections, wait until every admitted session ended, and report what the
    /// server observed about its connections.
    ///
    /// # Errors
    /// Returns stop, worker or join failures after waiting for admitted sessions.
    pub fn finish(mut self) -> TestResult<Closed> {
        self.stop.store(true, Ordering::SeqCst);
        TcpStream::connect(self.address).map(drop)?;
        let worker = self.worker.take().ok_or("server already finished")?;
        worker.join().map_err(|_| "server thread panicked")??;
        Ok(Closed {
            control: self.observed.control_closed.load(Ordering::SeqCst),
            data: self.observed.data_closed.load(Ordering::SeqCst),
        })
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            self.stop.store(true, Ordering::SeqCst);
            TcpStream::connect(self.address).map(drop).ok();
            // An unwinding test already failed, and this peer may be waiting on a connection the
            // failed test no longer closes.
            if !std::thread::panicking() {
                worker.join().ok();
            }
        }
    }
}

/// A connection of the server, which counts the bytes it writes.
struct Wire {
    /// The socket written to.
    stream: TcpStream,
    /// The bytes the server wrote to every connection.
    written: Arc<AtomicUsize>,
}

impl Wire {
    /// Count the writes to `stream` among the bytes `observed` reports.
    fn new(stream: TcpStream, observed: &Observed) -> Self {
        Self {
            stream,
            written: Arc::clone(&observed.written),
        }
    }
}

impl Write for Wire {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let count = self.stream.write(bytes)?;
        self.written.fetch_add(count, Ordering::SeqCst);
        Ok(count)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

/// What the server observed about its connections, shared with the peer that reports it.
pub(super) struct Observed {
    /// Commands the server read, as its log.
    pub(super) commands: watch::Sender<Vec<String>>,
    /// The bytes the server wrote to every connection.
    pub(super) written: Arc<AtomicUsize>,
    /// Whether the client closed the control connection.
    pub(super) control_closed: AtomicBool,
    /// Whether the client closed a data connection.
    pub(super) data_closed: AtomicBool,
}

impl Observed {
    /// Record one command before the server answers it.
    fn command(&self, command: &str) {
        self.commands
            .send_modify(|log| log.push(command.to_owned()));
    }

    /// Record one progress marker, which a test reads instead of a command.
    fn stage(&self, marker: &str) {
        self.command(marker);
    }
}

/// Answer one admitted control connection from `scenario`.
fn session(control: TcpStream, scenario: &Scenario, observed: &Observed) -> std::io::Result<()> {
    // A piece written while an earlier one awaits its acknowledgement would otherwise wait in
    // this side's send buffer after its progress marker, so the marker would not show the bytes
    // reached the client.
    control.set_nodelay(true)?;
    let mut reader = BufReader::new(control.try_clone()?);
    let mut control = Wire::new(control, observed);
    match scripted(scenario, "") {
        Some(pieces) => pieces.send(&mut control, observed)?,
        None => reply(&mut control, scenario.greeting)?,
    }
    if scenario.hang_up {
        return end_stream(&control.stream);
    }
    let mut data: Option<TcpListener> = None;
    loop {
        let Some(command) = read_command(&mut reader)? else {
            observed.control_closed.store(true, Ordering::SeqCst);
            return Ok(());
        };
        observed.command(&command);
        let verb = command.split(' ').next().unwrap_or_default();
        if let Some(held) = &scenario.held {
            held.gate(verb);
        }
        if !scenario.silence.answers(&command) {
            // A command the script never answers is a stalled control connection.
            continue;
        }
        if let Some(pieces) = scripted(scenario, verb) {
            pieces.send(&mut control, observed)?;
            continue;
        }
        match verb {
            "QUIT" => {
                reply(&mut control, "221 bye")?;
                // The client closes the control connection next, which the next read observes.
            }
            "PASV" | "EPSV" => data = passive(&command, &mut control, &scenario.passive)?,
            "USER" => reply(&mut control, "331 need password")?,
            "PASS" if scenario.login => reply(&mut control, "230 logged in")?,
            "PASS" => reply(&mut control, "530 login incorrect")?,
            "FEAT" => features(&mut control, &scenario.features)?,
            "MDTM" => modification(&mut control, &scenario.modification)?,
            "MLSD" | "LIST" => listed(&mut control, &scenario.listing, &mut data, observed)?,
            "RETR" => transferred(&mut control, scenario, &mut data, observed)?,
            _ => reply(&mut control, "200 ok")?,
        }
    }
}

/// Answer `PASV` and `EPSV` with a fresh listener, which is returned for its data connection.
fn passive(
    command: &str,
    control: &mut Wire,
    script: &Passive,
) -> std::io::Result<Option<TcpListener>> {
    if !matches!(command, "PASV" | "EPSV") {
        return Ok(None);
    }
    let Some(host) = script.host(command) else {
        // A mode this script does not offer is a command the server refuses.
        reply(control, "500 passive mode not implemented")?;
        return Ok(None);
    };
    let listener = TcpListener::bind((host, 0))?;
    let port = listener.local_addr()?.port();
    if command == "PASV" {
        let [a, b, c, d] = host.octets();
        reply(
            control,
            &format!(
                "227 Entering Passive Mode ({a},{b},{c},{d},{},{}).",
                port / 256,
                port % 256
            ),
        )?;
    } else {
        // A server on IBM i delimits with `!` and leaves out the usual sentence.
        let announced = script.extended_port.unwrap_or(port);
        if script.extended == Some('!') {
            reply(control, &format!("229 (!!!{announced}!)"))?;
        } else {
            reply(
                control,
                &format!("229 Entering Extended Passive Mode (|||{announced}|)"),
            )?;
        }
    }
    Ok(Some(listener))
}

/// Close the peer's write side and wait until the client closes the connection too.
///
/// A client that already closed its end leaves the socket unconnected or reset, which reports
/// the closed connection the peer observes rather than a peer failure.
fn close_data(stream: &mut Wire, observed: &Observed) -> std::io::Result<()> {
    // A client that closed the connection first ends the dialogue just as surely as one that
    // read every byte, so the observation covers both shapes.
    observed.data_closed.store(true, Ordering::SeqCst);
    end_stream(&stream.stream)
}

/// Close the write side of `stream` and read what the client still sends until it closes.
///
/// Closing with the client's bytes unread would answer them with a reset, and some hosts,
/// macOS among them, report that reset to the client before the end of stream it follows.
///
/// # Errors
/// Returns the write shutdown or read failure, except the broken pipe, reset, aborted and
/// not-connected errors of a peer that already closed, which end the stream normally.
fn end_stream(mut stream: &TcpStream) -> std::io::Result<()> {
    if let Err(error) = stream.shutdown(std::net::Shutdown::Write) {
        return if vanished(&error) { Ok(()) } else { Err(error) };
    }
    let mut exhausted = Vec::new();
    match stream.read_to_end(&mut exhausted) {
        Err(error) if vanished(&error) => Ok(()),
        other => other.map(drop),
    }
}

/// Whether the error reports a peer that closed its connection.
fn vanished(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
    )
}

/// Take the data connection the client opened after a passive reply.
fn accepted(data: &mut Option<TcpListener>, observed: &Observed) -> std::io::Result<Wire> {
    // The client connects before it sends the command, so the connection is already queued.
    let listener = data.take().ok_or_else(no_data)?;
    let (stream, _) = listener.accept()?;
    stream.set_nodelay(true)?;
    Ok(Wire::new(stream, observed))
}

/// Answer `FEAT` with its scripted reply.
fn features(control: &mut Wire, announced: &Features) -> std::io::Result<()> {
    write!(control, "{}\r\n", announced.0)?;
    control.flush()
}

/// Answer `MDTM`.
fn modification(control: &mut Wire, announced: &Modification) -> std::io::Result<()> {
    match announced {
        Modification::Instant(stamp) => reply(control, &format!("213 {stamp}")),
        Modification::Reply(line) => reply(control, line),
    }
}

/// Answer the command with the script's refusal, when it has one.
fn refused(
    control: &mut Wire,
    announced: Option<&'static str>,
    data: &mut Option<TcpListener>,
) -> std::io::Result<bool> {
    let Some(refusal) = announced else {
        return Ok(false);
    };
    *data = None;
    reply(control, refusal)?;
    Ok(true)
}

/// Answer a listing command, keeping the data connection observable until the client closes it.
fn listed(
    control: &mut Wire,
    announced: &Listing,
    data: &mut Option<TcpListener>,
    observed: &Observed,
) -> std::io::Result<()> {
    if refused(control, announced.refusal, data)? {
        return Ok(());
    }
    let lines = announced.lines.join("\r\n") + "\r\n";
    reply(control, "150 here comes the listing")?;
    let mut stream = accepted(data, observed)?;
    stream.write_all(lines.as_bytes())?;
    close_data(&mut stream, observed)?;
    reply(control, "226 listing done")
}

/// The pieces a scripted reply of `command` is sent in, when the test dribbles it.
fn scripted<'a>(scenario: &'a Scenario, command: &str) -> Option<&'a Dribble> {
    scenario
        .replies
        .as_ref()
        .filter(|(commands, _)| commands.contains(&command))
        .map(|(_, pieces)| pieces)
}

/// Answer `RETR`.
fn transferred(
    control: &mut Wire,
    scenario: &Scenario,
    data: &mut Option<TcpListener>,
    observed: &Observed,
) -> std::io::Result<()> {
    let announced = &scenario.transfer;
    if refused(control, announced.refusal, data)? {
        return Ok(());
    }
    reply(control, "150 opening data connection")?;
    let mut stream = accepted(data, observed)?;
    match &announced.dribble {
        Some(pieces) => pieces.send(&mut stream, observed)?,
        None => {
            if let Err(error) = stream.write_all(&announced.body) {
                if !vanished(&error) {
                    return Err(error);
                }
                observed.data_closed.store(true, Ordering::SeqCst);
                return Ok(());
            }
        }
    }
    if announced.completion_first {
        match (&scenario.completion, announced.completion) {
            (Some(pieces), _) => pieces.send(control, observed)?,
            (None, Some(completion)) => reply(control, completion)?,
            (None, None) => {}
        }
        observed.stage(&format!("{STAGE}completion-sent"));
        if let Some(release) = &announced.close_after {
            release.recv().ok();
        }
        close_data(&mut stream, observed)?;
        return Ok(());
    }
    if let Some(release) = &announced.close_after {
        release.recv().ok();
    }
    if announced.keep_open {
        // The client still holds the data connection open, so its read never sees an end.
        return match announced.completion {
            Some(completion) => reply(control, completion),
            None => Ok(()),
        };
    }
    // A client reading to the end of the file needs the write side closed, whether or not a
    // completion follows.
    close_data(&mut stream, observed)?;
    match (&scenario.completion, announced.completion) {
        (Some(pieces), _) => pieces.send(control, observed),
        (None, Some(completion)) => reply(control, completion),
        (None, None) => Ok(()),
    }
}

/// A transfer command without a data connection is a scripted-peer mistake.
fn no_data() -> std::io::Error {
    std::io::Error::other("no passive data connection")
}

/// Write one reply line.
fn reply(control: &mut Wire, line: &str) -> std::io::Result<()> {
    control.write_all(line.as_bytes())?;
    control.write_all(b"\r\n")?;
    control.flush()
}

/// Read one command line, or nothing when the client closed the connection.
fn read_command(reader: &mut BufReader<TcpStream>) -> std::io::Result<Option<String>> {
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim_end().to_owned()))
}

/// Accept control connections until the peer is finished.
fn accept_until_stopped(
    listener: &TcpListener,
    stop: &AtomicBool,
    scenarios: &[Scenario],
    observed: &Observed,
) -> std::io::Result<()> {
    let mut served = 0;
    while !stop.load(Ordering::SeqCst) {
        let Ok((control, _)) = listener.accept() else {
            return Ok(());
        };
        if stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        let last = scenarios.len().saturating_sub(1);
        let script = &scenarios[served.min(last)];
        served += 1;
        if let Err(error) = session(control, script, observed) {
            // A client that went away mid-reply is a completed session, not a peer failure;
            // Windows reports the closed connection as aborted rather than reset.
            if !vanished(&error) {
                return Err(error);
            }
            // A client that closed its connection took its control connection with it.
            observed.control_closed.store(true, Ordering::SeqCst);
        }
    }
    Ok(())
}

/// Start the scripted server on one loopback port.
///
/// # Errors
/// Returns bind or local-address lookup failures.
pub fn serve(scenarios: Vec<Scenario>) -> TestResult<Peer> {
    let listener = Arc::new(TcpListener::bind("127.0.0.1:0")?);
    let address = listener.local_addr()?;
    let observed = Arc::new(Observed {
        commands: watch::Sender::new(Vec::new()),
        written: Arc::new(AtomicUsize::new(0)),
        control_closed: AtomicBool::new(false),
        data_closed: AtomicBool::new(false),
    });
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = Arc::clone(&stop);
    let observing = Arc::clone(&observed);
    let accepting = Arc::clone(&listener);
    let worker = std::thread::spawn(move || {
        accept_until_stopped(&accepting, &stopping, &scenarios, &observing)
    });
    Ok(Peer {
        address,
        _listener: listener,
        observed,
        stop,
        worker: Some(worker),
    })
}

/// Shutdown regressions for the scripted peer.
#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::net::TcpStream;
    use std::sync::mpsc;

    use super::{Held, Scenario, serve};

    /// A peer whose worker already stopped still listens, so the wake-up connection of `finish`
    /// reaches it and the worker's own failure is what `finish` reports.
    #[test]
    fn ftp_peer_keeps_listening_after_its_worker_stops() {
        // The worker never takes from this rendezvous, so a send returns only once the stopped
        // worker dropped its script.
        let (script, releases) = mpsc::sync_channel(0);
        let peer = serve(vec![Scenario {
            held: Some(Held {
                commands: &[],
                releases,
            }),
            ..Scenario::default()
        }])
        .unwrap();
        let mut control = TcpStream::connect(peer.address()).unwrap();

        // A transfer without a passive data connection is a scripted-peer mistake, which stops
        // the worker.
        control.write_all(b"RETR script.pac\r\n").unwrap();
        assert!(script.send(()).is_err(), "the worker stopped");

        TcpStream::connect(peer.address()).expect("the peer still listens");
        let failure = peer.finish().err().map(|error| error.to_string());
        assert_eq!(failure.as_deref(), Some("no passive data connection"));
    }
}
