//! Native HTTP resources live on the existing local capability owner.
use super::{
    Admission, Capability, Lease, Registry, SignalEndpoint, native_resources::Reader, pump::State,
};
use maestro_extensions::native_io;
use std::sync::{Arc, mpsc::Sender};
use tokio::sync::oneshot;

/// Store-safe ownership of one original response.
#[derive(Clone)]
pub(crate) struct Response(pub(crate) Arc<Lease>);
/// Supplied HTTP operands with a retained original signal endpoint.
pub(crate) struct Fetch {
    /// Literal destination.
    pub(crate) url: String,
    /// Omitted selects GET.
    pub(crate) method: Option<String>,
    /// Native HTTP normalization applies to these pairs.
    pub(crate) headers: Vec<(String, String)>,
    /// Absent and present-empty stay distinct.
    pub(crate) body: Option<Vec<u8>>,
    /// Original caller cancellation.
    pub(crate) signal: Option<SignalEndpoint>,
}
/// Operations on the same retained response owner.
pub(crate) enum Request {
    /// Starts one attempt through supplied domain capabilities.
    Fetch(Fetch, oneshot::Sender<Result<Response, String>>, Admission),
    /// Parses the original body and uses the lossless JSON wire.
    Json(oneshot::Sender<Result<String, String>>, Admission),
    /// Collects the original body through its native response owner.
    Text(oneshot::Sender<Result<String, String>>, Admission),
    /// Reads the native status without consuming the body.
    Status(Sender<Result<u16, String>>),
    /// Original native reason phrase.
    StatusText(Sender<Result<String, String>>),
    /// Normalized response header pairs.
    Headers(Sender<Result<Vec<(String, String)>, String>>),
    /// Original consuming body with independent resource ownership.
    Body(Sender<Result<Option<Reader>, String>>),
}
impl State {
    /// Serves HTTP operations after releasing the capability-table borrow.
    pub(super) fn http(&self, id: u64, request: Request) {
        match request {
            Request::Fetch(fetch, reply, admission) => {
                let registry = self.registry.clone();
                self.admit(
                    Box::pin(async move {
                        let result = fetch_response(&registry, fetch).await;
                        let _ = reply.send(result);
                    }),
                    admission,
                );
            }
            request => self.response(id, request),
        }
    }
    /// Reads response metadata without borrowing the table during native calls.
    fn response(&self, id: u64, request: Request) {
        let capability = self.registry.capability(id);
        let response = match capability.as_deref() {
            Some(Capability::NativeResponse(response)) => Ok(response),
            _ => Err("native response resource ended".to_owned()),
        };
        let json = matches!(&request, Request::Json(..));
        match request {
            Request::Text(reply, admission) | Request::Json(reply, admission) => {
                let response = response.cloned();
                self.admit(
                    super::native_resources::consuming(
                        Box::pin(async move { consume(response?, json).await }),
                        reply,
                    ),
                    admission,
                );
            }
            Request::Status(reply) => {
                let _ = reply.send(response.map(|response| response.status()));
            }
            Request::StatusText(reply) => {
                let _ = reply.send(response.map(|response| response.status_text()));
            }
            Request::Headers(reply) => {
                let _ = reply.send(response.map(|response| response.headers()));
            }
            Request::Body(reply) => {
                let result = response.and_then(|response| {
                    response
                        .body()
                        .map(|body| self.registry.reader(body))
                        .transpose()
                });
                let _ = reply.send(result);
            }
            Request::Fetch(_, reply, _) => {
                let _ = reply.send(Err("operation requires native I/O".into()));
            }
        }
    }
}

/// Preserves native consumption failures and encodes only successful JSON values.
async fn consume(
    response: std::rc::Rc<dyn native_io::Response>,
    json: bool,
) -> Result<String, String> {
    if json {
        let value = response.json().await?;
        serde_json::to_string(&value).map_err(|error| error.to_string())
    } else {
        response.text().await
    }
}

/// Resolves the original signal and response outside every capability-table borrow.
async fn fetch_response(registry: &Registry, fetch: Fetch) -> Result<Response, String> {
    let io = registry
        .resources
        .borrow()
        .native_io
        .clone()
        .ok_or("native I/O capability unavailable")?;
    let signal = fetch
        .signal
        .as_ref()
        .map(|signal| registry.cancellation(signal))
        .transpose()?;
    let response = io
        .fetch(native_io::Request {
            url: fetch.url,
            method: fetch.method,
            headers: fetch.headers,
            body: fetch.body,
            signal,
        })
        .await?;
    registry
        .insert(Capability::NativeResponse(response))
        .map(Response)
}
