//! Passive data-connection preparation and transfer settlement.

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::LazyLock;

use tokio::net::TcpStream;

use super::ftp_context::{Data, FtpContext, Payload, connect_data};
use super::parse_control_response::Reply;
use crate::providers::http::decimal_prefix;
use crate::providers::http::get_uri::Failure;

/// The extended passive reply, whose payload sits between three delimiter characters.
static EXTENDED: LazyLock<Option<regex::Regex>> =
    LazyLock::new(|| regex::Regex::new("[|!]{3}(.+)[|!]").ok());

/// The classic passive reply, whose six numbers follow each other separated by commas.
static STANDARD: LazyLock<Option<regex::Regex>> =
    LazyLock::new(|| regex::Regex::new(r"([-\d]+,[-\d]+,[-\d]+,[-\d]+),([-\d]+),([-\d]+)").ok());

/// One passive strategy, in the order the client tries them.
#[derive(Clone, Copy)]
enum Strategy {
    /// Extended passive mode, which reuses the control connection's host.
    Extended,
    /// Classic passive mode, which keeps the advertised data host unless a NAT replacement
    /// applies.
    Standard,
}

/// The passive strategy that worked, once one has.
#[derive(Default)]
pub(super) struct Passive {
    /// The strategy this session keeps using.
    chosen: Option<Strategy>,
}

impl Passive {
    /// Open a data connection, reporting the first strategy that works.
    ///
    /// # Errors
    /// Returns the last strategy's failure when every strategy fails.
    pub(super) async fn prepare(
        &mut self,
        context: &mut FtpContext,
    ) -> Result<Data<TcpStream>, Failure> {
        if let Some(chosen) = self.chosen {
            return open(chosen, context).await;
        }
        let mut last: Option<Failure> = None;
        for strategy in [Strategy::Extended, Strategy::Standard] {
            match open(strategy, context).await {
                Ok(data) => {
                    self.chosen = Some(strategy);
                    return Ok(data);
                }
                Err(failure) => last = Some(failure),
            }
        }
        let reason = last.map_or_else(|| "no strategy".to_owned(), |failure| failure.to_string());
        Err(io::Error::other(format!(
            "None of the available transfer strategies work. Last error response was '{reason}'."
        ))
        .into())
    }
}

/// Ask for one passive data address and connect to it.
async fn open(strategy: Strategy, context: &mut FtpContext) -> Result<Data<TcpStream>, Failure> {
    let target = match strategy {
        Strategy::Extended => {
            let reply = context.request("EPSV").await?;
            SocketAddr::new(context.peer()?, extended_port(&reply.message)?)
        }
        Strategy::Standard => {
            let reply = context.request("PASV").await?;
            let advertised = passive_address(&reply.message)?;
            data_target(context.peer().ok(), advertised)
        }
    };
    connect_data(target)
        .await
        .map_err(|error| -> Failure { Box::new(error) })
}

/// Collect one transfer's bytes, settling control and data together.
///
/// # Errors
/// Returns the server's refusal, a transport failure, or the listing size refusal.
pub(super) async fn download(
    context: &mut FtpContext,
    command: &str,
    limit: Option<usize>,
) -> Result<Vec<u8>, Failure> {
    context.start(command).await?;
    let mut payload = Payload::bounded(limit);
    let mut chunk = [0u8; 8192];
    let mut eof = false;
    let mut completed = false;
    loop {
        if eof && completed {
            context.release();
            return Ok(payload.finish());
        }
        let event = {
            let (control, data) = context.parts();
            tokio::select! {
                reply = control.reply() => Event::Reply(reply),
                read = read(data, &mut chunk) => Event::Data(read),
            }
        };
        match event {
            Event::Reply(Err(failure)) | Event::Data(Err(failure)) => {
                return Err(context.fail(failure));
            }
            Event::Reply(Ok(reply)) if reply.refused() => {
                // A refusal answers the transfer; it leaves the session usable, as a session that
                // keeps its control connection does for the next command.
                context.release();
                return Err(Box::new(super::ftp_context::FtpError::of(reply)));
            }
            Event::Reply(Ok(reply)) if reply.intermediate() => {
                return Err(context.fail(unexpected(&reply)));
            }
            Event::Reply(Ok(reply)) if reply.code == 125 || reply.code == 150 => {
                if let Err(failure) = context.on_data_start() {
                    return Err(context.fail(failure));
                }
            }
            Event::Reply(Ok(reply)) => completed |= reply.completed(),
            Event::Data(Ok(0)) => {
                eof = true;
                context.on_data_done();
                context.shutdown_data().await;
            }
            Event::Data(Ok(read)) => {
                if let Err(failure) = payload.extend(&chunk[..read]) {
                    return Err(context.fail(failure));
                }
            }
        }
    }
}

/// One event a transfer settles.
enum Event {
    /// A reply on the control connection.
    Reply(Result<Reply, Failure>),
    /// A read on the data connection.
    Data(Result<usize, Failure>),
}

/// Read one data chunk while the transfer admitted the connection and its data has not ended.
async fn read(
    data: Option<&mut Data<TcpStream>>,
    chunk: &mut [u8; 8192],
) -> Result<usize, Failure> {
    match data {
        Some(data) if data.active() => data.read(&mut chunk[..]).await,
        _ => std::future::pending().await,
    }
}

/// The failure of a reply that asks for an answer the client cannot give.
fn unexpected(reply: &Reply) -> Failure {
    io::Error::other(format!(
        "Unexpected FTP response is requesting an answer: {}",
        reply.message
    ))
    .into()
}

/// The message reported for a passive reply this client cannot parse.
fn unparsable(command: &str, reply: &str) -> Failure {
    io::Error::other(format!("Can't parse {command} response: {reply}")).into()
}

/// The port of an extended passive reply, between any three matching delimiters.
fn extended_port(reply: &str) -> Result<u16, Failure> {
    let inside = EXTENDED
        .as_ref()
        .and_then(|pattern| pattern.captures(reply))
        .and_then(|groups| groups.get(1))
        .map(|group| group.as_str());
    let Some(inside) = inside else {
        return Err(unparsable("EPSV", reply));
    };
    decimal_prefix(inside)
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port != 0)
        .ok_or_else(|| unparsable("EPSV", reply))
}

/// The data address of a classic passive reply, masked to two bytes.
fn passive_address(reply: &str) -> Result<SocketAddr, Failure> {
    let Some(groups) = STANDARD
        .as_ref()
        .and_then(|pattern| pattern.captures(reply))
    else {
        return Err(unparsable("PASV", reply));
    };
    let host = groups.get(1).map_or("", |group| group.as_str());
    let high = groups
        .get(2)
        .and_then(|group| decimal_prefix(group.as_str()));
    let low = groups
        .get(3)
        .and_then(|group| decimal_prefix(group.as_str()));
    let octets = host
        .replace(',', ".")
        .parse::<Ipv4Addr>()
        .map_err(|_| unparsable("PASV", reply))?;
    let (Some(high), Some(low)) = (high, low) else {
        return Err(unparsable("PASV", reply));
    };
    let port =
        u16::try_from((high & 255) * 256 + (low & 255)).map_err(|_| unparsable("PASV", reply))?;
    Ok(SocketAddr::new(IpAddr::V4(octets), port))
}

/// The address a classic passive transfer connects to.
fn data_target(control: Option<IpAddr>, advertised: SocketAddr) -> SocketAddr {
    // A server behind NAT advertises an address outside its client's reach; the control host is
    // the one that works. Any other advertised host is kept, because a server may use a separate
    // one.
    match control {
        Some(control) if private(advertised.ip()) && !private(control) => {
            SocketAddr::new(control, advertised.port())
        }
        _ => advertised,
    }
}

/// Whether an address is a private IPv4 address, as the reference decides it.
fn private(address: IpAddr) -> bool {
    let address = match address {
        IpAddr::V4(address) => Some(address),
        IpAddr::V6(address) => address.to_ipv4_mapped(),
    };
    address.is_some_and(|address| address.is_private() || address == Ipv4Addr::LOCALHOST)
}
