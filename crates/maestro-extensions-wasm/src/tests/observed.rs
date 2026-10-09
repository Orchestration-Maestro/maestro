//! What both test hosts record, and the sessions their contexts are bound to.

use std::cell::Cell;
use std::collections::{HashMap, VecDeque};

use tokio::sync::oneshot;

/// The event name hosts reject registrations for.
pub const REJECTED: &str = "rejected";

/// The error a context of a replaced session reports.
pub const STALE: &str = "This context is stale after session replacement.";

/// Keeps a session operation pending in its wait for idle: the wait reports through `started`
/// and then waits until `open` resolves.
pub struct Hold {
    /// Reports that the operation is pending.
    pub started: oneshot::Sender<()>,
    /// Lets the operation continue.
    pub open: oneshot::Receiver<()>,
}

/// Channels that hold a wait for idle pending until the driver saw it.
pub fn gate() -> (Hold, oneshot::Receiver<()>, oneshot::Sender<()>) {
    let (started_tx, started) = oneshot::channel();
    let (open, opened) = oneshot::channel();
    (
        Hold {
            started: started_tx,
            open: opened,
        },
        started,
        open,
    )
}

/// Reports that a held operation is pending and waits until the driver lets it continue.
pub async fn pause(hold: Option<Hold>) {
    if let Some(Hold { started, open }) = hold {
        let _ = started.send(());
        let _ = open.await;
    }
}

/// The session a context is bound to; it goes stale once an operation replaces it.
pub struct Session {
    /// Working directory of the session.
    pub cwd: String,
    /// Whether an operation replaced the session.
    pub stale: Cell<bool>,
}

impl Session {
    /// A live session in `cwd`.
    pub fn new(cwd: &str) -> Self {
        Self {
            cwd: cwd.to_owned(),
            stale: Cell::new(false),
        }
    }

    /// The working directory, or the stale message.
    pub fn cwd(&self) -> Result<String, String> {
        if self.stale.get() {
            Err(STALE.to_owned())
        } else {
            Ok(self.cwd.clone())
        }
    }
}

/// Lines the host observed, the identities the extension dropped, how many resources the host
/// still lends out, and the registrations it received.
#[derive(Default)]
pub struct Observed {
    /// Lines in time order.
    pub transcript: Vec<String>,
    /// Identities the extension dropped, in drop order.
    pub dropped: Vec<u32>,
    /// Resources handed to the extension that it has not dropped.
    pub live: usize,
    /// Last identity handed out.
    next: u32,
    /// Registered callback keys by name, such as `event input`.
    registered: HashMap<String, u32>,
    /// Registrations in order, until released.
    order: VecDeque<(String, u32)>,
    /// The hold set for the next wait for idle.
    hold: Option<Hold>,
    /// Whether the next session operation fails without running its continuation.
    reject_session: bool,
}

impl Observed {
    /// Appends a line to the transcript.
    pub fn log(&mut self, line: impl Into<String>) {
        self.transcript.push(line.into());
    }

    /// Counts a resource lent to the extension.
    pub fn lend(&mut self) {
        self.live += 1;
    }

    /// Counts a lent resource the extension dropped.
    pub fn reclaim(&mut self) {
        self.live = self.live.saturating_sub(1);
    }

    /// Hands out a fresh callback identity.
    pub fn next_identity(&mut self) -> u32 {
        self.next += 1;
        self.lend();
        self.next
    }

    /// Records that the extension dropped an identity.
    pub fn drop_identity(&mut self, identity: u32) {
        self.dropped.push(identity);
        self.reclaim();
    }

    /// Records a handler registration under `key`, or rejects it.
    pub fn on(&mut self, event: &str, key: u32) -> Result<(), String> {
        if event == REJECTED {
            self.log(format!("reject event {event}"));
            return Err(format!("registration rejected: {event}"));
        }
        self.log(format!("register event {event}"));
        self.register(format!("event {event}"), key);
        Ok(())
    }

    /// Records a command registration under `key`.
    pub fn register_command(&mut self, name: &str, key: u32) {
        self.log(format!("register command {name}"));
        self.register(format!("command {name}"), key);
    }

    /// Records what the host last observed when the driver saw a held wait pending.
    pub fn suspended(&mut self) {
        let last = self.transcript.last().cloned().unwrap_or_default();
        self.log(format!("command suspended after {last}"));
    }

    /// Records a custom session entry.
    pub fn entry(&mut self, custom_type: &str, data: Option<&str>) {
        self.log(format!("entry {custom_type} {}", data.unwrap_or("none")));
    }

    /// Remembers the key a callback was registered under.
    fn register(&mut self, name: String, key: u32) {
        self.order.push_back((name.clone(), key));
        self.registered.insert(name, key);
    }

    /// The key a callback was registered under.
    pub fn callback(&self, name: &str) -> Result<u32, String> {
        self.registered
            .get(name)
            .copied()
            .ok_or_else(|| format!("nothing registered as {name}"))
    }

    /// Takes the registration under `name` out of the line for release, so that releasing every
    /// registration afterwards does not release it twice. Returns its key, or none when nothing
    /// is registered under `name`.
    pub fn take_registration(&mut self, name: &str) -> Option<u32> {
        let position = self.order.iter().position(|(queued, _)| queued == name)?;
        self.order.remove(position).map(|(_, key)| key)
    }

    /// Takes the registration next in line for release, in registration order; registrations
    /// made while earlier ones are released queue up behind them.
    pub fn next_to_release(&mut self) -> Option<(String, u32)> {
        self.order.pop_front()
    }

    /// Keeps the next wait for idle pending.
    pub fn hold_next_idle(&mut self, hold: Hold) {
        self.hold = Some(hold);
    }

    /// Takes the hold set for the wait for idle that is starting.
    pub fn take_hold(&mut self) -> Option<Hold> {
        self.hold.take()
    }

    /// Makes the next session operation fail without running its continuation.
    pub fn reject_next_session(&mut self) {
        self.reject_session = true;
    }

    /// Whether the operation that is starting is to fail; consumes the request.
    pub fn take_rejection(&mut self) -> bool {
        std::mem::take(&mut self.reject_session)
    }
}
