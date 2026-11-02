//! Scripted FTP server peer: a control connection answered from a scenario, with a passive data
//! connection for each listing and transfer.

#![cfg(not(target_arch = "wasm32"))]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;

use tokio::sync::watch;

/// Fallible test body result.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// How the server answers `FEAT`: the reply it sends.
#[derive(Clone)]
pub struct Features(pub &'static str);

impl Features {
    /// A multiline reply that advertises a machine listing.
    pub const MACHINE: Self = Self("211-Features:\r\n MLST\r\n MDTM\r\n211 End");
}

/// How the server answers `MDTM`.
#[derive(Clone)]
pub enum Modification {
    /// A `213` reply carrying this timestamp.
    Instant(&'static str),
    /// This reply line, sent verbatim.
    Reply(&'static str),
}

impl Modification {
    /// The reply of a server that does not have the requested path.
    pub const MISSING: Self = Self::Reply("550 file unavailable");
}

/// What the server answers a listing command with.
#[derive(Clone)]
pub struct Listing {
    /// Lines sent on the data connection.
    pub lines: Vec<String>,
    /// The reply that refuses the command, instead of a listing.
    pub refusal: Option<&'static str>,
}

/// What the server sends for the requested file.
pub struct Transfer {
    /// Bytes sent on the data connection.
    pub body: Vec<u8>,
    /// The reply that refuses the command, instead of a transfer.
    pub refusal: Option<&'static str>,
    /// The reply sent after the bytes, or none when no completion follows.
    pub completion: Option<&'static str>,
    /// Send the completion before closing the data connection.
    pub completion_first: bool,
    /// Keep the data connection open after the bytes, without waiting for the client.
    pub keep_open: bool,
    /// Close the write side only after the test releases it.
    pub close_after: Option<mpsc::Receiver<()>>,
    /// Send the body in these pieces, pausing for a release between them.
    pub dribble: Option<Dribble>,
}

impl Transfer {
    /// The reply that completes a transfer which sent every byte.
    pub const COMPLETE: &'static str = "226 transfer complete";

    /// A transfer that sends these bytes and completes.
    pub fn body(body: Vec<u8>) -> Self {
        Self {
            body,
            refusal: None,
            completion: Some(Self::COMPLETE),
            completion_first: false,
            keep_open: false,
            close_after: None,
            dribble: None,
        }
    }
}

/// Bytes the peer sends in pieces, waiting for the test after each piece.
pub struct Dribble {
    /// The pieces, sent in order as raw bytes.
    pub parts: &'static [&'static [u8]],
    /// One release after each piece but the last.
    pub releases: mpsc::Receiver<()>,
}

impl Dribble {
    /// Send every piece, reporting each one to the test.
    fn send(&self, control: &mut Wire, observed: &Observed) -> std::io::Result<()> {
        for (index, part) in self.parts.iter().enumerate() {
            control.write_all(part)?;
            control.flush()?;
            observed.stage(&format!("{STAGE}piece-{index}"));
            if index + 1 < self.parts.len() && self.releases.recv().is_err() {
                return Ok(());
            }
        }
        Ok(())
    }
}

/// How the server answers the passive-mode commands.
#[derive(Clone)]
pub struct Passive {
    /// The character an `EPSV` reply repeats three times around its port, when `EPSV` is answered.
    pub extended: Option<char>,
    /// The data host `PASV` advertises, when `PASV` is answered.
    pub standard: Option<Ipv4Addr>,
    /// A port an `EPSV` reply announces instead of the one it listens on.
    pub extended_port: Option<u16>,
}

impl Passive {
    /// A server that answers both commands on its control connection's address.
    pub const BOTH: Self = Self {
        extended: Some('|'),
        standard: Some(Ipv4Addr::LOCALHOST),
        extended_port: None,
    };

    /// The data host this script binds for the command, when it answers it.
    fn host(&self, command: &str) -> Option<Ipv4Addr> {
        match command {
            "EPSV" if self.extended.is_some() => Some(Ipv4Addr::LOCALHOST),
            "PASV" => self.standard,
            _ => None,
        }
    }
}

/// Commands the server does not answer, which leaves the client waiting.
#[derive(Clone, Copy)]
pub struct Silence {
    /// Whether `MDTM` is answered.
    pub modification: bool,
    /// Whether `QUIT` is answered.
    pub quit: bool,
    /// Other commands whose replies the peer withholds.
    pub commands: &'static [&'static str],
}

impl Silence {
    /// A server that answers every command.
    pub const EVERY: Self = Self {
        modification: true,
        quit: true,
        commands: &[],
    };

    /// Whether the server answers this command.
    fn answers(self, command: &str) -> bool {
        (self.modification || !command.starts_with("MDTM"))
            && (self.quit || command != "QUIT")
            && !self
                .commands
                .iter()
                .any(|silent| command.starts_with(silent))
    }
}

/// One scripted server behaviour.
pub struct Scenario {
    /// The greeting sent before any command.
    pub greeting: &'static str,
    /// Whether `PASS` is accepted.
    pub login: bool,
    /// `FEAT` reply.
    pub features: Features,
    /// `MDTM` reply.
    pub modification: Modification,
    /// Listing content.
    pub listing: Listing,
    /// File content.
    pub transfer: Transfer,
    /// Passive-mode reply.
    pub passive: Passive,
    /// Commands the server does not answer.
    pub silence: Silence,
    /// Commands whose reply waits for the test to release it.
    pub held: Option<Held>,
    /// Replies the peer sends in pieces, with the commands they answer.
    pub replies: Option<(&'static [&'static str], Dribble)>,
    /// The completion reply of a transfer, sent in pieces.
    pub completion: Option<Dribble>,
    /// End the control connection's output right after the greeting.
    pub hang_up: bool,
}

/// Commands whose reply the peer holds until the test releases it.
pub struct Held {
    /// The commands whose reply waits for a release.
    pub commands: &'static [&'static str],
    /// One release per held reply, sent by the test.
    pub releases: mpsc::Receiver<()>,
}

impl Held {
    /// Wait for the test's release, when this command is one whose reply it holds.
    fn gate(&self, command: &str) {
        if self.commands.contains(&command) {
            self.releases.recv().ok();
        }
    }
}

impl Default for Scenario {
    fn default() -> Self {
        Self {
            greeting: "220 scripted ftp ready",
            login: true,
            features: Features::MACHINE,
            modification: Modification::Instant("20240101120000"),
            listing: Listing {
                lines: machine("script.pac", Some("20240101120000")),
                refusal: None,
            },
            transfer: Transfer::body(b"function FindProxyForURL(){return 'DIRECT'}".to_vec()),
            passive: Passive::BOTH,
            silence: Silence::EVERY,
            held: None,
            replies: None,
            completion: None,
            hang_up: false,
        }
    }
}

/// A machine-listing entry, with its modification stamp when the server reports one.
pub fn machine(name: &str, modified: Option<&str>) -> Vec<String> {
    let facts = modified.map_or_else(
        || format!("type=file;size=4; {name}"),
        |stamp| format!("type=file;size=4;modify={stamp}; {name}"),
    );
    vec![facts]
}

/// The prefix that marks a progress marker in the command log.
pub const STAGE: &str = "\u{1}";

/// What the server observed about its connections when it stopped.
pub struct Closed {
    /// Whether the client closed the control connection.
    pub control: bool,
    /// Whether the client closed a data connection.
    pub data: bool,
}

/// A running scripted server.
pub struct Peer {
    address: SocketAddr,
    commands: Arc<watch::Sender<Vec<String>>>,
    written: Arc<AtomicUsize>,
    control_closed: Arc<AtomicBool>,
    data_closed: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<std::io::Result<()>>>,
}

impl Peer {
    /// The loopback address the server listens on.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Commands the server has read so far.
    pub fn commands(&self) -> Vec<String> {
        self.commands.borrow().clone()
    }

    /// Wait until the server logged `count` lines starting with `line`.
    pub async fn reached(&self, line: &str, count: usize) {
        let mut log = self.commands.subscribe();
        // The peer holds the sender, so the log stays open while it is awaited.
        log.wait_for(|log| log.iter().filter(|seen| seen.starts_with(line)).count() >= count)
            .await
            .ok();
    }

    /// The bytes the server wrote to its connections so far.
    pub fn written(&self) -> usize {
        self.written.load(Ordering::SeqCst)
    }

    /// Stop accepting connections, wait until every admitted session ended, and report what the
    /// server observed about its connections.
    ///
    /// # Errors
    /// Returns stop, worker or join failures after waiting for admitted sessions.
    pub fn finish(mut self) -> TestResult<Closed> {
        self.stop.store(true, Ordering::SeqCst);
        match TcpStream::connect(self.address) {
            // The stopped listener can close before or during the wake-up handshake.
            Err(error)
                if error.kind() == std::io::ErrorKind::ConnectionRefused || vanished(&error) => {}
            other => other.map(drop)?,
        }
        let worker = self.worker.take().ok_or("server already finished")?;
        worker.join().map_err(|_| "server thread panicked")??;
        Ok(Closed {
            control: self.control_closed.load(Ordering::SeqCst),
            data: self.data_closed.load(Ordering::SeqCst),
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

/// Recorded observations of one admitted session.
struct Observed {
    commands: Arc<watch::Sender<Vec<String>>>,
    written: Arc<AtomicUsize>,
    control_closed: Arc<AtomicBool>,
    data_closed: Arc<AtomicBool>,
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
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let commands = Arc::new(watch::Sender::new(Vec::new()));
    let written = Arc::new(AtomicUsize::new(0));
    let control_closed = Arc::new(AtomicBool::new(false));
    let data_closed = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let observed = Observed {
        commands: Arc::clone(&commands),
        written: Arc::clone(&written),
        control_closed: Arc::clone(&control_closed),
        data_closed: Arc::clone(&data_closed),
    };
    let stopping = Arc::clone(&stop);
    let worker = std::thread::spawn(move || {
        accept_until_stopped(&listener, &stopping, &scenarios, &observed)
    });
    Ok(Peer {
        address,
        commands,
        written,
        control_closed,
        data_closed,
        stop,
        worker: Some(worker),
    })
}

/// Shutdown regressions for the scripted peer.
#[cfg(test)]
mod tests {
    use super::{Scenario, serve};

    /// Finishing without a client session tolerates the listener closing during wake-up.
    #[test]
    fn ftp_peer_finish_joins_an_idle_server() {
        for _ in 0..1_000 {
            let peer = serve(vec![Scenario::default()]).unwrap();
            let closed = peer
                .finish()
                .expect("the wake-up connection may race shutdown");
            assert!(!closed.control, "no client session was admitted");
            assert!(!closed.data, "no data connection was opened");
        }
    }
}
