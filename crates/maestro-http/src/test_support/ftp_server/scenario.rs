//! What a scripted FTP server answers: the scenario of one control connection.

use std::io::Write;
use std::net::Ipv4Addr;
use std::sync::mpsc;

use super::{Observed, Wire};

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
    #[must_use]
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
    pub(super) fn send(&self, control: &mut Wire, observed: &Observed) -> std::io::Result<()> {
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
    pub(super) fn host(&self, command: &str) -> Option<Ipv4Addr> {
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
    pub(super) fn answers(self, command: &str) -> bool {
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
    pub(super) fn gate(&self, command: &str) {
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
#[must_use]
pub fn machine(name: &str, modified: Option<&str>) -> Vec<String> {
    let facts = modified.map_or_else(
        || format!("type=file;size=4; {name}"),
        |stamp| format!("type=file;size=4;modify={stamp}; {name}"),
    );
    vec![facts]
}

/// The prefix that marks a progress marker in the command log.
pub const STAGE: &str = "\u{1}";
