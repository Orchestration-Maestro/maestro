//! The FTP client operations the source loader uses.

use url::Url;

use super::ftp_context::{FtpContext, FtpError};
use super::transfer::{self, Passive};
use super::{LISTING_LIMIT, port};
use crate::providers::http::get_uri::Failure;
use crate::providers::http::get_uri::listing;

/// The directory facts the reference client asks for before a machine listing.
const LISTING_FACTS: &str = concat!(
    "type;size;modify;unique;",
    "unix.mode;unix.owner;unix.group;",
    "unix.ownername;unix.groupname;"
);

/// The listing commands of a server that advertises machine listings.
const MACHINE_LISTINGS: [&str; 3] = ["MLSD", "LIST -a", "LIST"];

/// The listing commands of a server without machine listings.
const TEXT_LISTINGS: [&str; 2] = ["LIST -a", "LIST"];

/// Why a modification-time observation did not supply an instant.
pub(super) enum Modification {
    /// The server reported this instant.
    Instant(i64),
    /// The server refused the modification command, so a listing may still carry one.
    Unsupported,
    /// The server does not have the requested file.
    Missing,
}

/// One accessed FTP session, which owns its control connection.
pub(super) struct Client {
    /// The owned connections.
    context: FtpContext,
    /// The passive strategy this session keeps using.
    passive: Passive,
    /// The listing commands still worth trying, narrowed to the first that worked.
    listings: Vec<&'static str>,
}

impl Client {
    /// Connect, greet, log in and select the transfer settings the reference uses.
    ///
    /// # Errors
    /// Returns a decoding, transport, feature, login or settings failure.
    pub(super) async fn access(uri: &Url) -> Result<Self, Failure> {
        let decoded_user = super::super::super::proxy_agent::decode(uri.username())?;
        let user = credential(&decoded_user, "anonymous");
        let decoded_password =
            super::super::super::proxy_agent::decode(uri.password().unwrap_or_default())?;
        let password = credential(&decoded_password, "guest");
        let host = uri
            .host_str()
            .filter(|host| !host.is_empty())
            .unwrap_or("localhost");
        let mut context = FtpContext::open(host, port(uri)).await?;
        context.greeting().await?;
        context.request_ignoring_error("OPTS UTF8 ON").await?;
        login(&mut context, user, password).await?;
        let machine = features(&mut context).await?;
        context.request("TYPE I").await?;
        context.request_ignoring_error("STRU F").await?;
        context.request_ignoring_error("OPTS UTF8 ON").await?;
        if machine {
            context
                .request_ignoring_error(&format!("OPTS MLST {LISTING_FACTS}"))
                .await?;
        }
        Ok(Self {
            context,
            passive: Passive::default(),
            listings: if machine {
                MACHINE_LISTINGS.to_vec()
            } else {
                TEXT_LISTINGS.to_vec()
            },
        })
    }

    /// The file's modification instant, or why the server did not report one.
    ///
    /// # Errors
    /// Returns a transport failure, which ends the session.
    pub(super) async fn last_modified(&mut self, path: &str) -> Result<Modification, Failure> {
        let command = format!("MDTM {path}");
        match self.context.request(&command).await {
            Ok(reply) => Ok(reply
                .message
                .get(4..)
                .and_then(super::instant_millis)
                .map_or(Modification::Unsupported, Modification::Instant)),
            Err(failure) => match failure.downcast_ref::<FtpError>() {
                Some(error) if error.code == 550 => Ok(Modification::Missing),
                Some(_) => Ok(Modification::Unsupported),
                None => Err(failure),
            },
        }
    }

    /// The directory's content lines, from the first listing command the server accepts.
    ///
    /// # Errors
    /// Returns the last server refusal, a transport failure, or the listing size refusal.
    pub(super) async fn list(&mut self, directory: &str) -> Result<Vec<String>, Failure> {
        let mut last: Option<Failure> = None;
        for candidate in std::mem::take(&mut self.listings) {
            let command = if directory.is_empty() {
                candidate.to_owned()
            } else {
                format!("{candidate} {directory}")
            };
            let data = self.passive.prepare(&mut self.context).await?;
            self.context.install(data);
            match transfer::download(&mut self.context, &command, Some(LISTING_LIMIT)).await {
                Ok(bytes) => {
                    self.listings = vec![candidate];
                    return Ok(listing::lines(&String::from_utf8_lossy(&bytes)));
                }
                Err(failure) if failure.downcast_ref::<FtpError>().is_some() => {
                    last = Some(failure);
                }
                Err(failure) => return Err(failure),
            }
        }
        Err(last
            .unwrap_or_else(|| std::io::Error::other("no listing command could be tried").into()))
    }

    /// The file's bytes, transferred over a freshly prepared data connection.
    ///
    /// # Errors
    /// Returns a preparation or transfer failure.
    pub(super) async fn download(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        let data = self.passive.prepare(&mut self.context).await?;
        self.context.install(data);
        transfer::download(&mut self.context, &format!("RETR {path}"), None).await
    }

    /// Say goodbye and release the owned connections.
    pub(super) fn close(self) {
        self.context.close();
    }
}

/// Log in, answering a password request as the reference does.
async fn login(context: &mut FtpContext, user: &str, password: &str) -> Result<(), Failure> {
    let reply = context
        .request_ignoring_error(&format!("USER {user}"))
        .await?;
    if reply.completed() {
        return Ok(());
    }
    if reply.code != 331 {
        return Err(Box::new(FtpError::of(reply)));
    }
    let reply = context
        .request_ignoring_error(&format!("PASS {password}"))
        .await?;
    if reply.completed() {
        Ok(())
    } else {
        Err(Box::new(FtpError::of(reply)))
    }
}

/// Whether the server advertises a machine listing through its optional features.
async fn features(context: &mut FtpContext) -> Result<bool, Failure> {
    let reply = context.request_ignoring_error("FEAT").await?;
    Ok(reply.code < 400 && reply.message.starts_with("211-") && announces_mlst(&reply.message))
}

/// Whether a multiline feature reply lists `MLST` among its entries.
fn announces_mlst(message: &str) -> bool {
    let mut lines = message.split('\n');
    lines.next();
    // The first and last lines wrap the multiline reply.
    lines.next_back();
    lines.any(|line| {
        line.trim_matches(maestro_text::is_whitespace_scalar)
            .split(' ')
            .next()
            == Some("MLST")
    })
}

/// A URL credential, defaulting exactly as the reference client logs in.
fn credential<'a>(value: &'a str, default: &'a str) -> &'a str {
    if value.is_empty() { default } else { value }
}
