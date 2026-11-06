//! Ephemeral loopback HTTP for the driver's own packaged fixtures.

use super::resources;
use http_body_util::Full;
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use std::{
    convert::Infallible,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinSet};

/// An exclusively bound loopback fixture endpoint.
pub struct Server {
    /// HTTP origin used by page navigation and request interception.
    pub origin: String,
    /// Listener shutdown signal.
    stop: oneshot::Sender<()>,
    /// Owns the listener and every accepted connection.
    task: tokio::task::JoinHandle<io::Result<()>>,
}

impl Server {
    /// Canonicalize the package before admitting a request.
    ///
    /// # Errors
    /// Returns root-resolution or loopback binding failures.
    pub async fn start(root: &Path) -> io::Result<Self> {
        let root = root.to_path_buf();
        let root = Arc::new(
            tokio::task::spawn_blocking(move || maestro_realpath::canonicalize(root))
                .await
                .map_err(io::Error::other)??,
        );
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let origin = format!("http://{}", listener.local_addr()?);
        let (stop, mut stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    _ = &mut stopped => break,
                    result = listener.accept() => {
                        let (socket, _) = result?;
                        let root = Arc::clone(&root);
                        connections.spawn(async move {
                            let service = service_fn(move |request| respond(Arc::clone(&root), request));
                            if let Err(error) = http1::Builder::new().serve_connection(TokioIo::new(socket), service).await {
                                eprintln!("HTTP {error}");
                            }
                        });
                    }
                    Some(result) = connections.join_next(), if !connections.is_empty() => { result.map_err(io::Error::other)?; }
                }
            }
            connections.shutdown().await;
            Ok(())
        });
        Ok(Self { origin, stop, task })
    }

    /// Stop acceptance and close every retained socket before returning.
    ///
    /// # Errors
    /// Returns listener or connection-worker failures.
    pub async fn close(self) -> io::Result<()> {
        // A failed signal means the listener has already stopped; its join carries the error.
        let _ = self.stop.send(());
        self.task.await.map_err(io::Error::other)?
    }
}

/// Read packaged bytes off the runtime thread and apply the fixture's HTTP headers.
async fn respond(
    root: Arc<PathBuf>,
    request: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = request
        .uri()
        .path_and_query()
        .map_or("/", |value| value.as_str())
        .to_owned();
    let resource = tokio::task::spawn_blocking(move || resources::read(&root, &path)).await;
    let mut response = Response::new(Full::new(Bytes::new()));
    response.headers_mut().insert(
        "access-control-allow-origin",
        hyper::header::HeaderValue::from_static("*"),
    );
    match resource {
        Ok(resource) => {
            *response.status_mut() = resource.status;
            if let Some(mime) = resource.mime {
                response.headers_mut().insert(
                    "content-type",
                    hyper::header::HeaderValue::from_static(mime),
                );
            }
            if let Some(policy) = resource.policy {
                response.headers_mut().insert(
                    "content-security-policy",
                    hyper::header::HeaderValue::from_static(policy),
                );
            }
            *response.body_mut() = Full::new(Bytes::from(resource.bytes));
        }
        Err(error) => {
            eprintln!("HTTP resource worker: {error}");
            *response.status_mut() = hyper::StatusCode::INTERNAL_SERVER_ERROR;
        }
    }
    Ok(response)
}
