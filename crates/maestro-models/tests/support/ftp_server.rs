//! Scripted FTP server peer: a control connection answered from a scenario, with a passive data
//! connection for each listing and transfer.

#![cfg(not(target_arch = "wasm32"))]
// The library and the integration binary share this peer; each builds only the reply shapes and
// helpers its own scenarios need, so the other consumer's items are unreachable here.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

/// Fallible test body result.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// How the server answers `FEAT`.
#[derive(Clone)]
pub enum Features {
    /// A multiline reply that advertises a machine listing.
    Machine,
    /// A multiline reply with no machine listing.
    Text,
    /// The single-line reply of a server without optional features.
    Absent,
    /// A refused command.
    Refused,
}

/// How the server answers `MDTM`.
#[derive(Clone)]
pub enum Modification {
    /// A `213` reply carrying this timestamp.
    Instant(&'static str),
    /// The path is unavailable.
    Missing,
    /// The command is not implemented.
    Unsupported,
}

/// What the server answers a listing command with.
#[derive(Clone)]
pub enum Listing {
    /// Lines sent on the data connection.
    Lines(Vec<String>),
    /// A refused command.
    Refused,
}

/// What the server sends for the requested file.
#[derive(Clone)]
pub enum Transfer {
    /// The complete file.
    Body(Vec<u8>),
    /// Half of the file, then a transfer that broke off.
    Interrupted(Vec<u8>),
    /// A refused command.
    Refused,
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
}

impl Default for Scenario {
    fn default() -> Self {
        Self {
            login: true,
            features: Features::Machine,
            modification: Modification::Instant("20240101120000"),
            listing: Listing::Lines(machine("script.pac", Some("20240101120000"))),
            transfer: Transfer::Body(b"function FindProxyForURL(){return 'DIRECT'}".to_vec()),
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

/// A Unix text-listing entry, which carries no comparable modification stamp.
pub fn unix(name: &str) -> Vec<String> {
    vec![format!("-rw-r--r-- 1 user group 4 Nov 5 13:46 {name}")]
}

/// A DOS text-listing entry, which carries no comparable modification stamp.
pub fn dos(name: &str) -> Vec<String> {
    vec![format!("11-05-20  01:46PM                  4 {name}")]
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
    events: Option<tokio::sync::mpsc::UnboundedReceiver<String>>,
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

    /// Commands as they arrive, for tests that must act during a dialogue.
    pub fn events(&mut self) -> Option<tokio::sync::mpsc::UnboundedReceiver<String>> {
        self.events.take()
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
    events: tokio::sync::mpsc::UnboundedSender<String>,
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
        let _ = self.events.send(command.to_owned());
    }
}

/// Answer one admitted control connection from `scenario`.
fn session(
    mut control: TcpStream,
    scenario: &Scenario,
    observed: &Observed,
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
        if command == "QUIT" {
            reply(&mut control, "221 bye")?;
            // The client closes the control connection next, which the next read observes.
            continue;
        }
        if let Some(listener) = passive(&command, &mut control)? {
            data = Some(listener.accept()?.0);
            continue;
        }
        match command.split(' ').next().unwrap_or_default() {
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
fn passive(command: &str, control: &mut TcpStream) -> std::io::Result<Option<TcpListener>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    match command {
        "PASV" => reply(
            control,
            &format!(
                "227 Entering Passive Mode (127,0,0,1,{},{}).",
                port / 256,
                port % 256
            ),
        )?,
        "EPSV" => reply(
            control,
            &format!("229 Entering Extended Passive Mode (|||{port}|)"),
        )?,
        _ => return Ok(None),
    }
    Ok(Some(listener))
}

/// Answer `FEAT` in each reply shape a server may use.
fn features(control: &mut TcpStream, announced: &Features) -> std::io::Result<()> {
    match announced {
        Features::Machine => {
            write!(control, "211-Features:\r\n MLST\r\n MDTM\r\n211 End\r\n")?;
            control.flush()
        }
        Features::Text => {
            write!(control, "211-Features:\r\n MDTM\r\n211 End\r\n")?;
            control.flush()
        }
        Features::Absent => reply(control, "211 no features"),
        Features::Refused => reply(control, "500 unknown command"),
    }
}

/// Answer `MDTM`.
fn modification(control: &mut TcpStream, announced: &Modification) -> std::io::Result<()> {
    match announced {
        Modification::Instant(stamp) => reply(control, &format!("213 {stamp}")),
        Modification::Missing => reply(control, "550 file unavailable"),
        Modification::Unsupported => reply(control, "500 unknown command"),
    }
}

/// Answer a listing command, keeping the data connection observable until the client closes it.
fn listed(
    control: &mut TcpStream,
    announced: &Listing,
    data: &mut Option<TcpStream>,
    observed: &Observed,
) -> std::io::Result<()> {
    let lines = match announced {
        Listing::Refused => {
            *data = None;
            return reply(control, "550 listing refused");
        }
        Listing::Lines(lines) => lines.join("\r\n") + "\r\n",
    };
    reply(control, "150 here comes the listing")?;
    let stream = data.take().ok_or_else(no_data)?;
    send_body(stream, lines.as_bytes(), observed)?;
    reply(control, "226 listing done")
}

/// Answer `RETR`.
fn transferred(
    control: &mut TcpStream,
    announced: &Transfer,
    data: &mut Option<TcpStream>,
    observed: &Observed,
) -> std::io::Result<()> {
    let (body, completion) = match announced {
        Transfer::Body(body) => (body.as_slice(), "226 transfer complete"),
        Transfer::Interrupted(body) => (&body[..body.len() / 2], "426 transfer aborted"),
        Transfer::Refused => {
            *data = None;
            return reply(control, "550 no such file");
        }
    };
    reply(control, "150 opening data connection")?;
    let stream = data.take().ok_or_else(no_data)?;
    send_body(stream, body, observed)?;
    reply(control, completion)
}

/// Send `body`, then wait for the client to close the data connection before completing.
fn send_body(mut stream: TcpStream, body: &[u8], observed: &Observed) -> std::io::Result<()> {
    let outcome = transfer_body(&mut stream, body);
    // A client that closed the connection first ends the dialogue just as surely as one that
    // read every byte, so the observation covers both shapes.
    observed.data_closed.store(true, Ordering::SeqCst);
    outcome
}

/// Write the body, then read until the client closes the data connection.
fn transfer_body(stream: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
    stream.write_all(body)?;
    stream.shutdown(std::net::Shutdown::Write)?;
    // The client closes the data connection before it reads the completion reply.
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
        if let Err(error) = session(control, script, observed) {
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
    let (events, receiver) = tokio::sync::mpsc::unbounded_channel();
    let observed = Observed {
        commands: Arc::clone(&commands),
        events,
        control_closed: Arc::clone(&control_closed),
        data_closed: Arc::clone(&data_closed),
    };
    let stopping = Arc::clone(&stop);
    let worker = std::thread::spawn(move || {
        accept_until_stopped(&listener, &stopping, &scenarios, &observed)
    });
    Ok(Peer {
        events: Some(receiver),
        address,
        commands,
        control_closed,
        data_closed,
        stop,
        worker: Some(worker),
    })
}
