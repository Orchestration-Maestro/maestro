//! Native listener admission and owned shutdown.
#[cfg(not(target_arch = "wasm32"))]
use super::callback::{AuthorizationCode, CallbackResponse};
#[cfg(not(target_arch = "wasm32"))]
use crate::OAuthError;
use crate::{Cancellation, EventStream};

/// First outcome of the callback wait.
pub(super) enum CallbackOutcome {
    /// Accepted authorization.
    #[cfg(not(target_arch = "wasm32"))]
    Accepted(AuthorizationCode),
    /// The wait was cancelled.
    Cancelled,
    /// The listener failed after binding.
    #[cfg(not(target_arch = "wasm32"))]
    Failed(OAuthError),
}

/// Callback settlement with owned shutdown signals.
pub(super) struct CallbackServer {
    /// First callback/cancellation/error outcome.
    pub wait: EventStream<CallbackOutcome, ()>,
    /// Requests shutdown when an accepting listener exists.
    pub(super) stop: Cancellation,
    /// Completes once no accepting listener is owned.
    pub(super) stopped: EventStream<(), ()>,
}

impl CallbackServer {
    /// Construct callback settlement and shutdown signals.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn new() -> Self {
        Self {
            wait: EventStream::new(|_| true, |_| ()),
            stop: Cancellation::new(),
            stopped: EventStream::new(|()| true, |()| ()),
        }
    }
    /// Request admission shutdown and await its completion witness, not peer drain.
    pub(super) async fn close(&self) {
        self.stop.abort();
        self.stopped.result().await;
    }
}

impl Drop for CallbackServer {
    fn drop(&mut self) {
        self.stop.abort();
    }
}

/// Select the binding host without changing redirect identity.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn callback_host(value: Option<String>) -> String {
    value
        .filter(|host| !host.is_empty())
        .unwrap_or_else(|| "127.0.0.1".into())
}

/// Bind the native accepting listener.
#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn bind(
    state: String,
    port: u16,
    route: fn(&str, &str) -> CallbackResponse,
) -> Result<CallbackServer, OAuthError> {
    bind_with(
        state,
        port,
        route,
        |key| std::env::var(key).ok(),
        native_accept,
    )
    .await
}

/// One borrowing native accept step, replaceable at the effect boundary.
#[cfg(not(target_arch = "wasm32"))]
pub(super) type Accept<'a> = std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = std::io::Result<(tokio::net::TcpStream, std::net::SocketAddr)>,
            > + Send
            + 'a,
    >,
>;

/// Accept a real native peer without transferring listener ownership.
#[cfg(not(target_arch = "wasm32"))]
fn native_accept(listener: &tokio::net::TcpListener) -> Accept<'_> {
    Box::pin(listener.accept())
}

/// Bind with caller-provided environment lookup and an owning accept loop.
#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn bind_with(
    state: String,
    port: u16,
    route: fn(&str, &str) -> CallbackResponse,
    environment: impl FnOnce(&str) -> Option<String>,
    accept: impl for<'a> Fn(&'a tokio::net::TcpListener) -> Accept<'a> + Send + 'static,
) -> Result<CallbackServer, OAuthError> {
    let host = callback_host(environment("MAESTRO_OAUTH_CALLBACK_HOST"));
    let listener = bind_host(&host, port).await?;
    Ok(listen(listener, state, route, accept))
}

/// Own the accepting socket until stop or an accept failure is published.
#[cfg(not(target_arch = "wasm32"))]
fn listen(
    listener: tokio::net::TcpListener,
    state: String,
    route: fn(&str, &str) -> CallbackResponse,
    accept: impl for<'a> Fn(&'a tokio::net::TcpListener) -> Accept<'a> + Send + 'static,
) -> CallbackServer {
    let server = CallbackServer::new();
    let wait = server.wait.clone();
    let stop = server.stop.clone();
    let stopped = server.stopped.clone();
    let (shutdown, peers) = tokio::sync::watch::channel(());
    tokio::spawn(async move {
        loop {
            let crate::providers::http::Raced::Done(accepted) =
                crate::providers::http::race(accept(&listener), None, Some(&stop)).await
            else {
                break;
            };
            match accepted {
                Ok((stream, _)) => serve(stream, state.clone(), wait.clone(), peers.clone(), route),
                Err(error) => {
                    wait.push(CallbackOutcome::Failed(io_error(&error)));
                    break;
                }
            }
        }
        shutdown.send_replace(());
        drop(listener);
        stopped.push(());
    });
    server
}

/// Bind the configured host, enabling address reuse on Unix.
#[cfg(not(target_arch = "wasm32"))]
async fn bind_host(host: &str, port: u16) -> Result<tokio::net::TcpListener, OAuthError> {
    let address = tokio::net::lookup_host((host, port))
        .await
        .map_err(|error| io_error(&error))?
        .next()
        .ok_or_else(|| OAuthError::message("No callback host address"))?;
    let socket = if address.is_ipv4() {
        tokio::net::TcpSocket::new_v4()
    } else {
        tokio::net::TcpSocket::new_v6()
    }
    .map_err(|error| io_error(&error))?;
    #[cfg(unix)]
    socket
        .set_reuseaddr(true)
        .map_err(|error| io_error(&error))?;
    socket.bind(address).map_err(|error| io_error(&error))?;
    socket.listen(128).map_err(|error| io_error(&error))
}

/// Supply the real native error number without inventing symbolic codes or stacks.
#[cfg(not(target_arch = "wasm32"))]
fn io_error(error: &std::io::Error) -> OAuthError {
    let mut failure = OAuthError::message(error.to_string());
    failure.errno = error
        .raw_os_error()
        .map(|errno| crate::DiagnosticCode::Number(f64::from(errno)));
    failure
}

/// Serve an admitted peer independently so committed responses can finish after listener stop.
#[cfg(not(target_arch = "wasm32"))]
fn serve(
    stream: tokio::net::TcpStream,
    state: String,
    wait: EventStream<CallbackOutcome, ()>,
    mut shutdown: tokio::sync::watch::Receiver<()>,
    route: fn(&str, &str) -> CallbackResponse,
) {
    use http_body_util::Full;
    use hyper::{
        Request, Response,
        body::{Bytes, Incoming},
        service::service_fn,
    };
    use std::convert::Infallible;
    tokio::spawn(async move {
        let service = service_fn(move |request: Request<Incoming>| {
            let response = route(&request.uri().to_string(), &state);
            if let Some(code) = response.accepted {
                wait.push(CallbackOutcome::Accepted(code));
            }
            let mut outgoing = Response::new(Full::new(Bytes::from(response.body)));
            *outgoing.status_mut() = hyper::StatusCode::from_u16(response.status)
                .unwrap_or(hyper::StatusCode::INTERNAL_SERVER_ERROR);
            outgoing.headers_mut().insert(
                hyper::header::CONTENT_TYPE,
                hyper::header::HeaderValue::from_static(response.content_type),
            );
            async move { Ok::<_, Infallible>(outgoing) }
        });
        let connection = hyper::server::conn::http1::Builder::new()
            .serve_connection(hyper_util::rt::TokioIo::new(stream), service);
        tokio::pin!(connection);
        if matches!(
            futures_util::future::select(Box::pin(shutdown.changed()), connection.as_mut()).await,
            futures_util::future::Either::Right(_)
        ) {
            return;
        }
        connection.as_mut().graceful_shutdown();
        connection.await.ok();
    });
}
