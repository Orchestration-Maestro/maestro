//! Retained source compilation and destination-specific ordered route selection.

mod candidates;
mod connect;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use futures_util::FutureExt;
use futures_util::future::{Shared, WeakShared};

use super::get_uri::{self, Cache, Failure};
use super::pac_resolver::Resolver;
use super::{Fetch, HttpRequest, HttpResponse};

/// A successfully compiled source and its matching cache observation.
struct Compiled {
    /// Source text compared without adding a digest dependency.
    code: String,
    /// Observation published only after successful compilation.
    cache: Cache,
    /// Globals belong exclusively to this compiled source.
    resolver: Arc<Resolver>,
}

/// Failed source observation is distinct from code that cannot be compiled.
enum ReloadError {
    /// A transient source failure leaves successfully compiled globals intact.
    Source(Failure),
    /// Invalid replacement code invalidates reuse, including a subsequent unchanged response.
    Compilation(Failure),
}

/// Shared pending load settles for every concurrent caller.
type LoadFuture = crate::BoxFuture<Result<Arc<Compiled>, String>>;

/// Live callers, rather than the cache, own a pending operation.
type Loading = Shared<LoadFuture>;

/// State for one destination protocol and selected source URI.
#[derive(Default)]
struct State {
    /// Last successful source and interpreter.
    compiled: Option<Arc<Compiled>>,
    /// Only concurrent source loads are coalesced; cancellation releases the last live load.
    pending: Option<WeakShared<LoadFuture>>,
}

/// Destination protocol and selected source URI identify one interpreter.
type SourceKey = (String, String);

/// One bridge retains source instances independently of its timeout-keyed senders.
#[derive(Default)]
pub(crate) struct PacProxyAgent {
    /// Destination protocol and source URI pairs; no process-global state or general LRU.
    sources: Mutex<BTreeMap<SourceKey, Arc<Mutex<State>>>>,
}

impl PacProxyAgent {
    /// Load the selected source, evaluate once, and select before transmitting request bytes.
    pub(super) async fn request(
        &self,
        source: &url::Url,
        request: HttpRequest,
        route: &super::client::Route,
        client: &reqwest::Client,
    ) -> Result<HttpResponse, Failure> {
        let uri = url::Url::parse(
            source
                .as_str()
                .strip_prefix("pac+")
                .unwrap_or(source.as_str()),
        )?;
        let state = self
            .sources
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry((
                url::Url::parse(&request.url)?.scheme().into(),
                uri.to_string(),
            ))
            .or_default()
            .clone();
        let compiled = load(state, uri).await.map_err(std::io::Error::other)?;
        let result = compiled.resolver.resolve(&request.url).await?;
        let candidates = candidates::parse(&result);
        connect::request(candidates, request, route, client).await
    }
}

/// Share the pending operation without retaining a self-referential future cycle.
async fn load(state: Arc<Mutex<State>>, uri: url::Url) -> Result<Arc<Compiled>, String> {
    let pending = {
        let mut guard = state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(pending) = guard.pending.as_ref().and_then(WeakShared::upgrade) {
            pending
        } else {
            let pending = reload(&state, uri, guard.compiled.clone());
            guard.pending = pending.downgrade();
            pending
        }
    };
    pending.await
}

/// Pending work borrows its retained owner weakly so cancellation cannot create a cycle.
fn reload(state: &Arc<Mutex<State>>, uri: url::Url, previous: Option<Arc<Compiled>>) -> Loading {
    let weak = Arc::downgrade(state);
    async move {
        let result = compile(uri, previous).await;
        if let Some(state) = weak.upgrade() {
            let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
            match &result {
                Ok(compiled) => state.compiled = Some(Arc::clone(compiled)),
                Err(ReloadError::Compilation(_)) => state.compiled = None,
                Err(ReloadError::Source(_)) => {}
            }
            state.pending = None;
        }
        result.map_err(|error| match error {
            ReloadError::Source(error) | ReloadError::Compilation(error) => {
                super::client::chain(error.as_ref())
            }
        })
    }
    .boxed()
    .shared()
}

/// Source observations and compiled code are committed together by the caller.
async fn compile(
    uri: url::Url,
    previous: Option<Arc<Compiled>>,
) -> Result<Arc<Compiled>, ReloadError> {
    let fetch: Fetch = super::client::configured_fetch(super::client::Route::default());
    let loaded = get_uri::load(&uri, previous.as_ref().map(|old| &old.cache), &fetch).await;
    let source = match loaded {
        Ok(source) => source,
        Err(error) if error.is::<get_uri::notmodified::NotModified>() => {
            return previous.ok_or(ReloadError::Source(error));
        }
        Err(error) => return Err(ReloadError::Source(error)),
    };
    let code = String::from_utf8_lossy(&source.bytes).into_owned();
    let resolver = if let Some(old) = previous.filter(|old| old.code == code) {
        Arc::clone(&old.resolver)
    } else {
        Arc::new(
            Resolver::new(code.clone())
                .await
                .map_err(ReloadError::Compilation)?,
        )
    };
    Ok(Arc::new(Compiled {
        code,
        cache: source.cache,
        resolver,
    }))
}
