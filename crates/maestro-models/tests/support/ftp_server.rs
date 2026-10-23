//! Scripted FTP server peer: a control connection answered from a scenario, with a passive data
//! connection for each listing and transfer.

#![cfg(not(target_arch = "wasm32"))]
// Reply shapes and observations only the library tests build are gated on `cfg(test)`, so the
// integration binary compiles only the peer it reads.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

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
#[derive(Clone)]
pub struct Transfer {
    /// Bytes sent on the data connection.
    pub body: Vec<u8>,
    /// The reply that refuses the command, instead of a transfer.
    pub refusal: Option<&'static str>,
    /// The reply sent after the bytes, or none when the data connection stays open.
    pub completion: Option<&'static str>,
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
        }
    }
}

/// How the server answers the passive-mode commands.
#[derive(Clone)]
pub struct Passive {
    /// Whether `EPSV` is answered.
    pub extended: bool,
    /// The data host `PASV` advertises, when `PASV` is answered.
    pub standard: Option<Ipv4Addr>,
}

impl Passive {
    /// A server that answers both commands on its control connection's address.
    pub const BOTH: Self = Self {
        extended: true,
        standard: Some(Ipv4Addr::LOCALHOST),
    };

    /// The data host this script binds for the command, when it answers it.
    fn host(&self, command: &str) -> Option<Ipv4Addr> {
        match command {
            "EPSV" if self.extended => Some(Ipv4Addr::LOCALHOST),
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
}

impl Silence {
    /// A server that answers every command.
    pub const EVERY: Self = Self {
        modification: true,
        quit: true,
    };

    /// Whether the server answers this command.
    fn answers(self, command: &str) -> bool {
        (self.modification || !command.starts_with("MDTM")) && (self.quit || command != "QUIT")
    }
}

/// One scripted server behaviour.
#[derive(Clone)]
pub struct Scenario {
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
}

impl Default for Scenario {
    fn default() -> Self {
        Self {
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
    commands: Arc<Mutex<Vec<String>>>,
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
        self.commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Stop accepting connections, wait until every admitted session ended, and report what the
    /// server observed about its connections.
    ///
    /// # Errors
    /// Returns stop, worker or join failures after waiting for admitted sessions.
    pub fn finish(mut self) -> TestResult<Closed> {
        self.stop.store(true, Ordering::SeqCst);
        match TcpStream::connect(self.address) {
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {}
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
            worker.join().ok();
        }
    }
}

/// Recorded observations of one admitted session.
struct Observed {
    commands: Arc<Mutex<Vec<String>>>,
    control_closed: Arc<AtomicBool>,
    data_closed: Arc<AtomicBool>,
}

impl Observed {
    /// Record one command before the server answers it.
    fn command(&self, command: &str) {
        self.commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(command.to_owned());
    }
}

/// Answer one admitted control connection from `scenario`.
fn session(
    mut control: TcpStream,
    scenario: &Scenario,
    observed: &Observed,
    stop: &AtomicBool,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(control.try_clone()?);
    reply(&mut control, "220 scripted ftp ready")?;
    let mut data: Option<TcpStream> = None;
    loop {
        let Some(command) = read_command(&mut reader)? else {
            observed.control_closed.store(true, Ordering::SeqCst);
            return Ok(());
        };
        observed.command(&command);
        if !scenario.silence.answers(&command) {
            // A command the script never answers is a stalled control connection.
            continue;
        }
        match command.split(' ').next().unwrap_or_default() {
            "QUIT" => {
                reply(&mut control, "221 bye")?;
                // The client closes the control connection next, which the next read observes.
            }
            "PASV" | "EPSV" => match passive(&command, &mut control, &scenario.passive)? {
                Some(listener) => match accept_data(&listener, stop)? {
                    Some(stream) => data = Some(stream),
                    // A stopped peer leaves the data connection unopened and ends the session.
                    None => return Ok(()),
                },
                None => data = None,
            },
            "USER" => reply(&mut control, "331 need password")?,
            "PASS" if scenario.login => reply(&mut control, "230 logged in")?,
            "PASS" => reply(&mut control, "530 login incorrect")?,
            "FEAT" => features(&mut control, &scenario.features)?,
            "MDTM" => modification(&mut control, &scenario.modification)?,
            "MLSD" | "LIST" => listed(&mut control, &scenario.listing, &mut data, observed)?,
            "RETR" => transferred(&mut control, &scenario.transfer, &mut data, observed)?,
            _ => reply(&mut control, "200 ok")?,
        }
    }
}

/// Answer `PASV` and `EPSV` with a fresh listener, which is returned for its data connection.
fn passive(
    command: &str,
    control: &mut TcpStream,
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
    match command {
        "PASV" => {
            let [a, b, c, d] = host.octets();
            reply(
                control,
                &format!(
                    "227 Entering Passive Mode ({a},{b},{c},{d},{},{}).",
                    port / 256,
                    port % 256
                ),
            )?;
        }
        _ => reply(
            control,
            &format!("229 Entering Extended Passive Mode (|||{port}|)"),
        )?,
    }
    Ok(Some(listener))
}

/// Accept the data connection, or nothing when the peer stops before it opens.
fn accept_data(listener: &TcpListener, stop: &AtomicBool) -> std::io::Result<Option<TcpStream>> {
    listener.set_nonblocking(true)?;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                return Ok(Some(stream));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if stop.load(Ordering::SeqCst) {
                    return Ok(None);
                }
                std::thread::yield_now();
            }
            Err(error) => return Err(error),
        }
    }
}

/// Answer `FEAT` with its scripted reply.
fn features(control: &mut TcpStream, announced: &Features) -> std::io::Result<()> {
    write!(control, "{}\r\n", announced.0)?;
    control.flush()
}

/// Answer `MDTM`.
fn modification(control: &mut TcpStream, announced: &Modification) -> std::io::Result<()> {
    match announced {
        Modification::Instant(stamp) => reply(control, &format!("213 {stamp}")),
        Modification::Reply(line) => reply(control, line),
    }
}

/// Answer the command with the script's refusal, when it has one.
fn refused(
    control: &mut TcpStream,
    announced: Option<&'static str>,
    data: &mut Option<TcpStream>,
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
    control: &mut TcpStream,
    announced: &Listing,
    data: &mut Option<TcpStream>,
    observed: &Observed,
) -> std::io::Result<()> {
    if refused(control, announced.refusal, data)? {
        return Ok(());
    }
    let lines = announced.lines.join("\r\n") + "\r\n";
    reply(control, "150 here comes the listing")?;
    let stream = data.take().ok_or_else(no_data)?;
    send_body(stream, lines.as_bytes(), true, observed)?;
    reply(control, "226 listing done")
}

/// Answer `RETR`.
fn transferred(
    control: &mut TcpStream,
    announced: &Transfer,
    data: &mut Option<TcpStream>,
    observed: &Observed,
) -> std::io::Result<()> {
    if refused(control, announced.refusal, data)? {
        return Ok(());
    }
    reply(control, "150 opening data connection")?;
    let stream = data.take().ok_or_else(no_data)?;
    // A transfer without a completion reply keeps the data connection open, so the client's read
    // never sees its end.
    send_body(
        stream,
        &announced.body,
        announced.completion.is_some(),
        observed,
    )?;
    match announced.completion {
        Some(completion) => reply(control, completion),
        None => Ok(()),
    }
}

/// Send `body`, then wait for the client to close the data connection before completing.
fn send_body(
    mut stream: TcpStream,
    body: &[u8],
    complete: bool,
    observed: &Observed,
) -> std::io::Result<()> {
    let outcome = transfer_body(&mut stream, body, complete);
    // A client that closed the connection first ends the dialogue just as surely as one that
    // read every byte, so the observation covers both shapes.
    observed.data_closed.store(true, Ordering::SeqCst);
    outcome
}

/// Write the body, close the write side of a completed transfer, then wait for the client's close.
fn transfer_body(stream: &mut TcpStream, body: &[u8], complete: bool) -> std::io::Result<()> {
    stream.write_all(body)?;
    // A client reading to the end of the file needs the write side closed; one held mid-transfer
    // never sees its end.
    if complete {
        stream.shutdown(std::net::Shutdown::Write)?;
    }
    let mut exhausted = Vec::new();
    stream.read_to_end(&mut exhausted)?;
    Ok(())
}

/// A transfer command without a data connection is a scripted-peer mistake.
fn no_data() -> std::io::Error {
    std::io::Error::other("no passive data connection")
}

/// Write one reply line.
fn reply(control: &mut TcpStream, line: &str) -> std::io::Result<()> {
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
        if let Err(error) = session(control, script, observed, stop) {
            // A client that went away mid-reply is a completed session, not a peer failure.
            if !matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
            ) {
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
    let commands = Arc::new(Mutex::new(Vec::new()));
    let control_closed = Arc::new(AtomicBool::new(false));
    let data_closed = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let observed = Observed {
        commands: Arc::clone(&commands),
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
        control_closed,
        data_closed,
        stop,
        worker: Some(worker),
    })
}
