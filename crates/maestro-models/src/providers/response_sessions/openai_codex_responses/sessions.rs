//! Cached native sockets: identity-bound entries, busy leases and ownership of idle sockets.

use super::CodexError;
use super::continuation::Continuation;
use super::websocket::close_now;
use futures_util::{FutureExt, StreamExt};
use indexmap::IndexMap;
use std::collections::BTreeMap;
use std::future::{Future, poll_fn};
use std::pin::{Pin, pin};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{Instant, Sleep, sleep_until};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

/// The authenticated native socket.
pub(super) type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// How long a released socket stays cached.
const IDLE_TTL: Duration = Duration::from_millis(300_000);

/// Cached entries by session.
static ENTRIES: Mutex<BTreeMap<String, Arc<Entry>>> = Mutex::new(BTreeMap::new());

/// Lock a mutex whose protected values are replaced in single steps.
pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Why a socket is closed; the text is the close frame's reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CloseReason {
    /// Ordinary release or invalidation.
    Done,
    /// Explicit close of the session.
    DebugClose,
    /// Five idle minutes elapsed.
    IdleTimeout,
}

impl CloseReason {
    /// The close frame reason text.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::DebugClose => "debug_close",
            Self::IdleTimeout => "idle_timeout",
        }
    }
}

/// The endpoint and every effective header, which together decide whether a socket is reusable.
#[derive(PartialEq, Eq)]
pub(super) struct Identity {
    /// Resolved socket endpoint.
    url: String,
    /// Header values by lower-cased name.
    headers: BTreeMap<String, String>,
}

impl Identity {
    /// Identify a connection by its endpoint and the headers it is opened with.
    pub(super) fn new(url: &str, headers: &IndexMap<String, String>) -> Self {
        Self {
            url: url.to_owned(),
            headers: headers
                .iter()
                .map(|(name, value)| (name.to_ascii_lowercase(), value.clone()))
                .collect(),
        }
    }
}

/// A session's cached connection; the cache and every lease share it.
pub(super) struct Entry {
    /// Endpoint and headers the connection was opened with.
    identity: Identity,
    /// Session key of this retained entry.
    session: String,
    /// Idle ownership, active ownership or closure.
    state: Mutex<State>,
    /// The close request, kept even while nothing listens.
    close: watch::Sender<Option<CloseReason>>,
    /// Context of the last completed request.
    pub(super) continuation: Mutex<Option<Continuation>>,
}

/// Ownership of a cached socket.
enum State {
    /// An invocation owns the socket.
    Busy,
    /// The entry owns the socket and its scoped observer.
    Idle(Idle),
    /// No socket can be claimed.
    Closed,
}

/// Entry-owned idle resources with no strong entry back-reference.
struct Idle {
    /// The idle socket, closed when retired or its entry is destroyed.
    socket: Box<Socket>,
    /// Polls only this idle registration.
    task: JoinHandle<()>,
}

impl Drop for Entry {
    fn drop(&mut self) {
        if let State::Idle(idle) = &mut *lock(&self.state) {
            idle.task.abort();
            close_now(&mut idle.socket, CloseReason::Done);
        }
    }
}

/// Remove the session's slot only while it still holds `entry`, which the caller keeps alive.
fn remove_current(entry: &Arc<Entry>) {
    let session = &entry.session;
    let mut entries = lock(&ENTRIES);
    if entries
        .get(session)
        .is_some_and(|current| Arc::ptr_eq(current, entry))
    {
        entries.remove(session);
    }
}

/// An operation's ownership of one socket and, when cached, of its entry. Dropping it attempts
/// to send the close frame, with the session's requested reason if it has one (see
/// [`close_now`]), and removes its still-current cache slot.
pub(super) struct Lease {
    /// The open socket.
    pub(super) socket: Option<Box<Socket>>,
    /// Reason sent when the session has no close request.
    reason: CloseReason,
    /// Captured cache identity, removed when the lease is dropped.
    entry: Option<Arc<Entry>>,
    /// The socket came from the idle cache.
    pub(super) reused: bool,
}

impl Drop for Lease {
    fn drop(&mut self) {
        let requested = self.entry.as_ref().and_then(|entry| *entry.close.borrow());
        if let Some(socket) = &mut self.socket {
            close_now(socket, requested.unwrap_or(self.reason));
        }
        if let Some(entry) = &self.entry {
            remove_current(entry);
        }
    }
}

impl Lease {
    /// A socket the cache does not own.
    fn uncached(socket: Box<Socket>) -> Self {
        Self {
            socket: Some(socket),
            reason: CloseReason::Done,
            entry: None,
            reused: false,
        }
    }

    /// The context slot of the cached entry.
    pub(super) fn continuation(&self) -> Option<&Mutex<Option<Continuation>>> {
        self.entry.as_ref().map(|entry| &entry.continuation)
    }

    /// Subscribe to retained close requests, if this is a cached lease.
    pub(super) fn closed(&self) -> Option<watch::Receiver<Option<CloseReason>>> {
        self.entry.as_ref().map(|entry| entry.close.subscribe())
    }

    /// Park a healthy cached socket for five minutes from release, unless already closed.
    pub(super) fn keep(mut self) {
        let Some(entry) = self.entry.take() else {
            return;
        };
        let mut state = lock(&entry.state);
        if let Some(reason) = *entry.close.borrow() {
            self.reason = reason;
            return;
        }
        let Some(socket) = self.socket.take() else {
            return;
        };
        let owner = Arc::downgrade(&entry);
        let deadline = Instant::now() + IDLE_TTL;
        let task = tokio::spawn(async move {
            let mut timer = pin!(sleep_until(deadline));
            poll_fn(|context| poll_idle(&owner, timer.as_mut(), context)).await;
        });
        *state = State::Idle(Idle { socket, task });
    }
}

/// Poll only the currently registered idle owner, retiring it before identity-safe removal.
fn poll_idle(
    owner: &Weak<Entry>,
    mut timer: Pin<&mut Sleep>,
    context: &mut Context<'_>,
) -> Poll<()> {
    let Some(entry) = owner.upgrade() else {
        return Poll::Ready(());
    };
    let mut state = lock(&entry.state);
    let State::Idle(idle) = &mut *state else {
        return Poll::Ready(());
    };
    if idle.task.id() != tokio::task::id() {
        return Poll::Ready(());
    }
    let reason = if timer.as_mut().poll(context).is_ready() {
        CloseReason::IdleTimeout
    } else {
        loop {
            match idle.socket.poll_next_unpin(context) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(Message::Close(_)) | Err(_)) | None) => break,
                Poll::Ready(Some(Ok(_))) => {}
            }
        }
        CloseReason::Done
    };
    close_now(&mut idle.socket, reason);
    *state = State::Closed;
    drop(state);
    remove_current(&entry);
    Poll::Ready(())
}

/// Discard ready idle frames and detect an already-readable peer close.
fn ended(socket: &mut Socket) -> bool {
    loop {
        match socket.next().now_or_never() {
            None => return false,
            Some(Some(Ok(Message::Close(_)) | Err(_)) | None) => return true,
            Some(Some(Ok(_))) => {}
        }
    }
}

/// What the cache holds for a session when an operation asks.
pub(super) enum Claim {
    /// A matching idle socket, synchronously owned by the invocation.
    Idle(Lease),
    /// An operation owns the entry.
    Busy,
    /// Nothing usable remains.
    Absent,
}

/// Claim idle ownership under the lock; busy and closed entries remain distinct.
pub(super) fn claim(session: &str, identity: &Identity) -> Claim {
    let mut entries = lock(&ENTRIES);
    let Some(entry) = entries.get(session).cloned() else {
        return Claim::Absent;
    };
    let mut state = lock(&entry.state);
    if matches!(*state, State::Busy) {
        return Claim::Busy;
    }
    if let State::Idle(mut idle) = std::mem::replace(&mut *state, State::Closed) {
        idle.task.abort();
        if entry.identity == *identity && !ended(&mut idle.socket) {
            *state = State::Busy;
            let mut lease = Lease::uncached(idle.socket);
            lease.entry = Some(Arc::clone(&entry));
            lease.reused = true;
            return Claim::Idle(lease);
        }
        close_now(&mut idle.socket, CloseReason::Done);
    }
    entries.remove(session);
    Claim::Absent
}

/// Publish a freshly connected socket unless another operation published first.
pub(super) fn publish(session: &str, identity: Identity, socket: Socket) -> Lease {
    let mut entries = lock(&ENTRIES);
    if entries.contains_key(session) {
        drop(entries);
        return Lease::uncached(Box::new(socket));
    }
    let entry = Arc::new(Entry {
        identity,
        session: session.to_owned(),
        state: Mutex::new(State::Busy),
        close: watch::channel(None).0,
        continuation: Mutex::new(None),
    });
    entries.insert(session.to_owned(), Arc::clone(&entry));
    let mut lease = Lease::uncached(Box::new(socket));
    lease.entry = Some(entry);
    lease
}

/// Own a socket for one request: the session's idle socket when its identity matches, otherwise
/// a new connection, cached only when no other operation holds the session.
pub(super) async fn acquire(
    session: Option<&str>,
    identity: Identity,
    connect: impl Future<Output = Result<Socket, CodexError>>,
) -> Result<Lease, CodexError> {
    let Some(session) = session.filter(|session| !session.is_empty()) else {
        return Ok(Lease::uncached(Box::new(connect.await?)));
    };
    match claim(session, &identity) {
        Claim::Busy => return Ok(Lease::uncached(Box::new(connect.await?))),
        Claim::Idle(lease) => return Ok(lease),
        Claim::Absent => {}
    }
    Ok(publish(session, identity, connect.await?))
}

/// The session's cached entry, if any.
#[cfg(test)]
pub(super) fn cached_entry(session: &str) -> Option<Arc<Entry>> {
    lock(&ENTRIES).get(session).cloned()
}

/// Close the session's cached connection, or every session's when none or an empty one is named.
/// Statistics and fallback state are not touched.
pub(crate) fn close_openai_codex_web_socket_sessions(session_id: Option<&str>) {
    let closed: Vec<Arc<Entry>> = {
        let mut entries = lock(&ENTRIES);
        match session_id.filter(|id| !id.is_empty()) {
            Some(id) => entries.remove(id).into_iter().collect(),
            None => std::mem::take(&mut *entries).into_values().collect(),
        }
    };
    for entry in closed {
        entry.close.send_replace(Some(CloseReason::DebugClose));
        let mut state = lock(&entry.state);
        if let State::Idle(mut idle) = std::mem::replace(&mut *state, State::Closed) {
            idle.task.abort();
            close_now(&mut idle.socket, CloseReason::DebugClose);
        }
    }
}

#[cfg(test)]
mod tests;
