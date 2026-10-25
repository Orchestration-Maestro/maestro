//! Request-owned proxy connections with shared destination TLS setup.

use std::error::Error;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_util::FutureExt;
use hyper::body::Body;
use hyper_util::rt::TokioIo;
use tower_service::Service;

use super::client::ProxySelector;
use super::socks_proxy_agent::SocksProxyAgent;

pub(super) mod socket;
use socket::{Driver, Socket, driving};

/// Native connection errors retain causes through the HTTP boundary.
pub(super) type Failure = Box<dyn Error + Send + Sync>;

/// Immutable routing for a request-owned connection.
#[derive(Clone)]
pub(super) struct ProxyAgent {
    /// Selector consulted before connecting to the chosen proxy.
    select: ProxySelector,
}

impl Service<hyper::Uri> for ProxyAgent {
    type Response = TokioIo<tokio::net::TcpStream>;
    type Error = Failure;
    type Future = crate::BoxFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, uri: hyper::Uri) -> Self::Future {
        let select = Arc::clone(&self.select);
        Box::pin(async move {
            let destination = url::Url::parse(&uri.to_string())?;
            let proxy = select(&destination)
                .map_err(std::io::Error::other)?
                .ok_or_else(|| std::io::Error::other("Missing selected SOCKS proxy"))?;
            Ok(TokioIo::new(
                SocksProxyAgent::new(&proxy)?.connect(&uri).await?,
            ))
        })
    }
}

/// Retained configuration, without retaining live request connections.
#[derive(Clone)]
pub(super) struct Sender {
    /// TLS connector shares its configuration with reqwest.
    connector: hyper_rustls::HttpsConnector<ProxyAgent>,
    /// Supplied connection deadline includes destination TLS.
    timeout: Option<std::time::Duration>,
}

impl Sender {
    /// Send once and transfer connection ownership into the response body.
    pub(super) async fn request(
        mut self,
        request: hyper::Request<reqwest::Body>,
    ) -> Result<hyper::Response<reqwest::Body>, Failure> {
        let connection = self.connector.call(request.uri().clone());
        let socket = if let Some(timeout) = self.timeout {
            tokio::time::timeout(timeout, connection)
                .await
                .map_err(|_| ConnectTimeout)??
        } else {
            connection.await?
        };
        exchange(Socket::new(socket), request, false).await
    }
}

/// Send exactly once after route establishment, sharing conversion and driver ownership.
pub(super) async fn exchange(
    socket: Socket,
    mut request: hyper::Request<reqwest::Body>,
    absolute: bool,
) -> Result<hyper::Response<reqwest::Body>, Failure> {
    let host = request
        .uri()
        .authority()
        .ok_or_else(|| std::io::Error::other("Missing request authority"))?
        .as_str()
        .parse::<hyper::header::HeaderValue>()?;
    request
        .headers_mut()
        .entry(hyper::header::HOST)
        .or_insert(host);
    if !absolute {
        *request.uri_mut() = request
            .uri()
            .path_and_query()
            .map_or("/", |path| path.as_str())
            .parse()?;
    }
    let (mut sender, connection) = hyper::client::conn::http1::handshake(socket).await?;
    let mut connection: Driver = Box::pin(connection.fuse());
    let response = driving(&mut connection, sender.send_request(request)).await?;
    Ok(response.map(|body| response_body(body, connection)))
}

/// Drive the same connection while consuming its body, so dropping either phase closes it.
pub(super) fn response_body(mut body: hyper::body::Incoming, connection: Driver) -> reqwest::Body {
    let connection = std::sync::Mutex::new(Some(connection));
    let stream = futures_util::stream::poll_fn(move |context| {
        let mut connection = connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if connection
            .as_mut()
            .is_some_and(|connection| connection.as_mut().poll(context).is_ready())
        {
            *connection = None;
        }
        Pin::new(&mut body).poll_frame(context)
    });
    reqwest::Body::wrap(http_body_util::StreamBody::new(stream))
}

/// Retain configuration, not requests or sockets.
pub(super) fn sender(
    select: ProxySelector,
    tls: rustls::ClientConfig,
    timeout: Option<std::time::Duration>,
) -> Sender {
    Sender {
        connector: hyper_rustls::HttpsConnector::from((ProxyAgent { select }, tls)),
        timeout,
    }
}

/// Platform trust and route ALPN shared through reqwest's preconfigured TLS API.
pub(super) fn tls(http1_only: bool) -> Result<rustls::ClientConfig, Failure> {
    let provider = rustls::crypto::CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::aws_lc_rs::default_provider()));
    let verifier = rustls_platform_verifier::Verifier::new(Arc::clone(&provider))?;
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    config.alpn_protocols = if http1_only {
        vec![b"http/1.1".to_vec()]
    } else {
        vec![b"h2".to_vec(), b"http/1.1".to_vec()]
    };
    Ok(config)
}

/// Supplied deadline, distinct from the SOCKS handshake timer.
#[derive(Debug)]
pub(super) struct ConnectTimeout;
impl std::fmt::Display for ConnectTimeout {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Connection deadline elapsed")
    }
}
impl Error for ConnectTimeout {}

/// Decode one URI component strictly, without form-style plus replacement.
pub(super) fn decode(value: &str) -> std::io::Result<String> {
    let bytes = value.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'%'
            && !bytes
                .get(index + 1..index + 3)
                .is_some_and(|digits| digits.iter().all(u8::is_ascii_hexdigit))
        {
            return Err(std::io::Error::other("Invalid percent escape"));
        }
    }
    percent_encoding::percent_decode_str(value)
        .decode_utf8()
        .map(std::borrow::Cow::into_owned)
        .map_err(std::io::Error::other)
}

#[cfg(test)]
pub(super) mod tests;
