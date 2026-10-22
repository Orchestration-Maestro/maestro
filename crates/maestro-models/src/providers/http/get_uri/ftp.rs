//! FTP script sources: login, modification check, listing fallback and transfer.

use tokio::io::AsyncReadExt as _;

use super::{Cache, Failure, Source, listing, notfound::NotFound, notmodified::NotModified};
use suppaftp::Status;
use suppaftp::tokio::AsyncFtpStream;
use suppaftp::types::{FileType, FtpError, Mode};

/// The directory facts the reference client asks for before a machine listing.
const LISTING_FACTS: &str = concat!(
    "type;size;modify;unique;",
    "unix.mode;unix.owner;unix.group;",
    "unix.ownername;unix.groupname;"
);

/// Load one FTP script, closing the connection on every exit path.
pub(super) async fn load(uri: &url::Url, cache: Option<&Cache>) -> Result<Source, Failure> {
    let path = super::super::proxy_agent::decode(uri.path())?;
    let (mut stream, machine) = session(uri).await?;
    let result = exchange(&mut stream, &path, cache, machine).await;
    let _ = stream.quit().await;
    result
}

/// Compare the file's modification instant with the retained one, then transfer it.
async fn exchange(
    stream: &mut AsyncFtpStream,
    path: &str,
    cache: Option<&Cache>,
    machine: bool,
) -> Result<Source, Failure> {
    let observed = match modification(stream, path).await {
        Ok(Some(instant)) => Some(instant),
        Ok(None) | Err(Time::Unsupported) => listed(stream, path, machine).await?,
        Err(Time::Missing) => return Err(NotFound.into()),
    };
    let Some(instant) = observed else {
        return Err(NotFound.into());
    };
    if matches!(cache, Some(Cache::Ftp(Some(previous))) if *previous == instant) {
        return Err(NotModified.into());
    }
    Ok(Source {
        bytes: transfer(stream, path).await?,
        cache: Cache::Ftp(Some(instant)),
    })
}

/// Connect, log in and select the transfer settings the reference client uses.
async fn session(uri: &url::Url) -> Result<(AsyncFtpStream, bool), Failure> {
    let host = uri
        .host_str()
        .filter(|host| !host.is_empty())
        .unwrap_or("localhost");
    let mut stream = AsyncFtpStream::connect((host, port(uri))).await?;
    if stream
        .get_ref()
        .await
        .peer_addr()
        .is_ok_and(|address| address.is_ipv6())
    {
        stream.set_mode(Mode::ExtendedPassive);
    }
    stream.set_passive_nat_workaround(true);
    let _ = stream.opts("UTF8", Some("ON")).await;
    let user = credential(
        &super::super::proxy_agent::decode(uri.username())?,
        "anonymous",
    );
    let password = credential(
        &super::super::proxy_agent::decode(uri.password().unwrap_or_default())?,
        "guest",
    );
    stream.login(&user, &password).await?;
    let machine = advertised(&mut stream).await;
    stream.transfer_type(FileType::Binary).await?;
    let _ = stream.custom_command("STRU F", &[Status::CommandOk]).await;
    let _ = stream.opts("UTF8", Some("ON")).await;
    if machine {
        let _ = stream.opts("MLST", Some(LISTING_FACTS)).await;
    }
    Ok((stream, machine))
}

/// The port to reach: an explicit `0` selects the standard port, as in the reference.
pub(super) fn port(uri: &url::Url) -> u16 {
    uri.port().filter(|port| *port != 0).unwrap_or(21)
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
    let body = match stream.custom_command("FEAT", &[Status::CommandOk]).await {
        Ok(response) | Err(FtpError::UnexpectedResponse(response)) => response.body,
        Err(_) => return false,
    };
    String::from_utf8_lossy(&body)
        .lines()
        .any(|line| line.trim_start().starts_with("MLST"))
}

/// Why a modification-time observation could not be taken.
enum Time {
    /// The server reported the path as unavailable.
    Missing,
    /// The server does not implement the modification-time command.
    Unsupported,
}

/// The file's modification instant, or why the server did not report one.
async fn modification(stream: &mut AsyncFtpStream, path: &str) -> Result<Option<i64>, Time> {
    match stream
        .custom_command(format!("MDTM {path}"), &[Status::File])
        .await
    {
        // The reply body repeats its code, so the stamp starts at its fourth byte.
        Ok(response) => Ok(String::from_utf8_lossy(&response.body)
            .get(4..)
            .map(str::trim_end)
            .and_then(instant_millis)),
        Err(FtpError::UnexpectedResponse(response))
            if response.status == Status::FileUnavailable =>
        {
            Err(Time::Missing)
        }
        Err(_) => Err(Time::Unsupported),
    }
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

/// The containing directory's entry instant, when the server answered with a machine listing.
async fn listed(
    stream: &mut AsyncFtpStream,
    path: &str,
    machine: bool,
) -> Result<Option<i64>, Failure> {
    let directory = dirname(path);
    let name = basename(path);
    let lines = if machine {
        match stream.mlsd(Some(&directory)).await {
            Ok(lines) => lines,
            Err(_) => stream.list(Some(&directory)).await?,
        }
    } else {
        stream.list(Some(&directory)).await?
    };
    let content: Vec<String> = lines
        .into_iter()
        .filter(|line| !line.trim().is_empty() && !line.starts_with("total"))
        .collect();
    let Some(last) = content.last() else {
        return Ok(None);
    };
    let Some(format) = listing::format(last) else {
        return Err(std::io::Error::other(
            "The FTP server uses a directory-listing format other than MLSD, Unix or DOS",
        )
        .into());
    };
    let Some(entry) = listing::entry(&content, format, &name) else {
        return Ok(None);
    };
    Ok(match format {
        listing::Format::Mlsd => modified(entry),
        listing::Format::Dos | listing::Format::Posix => None,
    })
}

/// The `modify` fact of a machine-listing line, when it carries one.
fn modified(line: &str) -> Option<i64> {
    line.split(' ')
        .next()?
        .split(';')
        .filter_map(|fact| fact.split_once('='))
        .find(|(name, _)| name.eq_ignore_ascii_case("modify"))
        .and_then(|(_, stamp)| instant_millis(stamp))
}

/// Read the whole file, reporting a refused file as the source's not-found category.
async fn transfer(stream: &mut AsyncFtpStream, path: &str) -> Result<Vec<u8>, Failure> {
    let mut data = stream.retr_as_stream(path).await.map_err(unavailable)?;
    let mut bytes = Vec::new();
    data.read_to_end(&mut bytes).await?;
    data.finish().await?;
    Ok(bytes)
}

/// A refusal of the file itself is the not-found category, as the reference already reports it.
fn unavailable(error: FtpError) -> Failure {
    match error {
        FtpError::UnexpectedResponse(response) if response.status == Status::FileUnavailable => {
            NotFound.into()
        }
        error => error.into(),
    }
}

/// Node's `path.posix.dirname`: trailing slashes are dropped and the root keeps its slash.
pub(super) fn dirname(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    let parent = trimmed
        .rfind('/')
        .map_or("", |index| &trimmed[..index])
        .trim_end_matches('/');
    if !parent.is_empty() {
        parent.to_owned()
    } else if path.starts_with('/') {
        "/".to_owned()
    } else {
        ".".to_owned()
    }
}

/// Node's `path.posix.basename`: trailing slashes are dropped and a directory has no name.
pub(super) fn basename(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_owned()
}
