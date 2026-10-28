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

/// One tool registration the guest attempted, whether or not the host admitted it.
pub struct Attempt {
    /// The metadata document the guest sent.
    pub metadata: String,
    /// How many transcript lines the host recorded before the attempt.
    pub position: usize,
}

/// Lines the host observed, the identities the extension dropped, how many resources the host
/// still lends out, and the registrations it received.
#[derive(Default)]
pub struct Observed {
    /// An explicitly supplied response from the original host operation owner.
    pub host_operation_result: Option<Result<serde_json::Value, String>>,
    /// Controlled native waits while a custom view script drives independent tails.
    pub tick_clock: Option<super::custom_ticks::Clock>,
    /// Supplied native byte resources and exact operand observations.
    pub native_io: super::native_io_host::Bank,
    /// Supplied HTTP responses and forwarded request operands.
    pub native_http: super::native_http_host::Bank,
    /// Supplied file watch registration observations.
    pub native_watch: super::native_watch_host::Bank,
    /// Raw shell byte receiver observations.
    pub shell: super::shell_host::Bank,
    /// Replacement-operation host decisions.
    pub replacement: super::replacement_host::Bank,
    /// Setup-session host decisions.
    pub setup: super::setup_host::Bank,
    /// Completion witness after the admitted setup future releases its capture.
    pub setup_finished: Option<oneshot::Sender<()>>,
    /// Host-operation teardown witness for an in-flight dropped command.
    pub replacement_finished: Option<oneshot::Sender<()>>,
    /// Guest operation drop acknowledgement before the host admission gate opens.
    pub replacement_dropped: Option<oneshot::Sender<()>>,
    /// Total admitted compaction requests.
    pub compactions: usize,
    /// Ordinary action controls.
    pub actions: super::action_host::Bank,
    /// Host helper operations and their literal encoded operands.
    pub helper_calls: Vec<(String, String)>,
    /// Registration transport observations and supplied flag replies.
    pub registrations: super::registration_host::Bank,
    /// Event-channel observations.
    pub bus: super::bus_host::Bank,
    /// Lines in time order.
    pub transcript: Vec<String>,
    /// Identities the extension dropped, in drop order.
    pub dropped: Vec<u32>,
    /// Resources handed to the extension that it has not dropped.
    pub live: usize,
    /// Authored tool metadata in registration order.
    pub tools: Vec<String>,
    /// Tool registrations the guest attempted, admitted or not, in attempt order.
    pub attempts: Vec<Attempt>,
    pub schemas: Vec<(String, String, u32)>,
    pub schema_request_bytes: usize,
    pub schema_response_bytes: usize,
    pub schema_checks: usize,
    /// Delivered partial results in observation order.
    pub updates: Vec<String>,
    /// Partial result number to refuse, counted from zero, with the host's message.
    pub refuse_update: Option<(usize, String)>,
    /// Session query control state.
    pub readers: super::reader_host::Bank,
    /// Live context and catalog controls.
    pub catalog: super::catalog_host::Bank,
    /// Retained UI acquisition identities and supplied rejections.
    pub ui: super::ui_host::Bank,
    /// Themes the host publishes under each handed-out key.
    pub themes: super::theme_host::Bank,
    /// The guest state identity of each tool row, by tool call id.
    pub tool_rows: HashMap<String, u32>,
    /// Last identity handed out.
    next: u32,
    /// Registered callback keys by name, such as `event input`.
    registered: HashMap<String, u32>,
    /// Registrations in order, until released.
    order: VecDeque<(String, u32)>,
    /// The hold set for the next wait for idle.
    hold: VecDeque<Hold>,
    /// Whether the next session operation fails without running its continuation.
    reject_session: bool,
    /// Captured standard output of the guest, when the host installed one.
    pub stdout: Option<wasmtime_wasi::p2::pipe::MemoryOutputPipe>,
}

impl Observed {
    /// Checks only effects whose producers are closed by the named barrier phase.
    pub fn assert_pending_phase(&self, phase: super::join::Phase, start: usize, shortcut: bool) {
        let lines = &self.transcript[start..];
        match phase {
            super::join::Phase::BothPending => {
                let guard = if shortcut {
                    r#"entry released "pending shortcut""#
                } else {
                    r#"entry released "pending completion""#
                };
                assert!(!lines.contains(&guard.into()));
            }
            super::join::Phase::FirstCompleted => {
                assert!(lines.contains(&r#"entry pending-done "/first""#.into()));
                assert!(!lines.contains(&r#"entry pending-done "/second""#.into()));
            }
        }
    }
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

    /// Identities the controlled host still owns, including widget style callbacks.
    pub fn live_identities(&self) -> Vec<u32> {
        (1..=self.next)
            .filter(|id| !self.dropped.contains(id))
            .collect()
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

    /// Records one tool registration, rejecting the named rejection fixture.
    pub fn register_tool(
        &mut self,
        metadata: &str,
        (prepare, execute): (Option<u32>, u32),
        (render_call, render_result): (Option<u32>, Option<u32>),
    ) -> Result<(), String> {
        #[derive(serde::Deserialize)]
        struct Named {
            /// The registration identity observed by the host.
            name: String,
        }
        let Named { name } = serde_json::from_str(metadata).map_err(|e| e.to_string())?;
        self.attempts.push(Attempt {
            metadata: metadata.into(),
            position: self.transcript.len(),
        });
        if let Some(error) = self.registrations.refusal() {
            return Err(error);
        }
        if name == "rejected" {
            return Err("tool rejected: Ω".into());
        }
        self.tools.push(metadata.into());
        if let Some(prepare) = prepare {
            self.register(format!("prepare {name}"), prepare);
        }
        if let Some(render) = render_call {
            self.register(format!("render-call {name}"), render);
        }
        if let Some(render) = render_result {
            self.register(format!("render-result {name}"), render);
        }
        self.register(format!("tool {name}"), execute);
        Ok(())
    }

    /// Records what the host last observed when the driver saw a held wait pending.
    pub fn suspended(&mut self) {
        let last = self.transcript.last().cloned().unwrap_or_default();
        self.log(format!("command suspended after {last}"));
        if last == "auth started" {
            self.log(format!("auth pending resources {}", self.live));
        }
    }

    /// Records one delivered partial result, or the message the host refuses it with.
    pub fn update(&mut self, partial: &str, details: &serde_json::Value) -> Result<(), String> {
        let refusal = match &self.refuse_update {
            Some((position, message)) if *position == self.updates.len() => Some(message.clone()),
            _ => None,
        };
        self.updates.push(partial.into());
        if let Some(message) = refusal {
            return Err(message);
        }
        if *details == "fail update" {
            return Err("failed update: Ω".into());
        }
        Ok(())
    }

    /// Records a custom session entry.
    pub fn entry(&mut self, custom_type: &str, data: Option<&str>) {
        if ((custom_type == "setup-drop" && data == Some("\"setup-future\""))
            || (custom_type == "replacement-drop" && data == Some("\"future\"")))
            && let Some(finished) = self.setup_finished.take()
        {
            let _ = finished.send(());
        }
        if custom_type == "replacement-dropped"
            && data == Some("\"host\"")
            && let Some(done) = self.replacement_dropped.take()
        {
            let _ = done.send(());
        }
        if custom_type == "replacement-control" {
            self.replacement = serde_json::from_str(data.expect("replacement control")).unwrap();
        }
        if custom_type == "setup-control" {
            self.setup = serde_json::from_str(data.expect("setup control")).unwrap();
        }
        if custom_type == "action-control" {
            self.actions.control(data.expect("action control"));
        }
        if custom_type == "reader-control" {
            self.readers.control(data.expect("control document"));
        }
        if custom_type == "catalog-control" {
            self.catalog.control(data.expect("control document"));
        }
        if custom_type == "ui-control" {
            self.ui.control(data.expect("control document"));
        }
        if custom_type == "theme-control" {
            self.themes.control(data.expect("control document"));
        }
        self.log(format!("entry {custom_type} {}", data.unwrap_or("none")));
    }

    /// Remembers the key a callback was registered under.
    pub(super) fn register(&mut self, name: String, key: u32) {
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

    /// Every key registered under `name`, in registration order.
    pub fn registrations(&self, name: &str) -> Vec<u32> {
        self.order
            .iter()
            .filter(|(queued, _)| queued == name)
            .map(|(_, key)| *key)
            .collect()
    }

    /// Takes the registration next in line for release, in registration order, then the
    /// announced event-bus tails; registrations made while earlier ones are released queue up
    /// behind them.
    pub fn next_to_release(&mut self) -> Option<(String, u32)> {
        self.order.pop_front().or_else(|| {
            let key = self.bus.unreleased.pop()?;
            Some(("bus tail".into(), key))
        })
    }

    /// Keeps the next wait for idle pending.
    pub fn hold_next_idle(&mut self, hold: Hold) {
        self.hold.push_back(hold);
    }

    /// Takes the hold set for the wait for idle that is starting.
    pub fn take_hold(&mut self) -> Option<Hold> {
        self.hold.pop_front()
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
