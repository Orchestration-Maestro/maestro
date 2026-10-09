//! Native listener admission and owned shutdown.
#[cfg(not(target_arch = "wasm32"))]
use super::callback::{AuthorizationCode, route};
use crate::{Cancellation, EventStream, OAuthError};

/// First outcome of the callback wait.
pub(super) enum CallbackOutcome {
    /// Accepted authorization.
    #[cfg(not(target_arch = "wasm32"))]
    Accepted(AuthorizationCode),
    /// The wait was cancelled by pasted input.
    Cancelled,
    /// The listener failed after binding.
    #[cfg(not(target_arch = "wasm32"))]
    Failed(OAuthError),
}

/// One listener with a single settlement owner and a stopped witness.
pub(super) struct CallbackServer {
    /// First callback/cancellation/error outcome.
    pub wait: EventStream<CallbackOutcome, ()>,
    /// Requests accepting-listener shutdown.
    pub(super) stop: Cancellation,
    /// Completes only after the accepting listener is dropped.
    pub(super) stopped: EventStream<(), ()>,
}

impl CallbackServer {
    /// Construct an owned listener lifecycle.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn new() -> Self {
        Self {
            wait: EventStream::new(|_| true, |_| ()),
            stop: Cancellation::new(),
            stopped: EventStream::new(|()| true, |()| ()),
        }
    }
    /// Stop admission and observe listener release, without claiming peer drain.
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

/// Bind the native accepting listener and publish failures to its active wait.
#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn bind(state: String) -> Result<CallbackServer, OAuthError> {
    let host = callback_host(std::env::var("MAESTRO_OAUTH_CALLBACK_HOST").ok());
    let listener = bind_host(&host).await?;
    let server = CallbackServer::new();
    let wait = server.wait.clone();
    let stop = server.stop.clone();
    let stopped = server.stopped.clone();
    tokio::spawn(async move {
        loop {
            let crate::providers::http::Raced::Done(accepted) =
                crate::providers::http::race(listener.accept(), None, Some(&stop)).await
            else {
                break;
            };
            match accepted {
                Ok((stream, _)) => serve(stream, state.clone(), wait.clone()),
                Err(error) => {
                    wait.push(CallbackOutcome::Failed(io_error(&error)));
                    break;
                }
            }
        }
        drop(listener);
        stopped.push(());
    });
    Ok(server)
}

/// Bind the configured host, enabling address reuse on Unix.
#[cfg(not(target_arch = "wasm32"))]
async fn bind_host(host: &str) -> Result<tokio::net::TcpListener, OAuthError> {
    let address = tokio::net::lookup_host((host, 53692))
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

/// Reject binding only at the browser effect boundary.
#[cfg(target_arch = "wasm32")]
pub(super) async fn bind(state: String) -> Result<CallbackServer, OAuthError> {
    unavailable(state).await
}

/// Refuse the unavailable native binding at its effect boundary.
#[cfg(any(test, target_arch = "wasm32"))]
pub(super) async fn unavailable(_state: String) -> Result<CallbackServer, OAuthError> {
    Err(OAuthError::message(
        "Anthropic OAuth requires a native callback listener",
    ))
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
fn serve(stream: tokio::net::TcpStream, state: String, wait: EventStream<CallbackOutcome, ()>) {
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
        hyper::server::conn::http1::Builder::new()
            .serve_connection(hyper_util::rt::TokioIo::new(stream), service)
            .await
            .ok();
    });
}
