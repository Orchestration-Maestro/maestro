//! Ordered establishment ends before any application request can be replayed.

use base64::Engine as _;
use futures_util::FutureExt;
use hyper_util::rt::TokioIo;

use super::candidates::{self, Candidate};
use super::{Failure, HttpRequest, HttpResponse};
use crate::providers::transport::client::{self, Route};
use crate::providers::transport::proxy_agent::{
    self,
    socket::{Driver, Socket, driving, secure},
};

/// Forwarding policy prepared before the connection is selected.
struct ForwardProxy {
    /// Endpoint for outer TLS, distinct from destination TLS.
    endpoint: url::Url,
    /// Once-decoded credentials belonging to the proxy alone.
    authorization: Option<hyper::header::HeaderValue>,
}

/// Connection phase outcome; a proxy refusal is a response, never a fallback error.
enum Established {
    /// Selected transport and optional proxy for forwarding or deferred proxy TLS.
    Socket(Socket, Option<ForwardProxy>),
    /// CONNECT status, headers and streaming body retained by the installed parser.
    Refused(hyper::Response<reqwest::Body>),
}

/// Evaluate constructors and socket attempts in order; send the application body only once.
pub(super) async fn request(
    candidates: Vec<String>,
    request: HttpRequest,
    route: &Route,
    client: &reqwest::Client,
) -> Result<HttpResponse, Failure> {
    let destination = url::Url::parse(&request.url)?;
    let mut prepared: hyper::Request<reqwest::Body> =
        client::prepare(client, request)?.try_into()?;
    let tls = proxy_agent::tls(true)?;
    let connecting = async {
        match select(&candidates, &destination, &tls).await? {
            Established::Socket(socket, proxy) => {
                let socket = complete(socket, &destination, proxy.as_ref(), tls).await?;
                Ok::<_, Failure>(Established::Socket(socket, proxy))
            }
            refused @ Established::Refused(_) => Ok(refused),
        }
    };
    let selected = if let Some(timeout) = route.connect_timeout {
        tokio::time::timeout(timeout, connecting)
            .await
            .map_err(|_| proxy_agent::ConnectTimeout)??
    } else {
        connecting.await?
    };
    let (socket, proxy) = match selected {
        Established::Socket(socket, proxy) => (socket, proxy),
        Established::Refused(response) => {
            return Ok(client::finish(reqwest::Response::from(response)));
        }
    };
    let absolute = proxy.is_some() && destination.scheme() != "https";
    if absolute {
        prepared
            .headers_mut()
            .entry("proxy-connection")
            .or_insert(hyper::header::HeaderValue::from_static("close"));
    }
    if absolute
        && let Some(proxy) = proxy
        && let Some(auth) = proxy.authorization
    {
        prepared
            .headers_mut()
            .insert(hyper::header::PROXY_AUTHORIZATION, auth);
    }
    let response = proxy_agent::exchange(socket, prepared, absolute).await?;
    Ok(client::finish(reqwest::Response::from(response)))
}

/// Unknown words and establishment failures advance, but malformed recognized URLs do not.
async fn select(
    candidates: &[String],
    destination: &url::Url,
    tls: &rustls::ClientConfig,
) -> Result<Established, Failure> {
    for text in candidates {
        let candidate = candidates::candidate(text)?;
        if let Ok(selected) = establish(candidate, destination, tls).await {
            return Ok(selected);
        }
    }
    Err(std::io::Error::other(format!(
        "Failed to establish a socket connection to proxies: {}",
        serde_json::to_string(candidates)?
    ))
    .into())
}

/// Stop at the phase the reference considers selected for each route family.
async fn establish(
    candidate: Candidate,
    destination: &url::Url,
    tls: &rustls::ClientConfig,
) -> Result<Established, Failure> {
    match candidate {
        Candidate::Direct => Ok(Established::Socket(tcp(destination).await?, None)),
        Candidate::Socks(agent) => {
            let socket = agent.connect(&destination.as_str().parse()?).await?;
            Ok(Established::Socket(Socket::new(TokioIo::new(socket)), None))
        }
        Candidate::Http(proxy) => {
            let socket = tcp(&proxy).await?;
            if destination.scheme() == "https" {
                let socket = secure(socket, proxy.as_str().parse()?, tls.clone()).await?;
                tunnel(socket, destination, &proxy).await
            } else {
                let authorization = authorization(&proxy)?;
                Ok(Established::Socket(
                    socket,
                    Some(ForwardProxy {
                        endpoint: proxy,
                        authorization,
                    }),
                ))
            }
        }
        Candidate::Unknown => Err(std::io::Error::other("Unrecognized proxy directive").into()),
    }
}

/// Destination TLS is deliberately outside fallback; forwarding-proxy TLS is likewise selected.
async fn complete(
    socket: Socket,
    destination: &url::Url,
    proxy: Option<&ForwardProxy>,
    tls: rustls::ClientConfig,
) -> Result<Socket, Failure> {
    let endpoint = proxy.map_or(destination, |proxy| &proxy.endpoint);
    secure(socket, endpoint.as_str().parse()?, tls).await
}

/// Connect using native DNS without discarding bracketed IPv6 URL identity elsewhere.
async fn tcp(url: &url::Url) -> Result<Socket, Failure> {
    let host = url
        .host_str()
        .ok_or_else(|| std::io::Error::other("Could not determine `host`"))?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let port = url
        .port_or_known_default()
        .ok_or_else(|| std::io::Error::other("Missing endpoint port"))?;
    Ok(Socket::new(TokioIo::new(
        tokio::net::TcpStream::connect((host, port)).await?,
    )))
}

/// The existing HTTP parser owns both successful upgrades and non-200 responses.
async fn tunnel(
    socket: Socket,
    destination: &url::Url,
    proxy: &url::Url,
) -> Result<Established, Failure> {
    let authority = format!(
        "{}:{}",
        destination.host_str().unwrap_or_default(),
        destination.port_or_known_default().unwrap_or(443)
    );
    let mut request = hyper::Request::builder()
        .method("CONNECT")
        .uri(&authority)
        .header("host", &authority)
        .header("proxy-connection", "close");
    if let Some(auth) = authorization(proxy)? {
        request = request.header(hyper::header::PROXY_AUTHORIZATION, auth);
    }
    let request = request.body(reqwest::Body::from(Vec::new()))?;
    let (mut sender, connection) = hyper::client::conn::http1::handshake(socket).await?;
    let mut connection: Driver = Box::pin(connection.with_upgrades().fuse());
    let mut response = driving(&mut connection, sender.send_request(request)).await?;
    if response.status() != hyper::StatusCode::OK {
        return Ok(Established::Refused(
            response.map(|body| proxy_agent::response_body(body, connection)),
        ));
    }
    let upgraded = driving(&mut connection, hyper::upgrade::on(&mut response)).await?;
    Ok(Established::Socket(Socket::new(upgraded), None))
}

/// Decode proxy userinfo once; credentials never enter origin headers or CONNECT bodies.
fn authorization(proxy: &url::Url) -> Result<Option<hyper::header::HeaderValue>, Failure> {
    if proxy.username().is_empty() && proxy.password().is_none() {
        return Ok(None);
    }
    let user = proxy_agent::decode(proxy.username())?;
    let password = proxy_agent::decode(proxy.password().unwrap_or_default())?;
    let value =
        base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}").as_bytes());
    Ok(Some(format!("Basic {value}").parse()?))
}
