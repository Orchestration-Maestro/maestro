//! Native HTTP resources live on the existing local capability owner.
use super::{
    Admission, Capability, Lease, Registry, SignalEndpoint, held::NativeResponse,
    native_resources::Reader, pump::State,
};
use maestro_extensions::native_io::{self, IoFuture};
use std::sync::Arc;
use tokio::sync::oneshot;

/// Store-safe ownership of one original response.
#[derive(Clone)]
pub struct Response(pub(crate) Arc<Lease>);
impl Response {
    /// Reads the native status without consuming the body.
    pub(crate) fn status(&self) -> Result<u16, String> {
        self.0
            .run::<NativeResponse, _>(|response, _| Ok(response.status()))
    }
    /// Reads the original native reason phrase.
    pub(crate) fn status_text(&self) -> Result<String, String> {
        self.0
            .run::<NativeResponse, _>(|response, _| Ok(response.status_text()))
    }
    /// Reads the normalized response header pairs.
    pub(crate) fn headers(&self) -> Result<Vec<(String, String)>, String> {
        self.0
            .run::<NativeResponse, _>(|response, _| Ok(response.headers()))
    }
    /// Acquires the original consuming body with independent resource ownership.
    pub(crate) fn body(&self) -> Result<Option<Reader>, String> {
        self.0.run::<NativeResponse, _>(|response, registry| {
            response
                .body()
                .map(|body| registry.reader(body))
                .transpose()
        })
    }
}
impl Response {
    /// Collects the original body through its native response owner.
    pub(crate) async fn text(&self) -> Result<String, String> {
        self.0
            .observe::<NativeResponse, _>(|response, _| {
                let response = response.clone();
                Ok(Box::pin(async move { response.text().await }) as IoFuture<'static, _>)
            })
            .await
    }
    /// Parses the original body and uses the lossless JSON wire.
    pub(crate) async fn json(&self) -> Result<String, String> {
        self.0
            .observe::<NativeResponse, _>(|response, _| {
                let response = response.clone();
                Ok(Box::pin(async move {
                    let value = response.json().await?;
                    serde_json::to_string(&value).map_err(|error| error.to_string())
                }) as IoFuture<'static, _>)
            })
            .await
    }
}
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
/// Operations on the HTTP owner that need no response capability.
pub(crate) enum Request {
    /// Starts one attempt through supplied domain capabilities.
    Fetch(Fetch, oneshot::Sender<Result<Response, String>>, Admission),
}
impl State {
    /// Serves HTTP operations after releasing the capability-table borrow.
    pub(super) fn http(&self, request: Request) {
        let Request::Fetch(fetch, reply, admission) = request;
        let registry = self.registry.clone();
        self.admit(
            super::native_resources::consuming(
                Box::pin(async move { fetch_response(&registry, fetch).await }),
                reply,
            ),
            admission,
        );
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
