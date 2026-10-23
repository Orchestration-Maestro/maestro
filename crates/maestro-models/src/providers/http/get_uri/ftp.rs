//! FTP script sources: login, modification check, listing fallback and transfer.

use std::future::Future;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt as _};
use tokio::net::TcpStream;

use super::{Cache, Failure, Source, listing, notfound::NotFound, notmodified::NotModified};
use maestro_path::posix;
use suppaftp::Status;
use suppaftp::tokio::{AsyncFtpStream, AsyncNoTlsStream, TransferStream};
use suppaftp::types::{FileType, FtpError, Mode, Response};

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

/// An open FTP session and the passive strategy chosen for it.
struct Session {
    /// The control connection.
    stream: AsyncFtpStream,
    /// Whether the server advertises machine listings.
    machine: bool,
    /// The passive strategy that worked, once one has.
    strategy: Option<Strategy>,
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
        let mut stream = routing(stream).await;
        let _ = bounded(Connection::Control, stream.opts("UTF8", Some("ON"))).await;
        let user = credential(
            &super::super::proxy_agent::decode(uri.username())?,
            "anonymous",
        );
        let password = credential(
            &super::super::proxy_agent::decode(uri.password().unwrap_or_default())?,
            "guest",
        );
        bounded(Connection::Control, stream.login(&user, &password)).await?;
        let machine = advertised(&mut stream).await;
        bounded(Connection::Control, stream.transfer_type(FileType::Binary)).await?;
        let _ = bounded(
            Connection::Control,
            stream.custom_command("STRU F", &[Status::CommandOk]),
        )
        .await;
        let _ = bounded(Connection::Control, stream.opts("UTF8", Some("ON"))).await;
        if machine {
            let _ = bounded(
                Connection::Control,
                stream.opts("MLST", Some(LISTING_FACTS)),
            )
            .await;
        }
        Ok(Self {
            stream,
            machine,
            strategy: None,
        })
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
        let requested = bounded(
            Connection::Control,
            self.stream
                .custom_command(format!("MDTM {path}"), &[Status::File]),
        )
        .await;
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
        let mut data = self.data(Request::Listing(command)).await?;
        let bytes = payload(&mut data, Some(LISTING_LIMIT)).await?;
        bounded(Connection::Control, data.finish()).await?;
        Ok(split(&bytes))
    }

    /// Read the file, reporting a refused file as the source's not-found category.
    async fn transfer(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        let mut data = self.data(Request::File(path)).await.map_err(unavailable)?;
        let bytes = payload(&mut data, None).await?;
        bounded(Connection::Control, data.finish()).await?;
        Ok(bytes)
    }

    /// Open a data connection, using the first passive strategy the server accepts.
    async fn data(&mut self, request: Request<'_>) -> Result<Data, Failure> {
        let chosen = self.strategy;
        let strategy = chosen.unwrap_or(Strategy::Extended);
        self.stream.set_mode(strategy.mode());
        match self.connect(request).await {
            Ok(data) => {
                self.strategy = Some(strategy);
                Ok(data)
            }
            // A server that refuses a strategy may accept the next one, so the reference tries
            // both before it reports a transfer as impossible.
            Err(_) if chosen.is_none() => {
                self.strategy = Some(Strategy::Standard);
                self.stream.set_mode(Mode::Passive);
                self.connect(request).await
            }
            Err(failure) => Err(failure),
        }
    }

    /// Send one command and return the data connection it opened.
    async fn connect(&mut self, request: Request<'_>) -> Result<Data, Failure> {
        bounded(Connection::Control, async {
            match request {
                Request::Listing(command) => self
                    .stream
                    .custom_data_command(command, &[Status::AboutToSend, Status::AlreadyOpen])
                    .await
                    .map(|(_, data)| data),
                Request::File(path) => self.stream.retr_as_stream(path).await,
            }
        })
        .await
    }
}

/// A request carried by a data connection.
#[derive(Clone, Copy)]
enum Request<'a> {
    /// One listing command with its directory argument.
    Listing(&'a str),
    /// The file to transfer.
    File(&'a str),
}

/// A data connection the caller reads.
type Data = TransferStream<AsyncNoTlsStream>;

/// A passive transfer strategy, in the order the reference client tries them.
#[derive(Clone, Copy)]
enum Strategy {
    /// Extended passive mode, which takes the host of the control connection.
    Extended,
    /// Passive mode, whose advertised data host is kept unless a NAT replacement applies.
    Standard,
}

impl Strategy {
    /// The library mode this strategy selects.
    fn mode(self) -> Mode {
        match self {
            Self::Extended => Mode::ExtendedPassive,
            Self::Standard => Mode::Passive,
        }
    }
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

/// Resolve passive data hosts as the reference client does, then connect to the result.
async fn routing(stream: AsyncFtpStream) -> AsyncFtpStream {
    let control = stream
        .get_ref()
        .await
        .peer_addr()
        .ok()
        .map(|peer| peer.ip());
    stream.passive_stream_builder(move |advertised| {
        Box::pin(async move {
            let address = data_host(control, advertised);
            TcpStream::connect(address)
                .await
                .map_err(FtpError::ConnectionError)
        })
    })
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

/// A URL credential, defaulting exactly as the reference client logs in.
fn credential(value: &str, default: &str) -> String {
    if value.is_empty() {
        default.to_owned()
    } else {
        String::from(value)
    }
}

/// Whether the server advertises a machine listing (`MLST`).
async fn advertised(stream: &mut AsyncFtpStream) -> bool {
    // The library's own feature reader waits for continuation lines after a `211-` opener, which
    // a server without optional features never sends; reading the reply as a non-accepted answer
    // returns its complete body for both shapes and keeps the control connection in step.
    let reply = match bounded(
        Connection::Control,
        stream.custom_command("FEAT", &[Status::CommandOk]),
    )
    .await
    {
        Ok(response) => Some(response),
        Err(failure) => answered(&failure).cloned(),
    };
    let Some(reply) = reply else {
        return false;
    };
    let body = reply.body;
    String::from_utf8_lossy(&body)
        .lines()
        .any(|line| line.trim_start().starts_with("MLST"))
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
