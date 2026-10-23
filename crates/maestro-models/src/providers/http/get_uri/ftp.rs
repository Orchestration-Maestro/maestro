//! FTP script sources: login, modification check, listing fallback and transfer.

use std::future::Future;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;

use super::{Cache, Failure, Source, listing, notfound::NotFound, notmodified::NotModified};
use maestro_path::posix;
use suppaftp::Status;
use suppaftp::tokio::AsyncFtpStream;
use suppaftp::types::{FtpError, Response};

/// The directory facts the reference client asks for before a machine listing.
const LISTING_FACTS: &str = concat!(
    "type;size;modify;unique;",
    "unix.mode;unix.owner;unix.group;",
    "unix.ownername;unix.groupname;"
);

/// The silence a control or data connection tolerates before its wait counts as stalled.
pub(super) const INACTIVITY: Duration = Duration::from_secs(30);

/// The largest directory listing the reference client accepts, in bytes.
const LISTING_LIMIT: usize = 40 * 1024 * 1024;

/// Load one FTP script, closing the connection on every exit path.
pub(super) async fn load(uri: &url::Url, cache: Option<&Cache>) -> Result<Source, Failure> {
    let path = super::super::proxy_agent::decode(uri.path())?;
    let mut session = Session::open(uri).await?;
    let result = session.exchange(&path, cache).await;
    session.close().await;
    result
}

/// The port to reach: an explicit `0` selects the standard port, as in the reference.
pub(super) fn port(uri: &url::Url) -> u16 {
    uri.port().filter(|port| *port != 0).unwrap_or(21)
}

/// The millisecond instant of a `YYYYMMDDHHMMSS[.sss]` stamp.
pub(super) fn instant_millis(stamp: &str) -> Option<i64> {
    let (seconds, fraction) = stamp.split_once('.').unwrap_or((stamp, "0"));
    let digits = |start: usize, length: usize| seconds.get(start..start + length);
    let date = chrono::NaiveDate::from_ymd_opt(
        digits(0, 4)?.parse().ok()?,
        digits(4, 2)?.parse().ok()?,
        digits(6, 2)?.parse().ok()?,
    )?;
    let time = chrono::NaiveTime::from_hms_opt(
        digits(8, 2)?.parse().ok()?,
        digits(10, 2)?.parse().ok()?,
        digits(12, 2)?.parse().ok()?,
    )?;
    let millis: u32 = fraction.chars().take(3).collect::<String>().parse().ok()?;
    Some(date.and_time(time).and_utc().timestamp_millis() + i64::from(millis))
}

/// An open FTP session: its control connection, the passive strategy it accepted and whether that
/// connection stalled.
struct Session {
    /// The control connection.
    stream: AsyncFtpStream,
    /// Whether the server advertises machine listings.
    machine: bool,
    /// The passive strategy that worked, once one has.
    strategy: Option<Strategy>,
    /// The socket whose inactivity ended the session, which no later wait can use.
    stalled: Option<Connection>,
}

impl Session {
    /// Connect, log in and select the transfer settings the reference client uses.
    async fn open(uri: &url::Url) -> Result<Self, Failure> {
        let host = uri
            .host_str()
            .filter(|host| !host.is_empty())
            .unwrap_or("localhost");
        let stream = bounded(
            Connection::Control,
            AsyncFtpStream::connect((host, port(uri))),
        )
        .await?;
        let mut session = Self {
            stream,
            machine: false,
            strategy: None,
            stalled: None,
        };
        let _ = session.command("OPTS UTF8 ON", &[Status::CommandOk]).await;
        let user = credential(
            &super::super::proxy_agent::decode(uri.username())?,
            "anonymous",
        );
        let password = credential(
            &super::super::proxy_agent::decode(uri.password().unwrap_or_default())?,
            "guest",
        );
        session.login(&user, &password).await?;
        session.machine = session.advertised().await;
        session.command("TYPE I", &[Status::CommandOk]).await?;
        let _ = session.command("STRU F", &[Status::CommandOk]).await;
        let _ = session.command("OPTS UTF8 ON", &[Status::CommandOk]).await;
        if session.machine {
            let _ = session
                .command(format!("OPTS MLST {LISTING_FACTS}"), &[Status::CommandOk])
                .await;
        }
        Ok(session)
    }

    /// Log in, answering a password request as the reference does.
    async fn login(&mut self, user: &str, password: &str) -> Result<(), Failure> {
        let asked = self
            .command(
                format!("USER {user}"),
                &[Status::LoggedIn, Status::NeedPassword],
            )
            .await?;
        if asked.status == Status::NeedPassword {
            self.command(format!("PASS {password}"), &[Status::LoggedIn])
                .await?;
        }
        Ok(())
    }

    /// Send one command and read its reply, within one inactivity window.
    async fn command(
        &mut self,
        command: impl ToString,
        accepted: &[Status],
    ) -> Result<Response, Failure> {
        self.usable()?;
        let result = bounded(
            Connection::Control,
            self.stream.custom_command(command, accepted),
        )
        .await;
        self.settled(result)
    }

    /// Read one reply, within one inactivity window.
    async fn reply(&mut self, accepted: &[Status]) -> Result<Response, Failure> {
        self.usable()?;
        let result = bounded(Connection::Control, self.stream.read_response_in(accepted)).await;
        self.settled(result)
    }

    /// Refuse a wait once the control connection stalled, as the reference's closed client does.
    fn usable(&self) -> Result<(), Failure> {
        match self.stalled {
            Some(connection) => Err(Box::new(Stalled(connection))),
            None => Ok(()),
        }
    }

    /// Remember a control stall, after which the session can answer no further command.
    fn settled<T>(&mut self, result: Result<T, Failure>) -> Result<T, Failure> {
        if result.as_ref().err().is_some_and(stalling) {
            self.stalled = Some(Connection::Control);
        }
        result
    }

    /// Compare the file's modification instant with the retained one, then transfer it.
    async fn exchange(&mut self, path: &str, cache: Option<&Cache>) -> Result<Source, Failure> {
        let observed = match self.modification(path).await {
            Ok(Some(instant)) => Some(instant),
            Ok(None) | Err(Time::Unsupported) => self.listed(path).await?,
            Err(Time::Missing) => return Err(NotFound.into()),
            Err(Time::Failed(failure)) => return Err(failure),
        };
        let Some(instant) = observed else {
            return Err(NotFound.into());
        };
        if matches!(cache, Some(Cache::Ftp(Some(previous))) if *previous == instant) {
            return Err(NotModified.into());
        }
        Ok(Source {
            bytes: self.transfer(path).await?,
            cache: Cache::Ftp(Some(instant)),
        })
    }

    /// Say goodbye without waiting for the answer, as the reference client does.
    async fn close(self) {
        // shortcut: a control socket whose send buffer is full loses the goodbye, whose answer the
        // reference ignores as well.
        let _ = self.stream.get_ref().await.try_write(b"QUIT\r\n");
    }

    /// The file's modification instant, or why the server did not report one.
    async fn modification(&mut self, path: &str) -> Result<Option<i64>, Time> {
        let requested = self.command(format!("MDTM {path}"), &[Status::File]).await;
        match requested {
            // The reply body repeats its code, so the stamp starts at its fourth byte.
            Ok(response) => Ok(String::from_utf8_lossy(&response.body)
                .get(4..)
                .map(str::trim_end)
                .and_then(instant_millis)),
            Err(failure) if refused_file(&failure) => Err(Time::Missing),
            Err(failure) if replied(&failure) => Err(Time::Unsupported),
            Err(failure) => Err(Time::Failed(failure)),
        }
    }

    /// The containing directory's entry instant, when the server answered with a machine listing.
    async fn listed(&mut self, path: &str) -> Result<Option<i64>, Failure> {
        let directory = posix::dirname(path);
        let name = posix::basename(path, None);
        let lines = self.entries(&directory).await?;
        let Some(last) = lines.last() else {
            return Ok(None);
        };
        let Some(format) = listing::format(last) else {
            return Err(io::Error::other(
                "The FTP server uses a directory-listing format other than MLSD, Unix or DOS",
            )
            .into());
        };
        let Some(entry) = listing::entry(&lines, format, &name) else {
            return Ok(None);
        };
        Ok(match format {
            listing::Format::Mlsd => listing::modified(entry).and_then(instant_millis),
            listing::Format::Dos | listing::Format::Posix => None,
        })
    }

    /// The directory's entries, from a machine listing or from a text one.
    async fn entries(&mut self, directory: &str) -> Result<Vec<String>, Failure> {
        if self.machine {
            let command = format!("MLSD {directory}");
            match self.listing(&command).await {
                Ok(lines) => return Ok(lines),
                // The reference falls back to `LIST` only when the server answered the listing.
                Err(failure) if replied(&failure) => {}
                Err(failure) => return Err(failure),
            }
        }
        self.listing(&format!("LIST {directory}")).await
    }

    /// One listing command's lines, within the reference client's listing limit.
    async fn listing(&mut self, command: &str) -> Result<Vec<String>, Failure> {
        let mut data = self.data().await?;
        self.command(command, &[Status::AboutToSend, Status::AlreadyOpen])
            .await?;
        let bytes = payload(&mut data, Some(LISTING_LIMIT)).await?;
        self.complete(&mut data).await?;
        Ok(split(&bytes))
    }

    /// Read the file, reporting a refused file as the source's not-found category.
    async fn transfer(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        let mut data = self.data().await?;
        self.command(
            format!("RETR {path}"),
            &[Status::AboutToSend, Status::AlreadyOpen],
        )
        .await
        .map_err(unavailable)?;
        let bytes = payload(&mut data, None).await?;
        self.complete(&mut data).await?;
        Ok(bytes)
    }

    /// Close the data connection and read the reply that completes the transfer.
    async fn complete(&mut self, data: &mut TcpStream) -> Result<(), Failure> {
        data.shutdown()
            .await
            .map_err(|error| -> Failure { FtpError::ConnectionError(error).into() })?;
        self.reply(&[Status::ClosingDataConnection, Status::RequestedFileActionOk])
            .await?;
        Ok(())
    }

    /// Open a data connection with the passive strategy the server accepted, or the first it
    /// accepts when none was tried yet.
    async fn data(&mut self) -> Result<TcpStream, Failure> {
        if let Some(strategy) = self.strategy {
            return self.prepare(strategy).await;
        }
        // A server that refuses a strategy may accept the next one, so the reference tries both
        // before it reports a transfer as impossible.
        if let Ok(data) = self.prepare(Strategy::Extended).await {
            self.strategy = Some(Strategy::Extended);
            Ok(data)
        } else {
            let data = self.prepare(Strategy::Standard).await?;
            self.strategy = Some(Strategy::Standard);
            Ok(data)
        }
    }

    /// Ask the server for a data address and connect to it, without sending the operation itself.
    async fn prepare(&mut self, strategy: Strategy) -> Result<TcpStream, Failure> {
        let address = match strategy {
            Strategy::Extended => {
                let reply = self.command("EPSV", &[Status::ExtendedPassiveMode]).await?;
                let port = extended_port(&String::from_utf8_lossy(&reply.body))?;
                SocketAddr::new(self.control_host().await?, port)
            }
            Strategy::Standard => {
                let reply = self.command("PASV", &[Status::PassiveMode]).await?;
                let advertised = passive_address(&String::from_utf8_lossy(&reply.body))?;
                data_host(self.control_host().await.ok(), advertised)
            }
        };
        bounded(Connection::Data, TcpStream::connect(address)).await
    }

    /// The address of the control connection, which an extended passive transfer reuses.
    async fn control_host(&mut self) -> Result<IpAddr, Failure> {
        match self.stream.get_ref().await.peer_addr() {
            Ok(peer) => Ok(peer.ip()),
            Err(error) => Err(io::Error::other(format!(
                "Control socket is disconnected, can't get remote address: {error}"
            ))
            .into()),
        }
    }

    /// Whether the server advertises a machine listing (`MLST`).
    async fn advertised(&mut self) -> bool {
        // The library's own feature reader waits for continuation lines after a `211-` opener,
        // which a server without optional features never sends; reading the reply as a
        // non-accepted answer returns its complete body for both shapes and keeps the control
        // connection in step.
        match self.command("FEAT", &[Status::CommandOk]).await {
            Ok(response) => announces_mlst(&response.body),
            Err(failure) => {
                answered(&failure).is_some_and(|response| announces_mlst(&response.body))
            }
        }
    }
}

/// A passive transfer strategy, in the order the reference client tries them.
#[derive(Clone, Copy)]
enum Strategy {
    /// Extended passive mode, which takes the host of the control connection.
    Extended,
    /// Passive mode, whose advertised data host is kept unless a NAT replacement applies.
    Standard,
}

/// The connection a wait belongs to, named as the reference client names it.
#[derive(Clone, Copy, Debug)]
enum Connection {
    /// The control connection, which carries every command.
    Control,
    /// The data connection, which carries a listing or a transfer.
    Data,
}

impl Connection {
    /// The name this connection is reported under when it stalls.
    fn name(self) -> &'static str {
        match self {
            Self::Control => "control socket",
            Self::Data => "data socket",
        }
    }
}

/// Await `wait` for at most the inactivity deadline, as a socket timeout would.
async fn bounded<T, E>(
    connection: Connection,
    wait: impl Future<Output = Result<T, E>>,
) -> Result<T, Failure>
where
    E: Into<Failure>,
{
    match tokio::time::timeout(INACTIVITY, wait).await {
        Ok(result) => result.map_err(Into::into),
        Err(_) => Err(Box::new(Stalled(connection))),
    }
}

/// A wait that ended before the server answered.
#[derive(Debug)]
struct Stalled(Connection);

impl std::fmt::Display for Stalled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Timeout ({})", self.0.name())
    }
}

impl std::error::Error for Stalled {}

/// Whether the failure is an inactivity deadline rather than a server answer.
fn stalling(failure: &Failure) -> bool {
    failure.downcast_ref::<Stalled>().is_some()
}

/// Read a data connection to its end, refusing more than `limit` bytes.
pub(super) async fn payload<R>(data: &mut R, limit: Option<usize>) -> Result<Vec<u8>, Failure>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let read = bounded(Connection::Data, data.read(&mut chunk)).await?;
        if read == 0 {
            return Ok(bytes);
        }
        if limit.is_some_and(|limit| bytes.len() + read > limit) {
            return Err(io::Error::other(format!(
                "The FTP server sent a directory listing larger than {LISTING_LIMIT} bytes"
            ))
            .into());
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
}

/// The listing lines, without blank lines and without a `total` summary line.
fn split(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with("total"))
        .map(str::to_owned)
        .collect()
}

/// The address a passive data connection uses.
fn data_host(control: Option<IpAddr>, advertised: SocketAddr) -> SocketAddr {
    // A server behind NAT advertises an address outside its client's reach; the control host is the
    // one that works. Any other advertised host is kept, because a server may use a separate one.
    match control {
        Some(control) if private(advertised.ip()) && !private(control) => {
            SocketAddr::new(control, advertised.port())
        }
        _ => advertised,
    }
}

/// Whether an address is the loopback address or one of the private IPv4 ranges.
fn private(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => address.is_private() || address == Ipv4Addr::LOCALHOST,
        IpAddr::V6(_) => false,
    }
}

/// The port of an extended passive reply, between the delimiters the reference accepts.
fn extended_port(reply: &str) -> Result<u16, Failure> {
    // A server on IBM i answers `229 (!!!port!)` where most servers answer `(|||port|)`, so the
    // reference accepts either delimiter, mixed included.
    let delimiter = |character: char| matches!(character, '|' | '!');
    let opening = reply
        .char_indices()
        .find(|(index, _)| {
            reply[*index..]
                .chars()
                .take(3)
                .filter(|character| delimiter(*character))
                .count()
                == 3
        })
        .map(|(index, _)| index);
    let refused = || -> Failure { io::Error::other(unparsable("EPSV", reply)).into() };
    let Some(opening) = opening else {
        return Err(refused());
    };
    let payload = &reply[opening + 3..];
    let Some(closing) = payload.rfind(delimiter) else {
        return Err(refused());
    };
    payload[..closing].trim().parse().map_err(|_| refused())
}

/// The data address of a passive reply, from its comma-separated host and port.
fn passive_address(reply: &str) -> Result<SocketAddr, Failure> {
    let refused = || -> Failure { io::Error::other(unparsable("PASV", reply)).into() };
    let inside = reply
        .split_once('(')
        .and_then(|(_, rest)| rest.rsplit_once(')').map(|(inside, _)| inside));
    let Some(inside) = inside else {
        return Err(refused());
    };
    let numbers = inside
        .split(',')
        .map(|part| part.trim().parse::<u16>())
        .collect::<Result<Vec<u16>, _>>()
        .map_err(|_| refused())?;
    let [first, second, third, fourth, high, low] = numbers.as_slice() else {
        return Err(refused());
    };
    let octet = |value: &u16| u8::try_from(*value).map_err(|_| refused());
    let host = Ipv4Addr::new(octet(first)?, octet(second)?, octet(third)?, octet(fourth)?);
    Ok(SocketAddr::new(
        IpAddr::V4(host),
        ((high & 255) << 8) | (low & 255),
    ))
}

/// The message the reference reports for a passive reply it cannot parse.
fn unparsable(command: &str, reply: &str) -> String {
    format!("Can't parse {command} response: {reply}")
}

/// Whether a `FEAT` reply body advertises a machine listing.
fn announces_mlst(body: &[u8]) -> bool {
    String::from_utf8_lossy(body)
        .lines()
        .any(|line| line.trim_start().starts_with("MLST"))
}

/// A URL credential, defaulting exactly as the reference client logs in.
fn credential(value: &str, default: &str) -> String {
    if value.is_empty() {
        default.to_owned()
    } else {
        String::from(value)
    }
}

/// Why a modification-time observation did not supply an instant.
enum Time {
    /// The path is unavailable to this account.
    Missing,
    /// The server does not implement the modification-time command.
    Unsupported,
    /// The wait ended before the server answered.
    Failed(Failure),
}

/// The server's reply, when the failure carries one.
fn answered(failure: &Failure) -> Option<&Response> {
    match failure.downcast_ref::<FtpError>() {
        Some(FtpError::UnexpectedResponse(response)) => Some(response),
        _ => None,
    }
}

/// Whether the server answered the command, rather than the connection failing.
fn replied(failure: &Failure) -> bool {
    answered(failure).is_some()
}

/// Whether the server refused the file itself.
fn refused_file(failure: &Failure) -> bool {
    answered(failure).is_some_and(|response| response.status == Status::FileUnavailable)
}

/// A refusal of the file itself is the not-found category, as the reference already reports it.
fn unavailable(failure: Failure) -> Failure {
    if refused_file(&failure) {
        NotFound.into()
    } else {
        failure
    }
}
