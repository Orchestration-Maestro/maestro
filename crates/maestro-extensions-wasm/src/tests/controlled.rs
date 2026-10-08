//! The controlled adapter: implements the author ports in memory so the shared extension runs
//! without a component, and records what the host would observe.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use maestro_extensions_wasm::{
    AbortSignal, CommandContextPort, CommandHandler, CommandOptions, ContextPort, ExtensionAPI,
    ExtensionCommandContext, ExtensionContext, ExtensionEvent, ExtensionEventResult,
    ExtensionFuture, ExtensionHandler, ExtensionHost, ExtensionResult, NewSessionCommandOptions,
    ReplacedSessionContext, ReplacedSessionContextPort, SessionChangeResult, SignalPort,
};
use serde_json::Value;
use tokio::sync::oneshot;

/// The event name the host rejects registrations for.
pub const REJECTED: &str = "rejected";

/// The error a context of a replaced session reports.
pub const STALE: &str = "This context is stale after session replacement.";

/// Lines the host observed, shared by every handle to it.
#[derive(Clone, Default)]
pub struct Log(Rc<RefCell<Vec<String>>>);

impl Log {
    /// Appends a line.
    pub fn push(&self, line: impl Into<String>) {
        self.0.borrow_mut().push(line.into());
    }

    /// The lines so far.
    pub fn lines(&self) -> Vec<String> {
        self.0.borrow().clone()
    }
}

/// A cancellation flag the test controls.
#[derive(Clone, Default)]
pub struct Flag(Rc<Cell<bool>>);

impl Flag {
    /// A signal reading this flag.
    pub fn signal(&self) -> AbortSignal {
        AbortSignal::new(Rc::new(self.clone()))
    }

    /// Cancels the flag.
    pub fn abort(&self) {
        self.0.set(true);
    }
}

impl SignalPort for Flag {
    fn aborted(&self) -> bool {
        self.0.get()
    }
}

/// The session a context is bound to; it goes stale once an operation replaces it.
struct Session {
    /// Working directory of the session.
    cwd: String,
    /// Whether an operation replaced the session.
    stale: Cell<bool>,
}

impl Session {
    /// A live session in `cwd`.
    fn new(cwd: &str) -> Rc<Self> {
        Rc::new(Self {
            cwd: cwd.to_owned(),
            stale: Cell::new(false),
        })
    }

    /// The working directory, or the stale message.
    fn cwd(&self) -> ExtensionResult<String> {
        if self.stale.get() {
            Err(STALE.to_owned())
        } else {
            Ok(self.cwd.clone())
        }
    }
}

/// Keeps a session operation pending: the operation reports through `started` and then waits
/// until `open` resolves.
pub struct Hold {
    /// Reports that the operation is pending.
    pub started: oneshot::Sender<()>,
    /// Lets the operation continue.
    pub open: oneshot::Receiver<()>,
}

/// Ordinary context of the controlled host.
struct Ordinary(Rc<Session>);

impl ContextPort for Ordinary {
    fn cwd(&self) -> ExtensionResult<String> {
        self.0.cwd()
    }
}

/// Command context of the controlled host.
#[derive(Clone)]
struct Command {
    /// The session the context is bound to.
    session: Rc<Session>,
    /// Where the context records what it observes.
    log: Log,
    /// The next session operation waits on this hold.
    hold: Rc<RefCell<Option<Hold>>>,
}

impl Command {
    /// Waits for the hold set for the next operation, if any.
    async fn pending(&self) {
        let hold = self.hold.borrow_mut().take();
        if let Some(Hold { started, open }) = hold {
            let _ = started.send(());
            let _ = open.await;
        }
    }
}

impl ContextPort for Command {
    fn cwd(&self) -> ExtensionResult<String> {
        self.session.cwd()
    }
}

impl ReplacedSessionContextPort for Command {
    fn command(&self) -> Rc<dyn CommandContextPort> {
        Rc::new(self.clone())
    }
}

impl CommandContextPort for Command {
    fn wait_for_idle(&self) -> ExtensionFuture<'_, ()> {
        Box::pin(async move {
            self.log
                .push(format!("wait-for-idle {}", self.session.cwd()?));
            Ok(())
        })
    }

    fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionChangeResult> {
        Box::pin(async move {
            let options = options.ok_or("the scenario always supplies options")?;
            let parent = options.data.parent_session.as_deref().unwrap_or("none");
            self.log.push(format!("new-session start parent={parent}"));
            self.pending().await;
            let replacement = Command {
                session: Session::new("/replacement"),
                ..self.clone()
            };
            if let Some(with_session) = options.with_session {
                with_session(ReplacedSessionContext::new(&replacement)).await?;
            }
            self.session.stale.set(true);
            self.log.push("new-session done cancelled=false");
            Ok(SessionChangeResult { cancelled: false })
        })
    }
}

/// One registration the controlled host received.
enum Registration {
    /// A handler for an event name.
    Event(String, ExtensionHandler),
    /// A command.
    Command(String, CommandHandler),
}

/// The controlled host: records registrations and serves contexts from memory.
#[derive(Clone)]
pub struct ControlledHost {
    /// What the host observed.
    log: Log,
    /// Registrations in order; dropping one releases its closure.
    registrations: Rc<RefCell<Vec<Registration>>>,
    /// The next session operation waits on this hold.
    hold: Rc<RefCell<Option<Hold>>>,
}

impl ControlledHost {
    /// A host recording into `log`.
    pub fn new(log: Log) -> Self {
        Self {
            log,
            registrations: Rc::default(),
            hold: Rc::default(),
        }
    }

    /// The author handle registering with this host.
    pub fn api(&self) -> ExtensionAPI {
        ExtensionAPI::new(Rc::new(self.clone()))
    }

    /// Keeps the next session operation pending.
    pub fn hold_next_session(&self, hold: Hold) {
        *self.hold.borrow_mut() = Some(hold);
    }

    /// The first handler registered for `event`.
    pub fn handler(&self, event: &str) -> Option<ExtensionHandler> {
        self.registrations
            .borrow()
            .iter()
            .find_map(|registration| match registration {
                Registration::Event(name, handler) if name == event => Some(Rc::clone(handler)),
                Registration::Event(..) | Registration::Command(..) => None,
            })
    }

    /// The handler of a registered command.
    pub fn command_handler(&self, command: &str) -> Option<CommandHandler> {
        self.registrations
            .borrow()
            .iter()
            .find_map(|registration| match registration {
                Registration::Command(name, handler) if name == command => Some(Rc::clone(handler)),
                Registration::Event(..) | Registration::Command(..) => None,
            })
    }

    /// An ordinary context in the working directory.
    pub fn context() -> ExtensionContext {
        ExtensionContext::new(Rc::new(Ordinary(Session::new("/work"))))
    }

    /// A command context in the working directory.
    pub fn command_context(&self) -> ExtensionCommandContext {
        ExtensionCommandContext::new(Rc::new(Command {
            session: Session::new("/work"),
            log: self.log.clone(),
            hold: Rc::clone(&self.hold),
        }))
    }

    /// Drops every registration in order, which releases the closures they hold.
    pub fn release_all(&self) {
        let registrations = std::mem::take(&mut *self.registrations.borrow_mut());
        for registration in registrations {
            drop(registration);
        }
    }

    /// Runs the first handler of `event` against an event the test edits.
    pub async fn dispatch(
        &self,
        event: &str,
        edited: &mut ExtensionEvent,
    ) -> ExtensionResult<Option<ExtensionEventResult>> {
        let handler = self
            .handler(event)
            .ok_or_else(|| format!("no handler for {event}"))?;
        handler(edited, Self::context()).await
    }
}

impl ExtensionHost for ControlledHost {
    fn on(&self, event: &str, handler: ExtensionHandler) -> ExtensionResult<()> {
        if event == REJECTED {
            self.log.push(format!("reject event {event}"));
            return Err(format!("registration rejected: {event}"));
        }
        self.log.push(format!("register event {event}"));
        self.registrations
            .borrow_mut()
            .push(Registration::Event(event.to_owned(), handler));
        Ok(())
    }

    fn register_command(&self, name: &str, options: CommandOptions) -> ExtensionResult<()> {
        self.log.push(format!("register command {name}"));
        self.registrations
            .borrow_mut()
            .push(Registration::Command(name.to_owned(), options.handler));
        Ok(())
    }

    fn append_entry(&self, custom_type: &str, data: Option<Value>) -> ExtensionResult<()> {
        let data = data.map_or_else(|| "none".to_owned(), |data| data.to_string());
        self.log.push(format!("entry {custom_type} {data}"));
        Ok(())
    }
}
