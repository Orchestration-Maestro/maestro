//! Per-session socket counters and the fallback flag; none of it needs native I/O.

#[cfg(not(target_arch = "wasm32"))]
use super::continuation::Request;
use crate::{DiagnosticInput, format_thrown_value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// What one session's requests did; optional fields stay absent until a request or a recorded
/// outcome sets them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct OpenAICodexWebSocketDebugStats {
    /// Selected request attempts, counted before the send.
    pub(crate) requests: u64,
    /// Requests that opened a connection.
    pub(crate) connections_created: u64,
    /// Requests that reused an idle cached connection.
    pub(crate) connections_reused: u64,
    /// Requests that selected a cached-context transport.
    pub(crate) cached_context_requests: u64,
    /// Requests whose body carried `store: true`.
    pub(crate) store_true_requests: u64,
    /// Requests sent without a previous response identifier.
    pub(crate) full_context_requests: u64,
    /// Requests sent with a previous response identifier.
    pub(crate) delta_requests: u64,
    /// Input length of the last request: array items, or UTF-16 units of a text input.
    pub(crate) last_input_items: usize,
    /// Input length of the last delta request, absent after a full request.
    pub(crate) last_delta_input_items: Option<usize>,
    /// Previous response identifier of the last delta request, absent after a full request.
    pub(crate) last_previous_response_id: Option<String>,
    /// Recorded socket failures.
    pub(crate) websocket_failures: u64,
    /// Recorded fallbacks to the other transport.
    pub(crate) sse_fallbacks: u64,
    /// Fallback state at the last fallback or failure record.
    pub(crate) websocket_fallback_active: Option<bool>,
    /// Text of the last recorded failure.
    pub(crate) last_web_socket_error: Option<String>,
}

/// How a request obtained its connection and which transport it selected.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
pub(super) struct Mode {
    /// An idle cached connection served the request.
    pub(super) reused: bool,
    /// The cached-context transport was selected.
    pub(super) cached: bool,
}

/// Statistics and fallback flags, keyed by session.
struct DebugState {
    /// Counters of sessions that selected a request or recorded an outcome.
    stats: BTreeMap<String, OpenAICodexWebSocketDebugStats>,
    /// Sessions whose socket failed.
    fallback: BTreeSet<String>,
}

/// Process-wide debug state.
static DEBUG: Mutex<DebugState> = Mutex::new(DebugState {
    stats: BTreeMap::new(),
    fallback: BTreeSet::new(),
});

/// Lock the debug state; a poisoned lock is used as it is.
fn lock() -> MutexGuard<'static, DebugState> {
    DEBUG.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Count one selected request attempt before it is sent.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn count_request(session_id: &str, request: &Request, mode: Mode) {
    let mut guard = lock();
    let stats = guard.stats.entry(session_id.to_owned()).or_default();
    stats.requests += 1;
    if mode.reused {
        stats.connections_reused += 1;
    } else {
        stats.connections_created += 1;
    }
    stats.cached_context_requests += u64::from(mode.cached);
    stats.store_true_requests += u64::from(request.store_true);
    stats.last_input_items = request.input_items;
    if request.previous_response_id.is_some() {
        stats.delta_requests += 1;
        stats.last_delta_input_items = Some(request.input_items);
        stats
            .last_previous_response_id
            .clone_from(&request.previous_response_id);
    } else {
        stats.full_context_requests += 1;
        stats.last_delta_input_items = None;
        stats.last_previous_response_id = None;
    }
}

/// An independent copy of the session's statistics, if any request or record created them.
pub(crate) fn get_openai_codex_web_socket_debug_stats(
    session_id: &str,
) -> Option<OpenAICodexWebSocketDebugStats> {
    lock().stats.get(session_id).cloned()
}

/// Clear statistics and fallback state of one session, or of every session when none or an
/// empty one is named. Connections are not touched.
pub(crate) fn reset_openai_codex_web_socket_debug_stats(session_id: Option<&str>) {
    let mut state = lock();
    if let Some(id) = session_id.filter(|id| !id.is_empty()) {
        state.stats.remove(id);
        state.fallback.remove(id);
    } else {
        state.stats.clear();
        state.fallback.clear();
    }
}

/// Whether the session's socket failed since its state was last reset.
pub(crate) fn is_web_socket_sse_fallback_active(session_id: Option<&str>) -> bool {
    session_id
        .filter(|id| !id.is_empty())
        .is_some_and(|id| lock().fallback.contains(id))
}

/// Count a fallback and copy the session's current fallback state, which may be false.
pub(crate) fn record_web_socket_sse_fallback(session_id: Option<&str>) {
    let Some(id) = session_id.filter(|id| !id.is_empty()) else {
        return;
    };
    let mut guard = lock();
    let DebugState { stats, fallback } = &mut *guard;
    let stats = stats.entry(id.to_owned()).or_default();
    stats.sse_fallbacks += 1;
    stats.websocket_fallback_active = Some(fallback.contains(id));
}

/// Activate the session's fallback and record the failure text.
pub(crate) fn record_web_socket_failure(session_id: Option<&str>, error: DiagnosticInput<'_>) {
    let Some(id) = session_id.filter(|id| !id.is_empty()) else {
        return;
    };
    let text = format_thrown_value(error);
    let mut guard = lock();
    guard.fallback.insert(id.to_owned());
    let stats = guard.stats.entry(id.to_owned()).or_default();
    stats.websocket_failures += 1;
    stats.last_web_socket_error = Some(text);
    stats.websocket_fallback_active = Some(true);
}
