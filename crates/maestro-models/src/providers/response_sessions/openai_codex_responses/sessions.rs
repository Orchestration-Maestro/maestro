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
use std::task::Poll;
use std::time::Duration;
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep_until};
use tokio_tungstenite::tungstenite::{Error, Message};
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
    /// Whether an operation owns the socket.
    state: Mutex<State>,
    /// The close request, kept even while nothing listens.
    close: watch::Sender<Option<CloseReason>>,
    /// Context of the last completed request.
    pub(super) continuation: Mutex<Option<Continuation>>,
}

/// Who owns the cached socket.
enum State {
    /// An operation owns it.
    Busy,
    /// The idle task owns it.
    Idle(Idle),
}

/// The idle task of a released socket.
pub(super) struct Idle {
    /// Hands the socket to the next operation.
    pub(super) wake: oneshot::Sender<()>,
    /// Returns the socket when handed off, or nothing once the task closed it.
    pub(super) task: JoinHandle<Option<Held>>,
}

/// A socket that sends its close frame when it is dropped.
pub(super) struct Held {
    /// The open socket.
    pub(super) socket: Socket,
    /// Reason sent with the close frame.
    reason: CloseReason,
}

impl Drop for Held {
    fn drop(&mut self) {
        close_now(&mut self.socket, self.reason);
    }
}

/// A session's slot in the cache, held by whoever owns the entry's socket.
struct Slot {
    /// Session key.
    session: String,
    /// The entry the slot held when it was claimed.
    entry: Arc<Entry>,
}

/// Removes the slot when dropped, unless it already holds a replacement.
struct SlotGuard(Option<Slot>);

impl SlotGuard {
    /// Take the slot so dropping the guard leaves the cache alone.
    fn disarm(mut self) -> Option<Slot> {
        self.0.take()
    }
}

impl Drop for SlotGuard {
    fn drop(&mut self) {
        if let Some(Slot { session, entry }) = &self.0 {
            remove_current(session, entry);
        }
    }
}

/// Remove the session's slot only while it still holds `entry`.
fn remove_current(session: &str, entry: &Arc<Entry>) {
    let removed = {
        let mut entries = lock(&ENTRIES);
        if entries
            .get(session)
            .is_some_and(|current| Arc::ptr_eq(current, entry))
        {
            entries.remove(session)
        } else {
            None
        }
    };
    drop(removed);
}

/// An operation's ownership of one socket and, when cached, of its entry.
pub(super) struct Lease {
    /// The socket, closed when the lease is dropped.
    pub(super) held: Held,
    /// The cache slot, removed when the lease is dropped.
    slot: SlotGuard,
    /// The socket came from the idle cache.
    pub(super) reused: bool,
}

impl Lease {
    /// A socket the cache does not own.
    fn uncached(socket: Socket) -> Self {
        Self {
            held: Held {
                socket,
                reason: CloseReason::Done,
            },
            slot: SlotGuard(None),
            reused: false,
        }
    }

    /// The cached entry this lease keeps busy.
    fn entry(&self) -> Option<&Arc<Entry>> {
        self.slot.0.as_ref().map(|slot| &slot.entry)
    }

    /// The context slot of the cached entry.
    pub(super) fn continuation(&self) -> Option<&Mutex<Option<Continuation>>> {
        self.entry().map(|entry| &entry.continuation)
    }

    /// Resolves when the session is explicitly closed; never for an uncached lease.
    pub(super) fn closed(&self) -> impl Future<Output = CloseReason> + use<> {
        wait_closed(self.entry().map(|entry| entry.close.subscribe()))
    }

    /// Return a healthy socket to the cache, where it stays idle for five minutes.
    pub(super) fn keep(self) {
        let Self { mut held, slot, .. } = self;
        let Some(Slot { session, entry }) = slot.disarm() else {
            return;
        };
        let close = entry.close.subscribe();
        if let Some(reason) = *close.borrow() {
            held.reason = reason;
            return;
        }
        let (wake, ready) = oneshot::channel();
        let park = Park {
            ready,
            close,
            deadline: Instant::now() + IDLE_TTL,
            session,
            owner: Arc::downgrade(&entry),
        };
        let task = tokio::spawn(park.run(held));
        *lock(&entry.state) = State::Idle(Idle { wake, task });
    }
}

/// Wait for a close request, which a subscription sees even if it was made before subscribing.
async fn wait_closed(close: Option<watch::Receiver<Option<CloseReason>>>) -> CloseReason {
    let Some(mut close) = close else {
        return std::future::pending().await;
    };
    loop {
        if let Some(reason) = *close.borrow_and_update() {
            return reason;
        }
        if close.changed().await.is_err() {
            return CloseReason::Done;
        }
    }
}

/// What an idle socket waits for.
struct Park {
    /// The next operation's claim.
    ready: oneshot::Receiver<()>,
    /// Explicit close requests.
    close: watch::Receiver<Option<CloseReason>>,
    /// When the socket expires.
    deadline: Instant,
    /// Session key of the slot.
    session: String,
    /// The entry the socket belongs to; the task never keeps it alive.
    owner: Weak<Entry>,
}

/// One thing that happened to an idle socket.
enum Parked {
    /// An operation claimed the socket.
    Handoff,
    /// The entry was closed or dropped.
    Close,
    /// The idle period elapsed.
    Expire,
    /// The socket produced a message, an error or its end.
    Frame(Option<Result<Message, Error>>),
}

/// A claim hands the socket over; a dropped claim sender closes it.
fn handoff(claimed: bool) -> Parked {
    if claimed {
        Parked::Handoff
    } else {
        Parked::Close
    }
}

/// Discard what the socket already holds and report whether the peer ended it.
fn ended(socket: &mut Socket) -> bool {
    loop {
        match socket.next().now_or_never() {
            None => return false,
            Some(Some(Ok(Message::Close(_)) | Err(_)) | None) => return true,
            Some(Some(Ok(_))) => {}
        }
    }
}

impl Park {
    /// Wait for the first event.
    async fn next(&mut self, socket: &mut Socket) -> Parked {
        let mut closing = pin!(self.close.changed());
        let mut timer = pin!(sleep_until(self.deadline));
        let ready = &mut self.ready;
        poll_fn(|context| {
            if let Poll::Ready(claimed) = Pin::new(&mut *ready).poll(context) {
                return Poll::Ready(handoff(claimed.is_ok()));
            }
            if closing.as_mut().poll(context).is_ready() {
                return Poll::Ready(Parked::Close);
            }
            if timer.as_mut().poll(context).is_ready() {
                return Poll::Ready(Parked::Expire);
            }
            socket.poll_next_unpin(context).map(Parked::Frame)
        })
        .await
    }

    /// An operation has claimed the entry, so an expiry that raced with its handoff yields.
    fn claimed(&self) -> bool {
        self.owner
            .upgrade()
            .is_some_and(|entry| matches!(*lock(&entry.state), State::Busy))
    }

    /// Own the idle socket: answer pings, discard messages, and end on handoff, close,
    /// expiry or the peer going away. A handoff returns the socket.
    async fn run(mut self, mut held: Held) -> Option<Held> {
        let outcome = loop {
            match self.next(&mut held.socket).await {
                Parked::Handoff if !ended(&mut held.socket) => return Some(held),
                Parked::Handoff => break CloseReason::Done,
                Parked::Close => {
                    break self.close.borrow().unwrap_or(CloseReason::Done);
                }
                Parked::Expire if self.claimed() => return Some(held),
                Parked::Expire => break CloseReason::IdleTimeout,
                Parked::Frame(Some(Ok(Message::Close(_)) | Err(_)) | None) => {
                    break CloseReason::Done;
                }
                Parked::Frame(Some(Ok(_))) => {}
            }
        };
        held.reason = outcome;
        if let Some(entry) = self.owner.upgrade() {
            remove_current(&self.session, &entry);
        }
        None
    }
}

/// What the cache holds for a session when an operation asks.
pub(super) enum Claim {
    /// A matching idle socket, now marked busy.
    Idle(Arc<Entry>, Idle),
    /// An operation owns the entry.
    Busy,
    /// Nothing usable remains.
    Absent,
}

/// Mark a matching idle entry busy, release a mismatching one, and report a busy one.
pub(super) fn claim(session: &str, identity: &Identity) -> Claim {
    let mut entries = lock(&ENTRIES);
    let Some(entry) = entries.get(session).cloned() else {
        return Claim::Absent;
    };
    let mut state = lock(&entry.state);
    if matches!(*state, State::Busy) {
        return Claim::Busy;
    }
    if entry.identity != *identity {
        drop(state);
        let removed = entries.remove(session);
        drop(entries);
        entry.close.send_replace(Some(CloseReason::Done));
        drop(removed);
        return Claim::Absent;
    }
    match std::mem::replace(&mut *state, State::Busy) {
        State::Idle(idle) => Claim::Idle(Arc::clone(&entry), idle),
        State::Busy => Claim::Busy,
    }
}

/// Publish a freshly connected socket unless another operation published first.
pub(super) fn publish(session: &str, identity: Identity, socket: Socket) -> Lease {
    let mut entries = lock(&ENTRIES);
    if entries.contains_key(session) {
        drop(entries);
        return Lease::uncached(socket);
    }
    let entry = Arc::new(Entry {
        identity,
        state: Mutex::new(State::Busy),
        close: watch::channel(None).0,
        continuation: Mutex::new(None),
    });
    entries.insert(session.to_owned(), Arc::clone(&entry));
    Lease {
        slot: SlotGuard(Some(Slot {
            session: session.to_owned(),
            entry,
        })),
        ..Lease::uncached(socket)
    }
}

/// Own a socket for one request: the session's idle socket when its identity matches, otherwise
/// a new connection, cached only when no other operation holds the session.
pub(super) async fn acquire(
    session: Option<&str>,
    identity: Identity,
    connect: impl Future<Output = Result<Socket, CodexError>>,
) -> Result<Lease, CodexError> {
    let Some(session) = session.filter(|session| !session.is_empty()) else {
        return Ok(Lease::uncached(connect.await?));
    };
    match claim(session, &identity) {
        Claim::Busy => return Ok(Lease::uncached(connect.await?)),
        Claim::Idle(entry, idle) => {
            let slot = SlotGuard(Some(Slot {
                session: session.to_owned(),
                entry,
            }));
            // A closed receiver means the idle task already ended.
            let _ = idle.wake.send(());
            if let Ok(Some(held)) = idle.task.await {
                return Ok(Lease {
                    held,
                    slot,
                    reused: true,
                });
            }
        }
        Claim::Absent => {}
    }
    Ok(publish(session, identity, connect.await?))
}

/// The session's cached entry, if any.
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
    }
}
