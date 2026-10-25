//! FTP script sources: login, modification check, listing fallback and transfer.

mod client;
mod ftp_context;
pub(super) mod parse_control_response;
mod transfer;

use std::io;
use std::time::Duration;

#[cfg(test)]
use tokio::io::AsyncRead;

use super::{Cache, Failure, Source, listing, notfound::NotFound, notmodified::NotModified};
use client::{Client, Modification};
#[cfg(test)]
pub(super) use ftp_context::CONSUMED;
use ftp_context::FtpError;
#[cfg(test)]
use ftp_context::{Data, Payload};

/// The silence a control or data connection tolerates before its wait counts as stalled.
pub(super) const INACTIVITY: Duration = Duration::from_secs(30);

/// The largest directory listing the reference client accepts, in bytes.
const LISTING_LIMIT: usize = 40 * 1024 * 1024;

/// Load one FTP script, closing the connection on every exit path.
pub(super) async fn load(uri: &url::Url, cache: Option<&Cache>) -> Result<Source, Failure> {
    let path = super::super::proxy_agent::decode(uri.path())?;
    let mut client = Client::access(uri).await?;
    let result = exchange(&mut client, &path, cache).await;
    client.close();
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

/// Read a supplied reader to its end, under the data connection's inactivity window.
#[cfg(test)]
pub(super) async fn payload<R>(data: &mut R, limit: Option<usize>) -> Result<Vec<u8>, Failure>
where
    R: AsyncRead + Unpin,
{
    let mut reader = Data::new(data);
    reader.admit();
    let mut payload = Payload::bounded(limit);
    let mut chunk = [0u8; 8192];
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            return Ok(payload.finish());
        }
        payload.extend(&chunk[..read])?;
    }
}

/// Compare the retained observation with the file, then transfer it.
async fn exchange(
    client: &mut Client,
    path: &str,
    cache: Option<&Cache>,
) -> Result<Source, Failure> {
    let observed = match client.last_modified(path).await? {
        Modification::Instant(instant) => Some(instant),
        Modification::Missing => return Err(NotFound.into()),
        Modification::Unsupported => listed(client, path).await?,
    };
    let Some(instant) = observed else {
        return Err(NotFound.into());
    };
    if matches!(cache, Some(Cache::Ftp(Some(previous))) if *previous == instant) {
        return Err(NotModified.into());
    }
    Ok(Source {
        bytes: client.download(path).await.map_err(unavailable)?,
        cache: Cache::Ftp(Some(instant)),
    })
}

/// The containing directory's entry instant, when the server answered with a machine listing.
async fn listed(client: &mut Client, path: &str) -> Result<Option<i64>, Failure> {
    let directory = maestro_path::posix::dirname(path);
    let name = maestro_path::posix::basename(path, None);
    let lines = client.list(&directory).await?;
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

/// A refusal of the file itself is the not-found category, as the reference already reports it.
fn unavailable(failure: Failure) -> Failure {
    match failure.downcast_ref::<FtpError>() {
        Some(error) if error.code == 550 => NotFound.into(),
        _ => failure,
    }
}
