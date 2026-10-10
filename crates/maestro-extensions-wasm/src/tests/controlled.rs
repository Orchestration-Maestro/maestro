//! The controlled adapter: an in-process host that implements the wire imports, so the
//! component adapter's own functions run natively against it, and that records what a real
//! host would observe.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::observed::{Observed, Session, pause};
use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};
use crate::component_adapter::{Exports, Imports};
use crate::types::{ExtensionFuture, ExtensionResult};

/// A cancellation flag the test controls.
#[derive(Clone, Default)]
pub struct Flag(Rc<Cell<bool>>);

impl Flag {
    /// Cancels the flag.
    pub fn abort(&self) {
        self.0.set(true);
    }
}

/// Counts one resource lent to the extension until the extension drops it.
struct Lease(Rc<RefCell<Observed>>);

impl Lease {
    /// Lends a resource.
    fn new(observed: &Rc<RefCell<Observed>>) -> Self {
        observed.borrow_mut().lend();
        Self(Rc::clone(observed))
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.0.borrow_mut().reclaim();
    }
}

/// Owner handle of a callback identity; the host sees it dropped.
pub struct Callback {
    /// Key of the identity.
    id: u32,
    /// What the host observed.
    observed: Rc<RefCell<Observed>>,
}

impl Drop for Callback {
    fn drop(&mut self) {
        self.observed.borrow_mut().drop_identity(self.id);
    }
}

/// Host cancellation flag lent to the extension.
pub struct Signal {
    /// The flag the test controls.
    flag: Flag,
    /// Marks the resource as lent.
    _lease: Lease,
}

/// Progress resource lent to tool execution.
pub struct Update {
    /// Observations shared with the host.
    observed: Rc<RefCell<Observed>>,
    /// Counts its owned lifetime.
    _lease: Lease,
}

/// Ordinary context lent to the extension.
pub struct Context {
    /// The session the context is bound to.
    session: Session,
    /// Marks the resource as lent.
    _lease: Lease,
}

/// Command context lent to the extension.
pub struct CommandContext {
    /// The session the context is bound to.
    session: Session,
    /// Marks the resource as lent.
    _lease: Lease,
}

/// Replacement context lent to the extension.
pub struct ReplacedContext {
    /// The session the context is bound to.
    session: Session,
    /// Marks the resource as lent.
    _lease: Lease,
}

/// The controlled host: records registrations and serves contexts from memory.
#[derive(Clone, Default)]
pub struct Controlled {
    /// What the host observed.
    observed: Rc<RefCell<Observed>>,
}

impl Controlled {
    /// What the host observed so far.
    pub fn observed(&self) -> std::cell::Ref<'_, Observed> {
        self.observed.borrow()
    }

    /// Changes what the host observed, such as the hold of the next wait for idle.
    pub fn observe(&self) -> std::cell::RefMut<'_, Observed> {
        self.observed.borrow_mut()
    }

    /// A cancellation signal reading `flag`.
    pub fn signal(&self, flag: &Flag) -> Signal {
        Signal {
            flag: flag.clone(),
            _lease: Lease::new(&self.observed),
        }
    }

    /// An ordinary context in `cwd`.
    pub fn context(&self, cwd: &str) -> Context {
        Context {
            session: Session::new(cwd),
            _lease: Lease::new(&self.observed),
        }
    }

    /// A progress resource sharing this host's observations.
    pub fn update(&self) -> Update {
        Update {
            observed: Rc::clone(&self.observed),
            _lease: Lease::new(&self.observed),
        }
    }

    /// A command context in `cwd`.
    pub fn command_context(&self, cwd: &str) -> CommandContext {
        CommandContext {
            session: Session::new(cwd),
            _lease: Lease::new(&self.observed),
        }
    }

    /// A replacement context in `cwd`.
    pub fn replaced_context(&self, cwd: &str) -> ReplacedContext {
        ReplacedContext {
            session: Session::new(cwd),
            _lease: Lease::new(&self.observed),
        }
    }

    /// Runs the continuation announced for an operation against a fresh replacement context.
    async fn continue_with(&self, callback: &Callback, cwd: &str) -> ExtensionResult<()> {
        Exports::new(self.clone())
            .invoke_with_session(callback.id, self.replaced_context(cwd))
            .await
    }
}

impl Imports for Controlled {
    type Update = Update;
    type Callback = Callback;
    type Signal = Signal;
    type Context = Context;
    type CommandContext = CommandContext;
    type ReplacedContext = ReplacedContext;

    fn new_callback(&self) -> (u32, Callback) {
        let id = self.observed.borrow_mut().next_identity();
        (
            id,
            Callback {
                id,
                observed: Rc::clone(&self.observed),
            },
        )
    }

    fn on(&self, event: &str, handler: &Callback) -> ExtensionResult<()> {
        self.observed.borrow_mut().on(event, handler.id)
    }

    fn register_command(
        &self,
        name: &str,
        _description: Option<&str>,
        handler: &Callback,
    ) -> ExtensionResult<()> {
        self.observed
            .borrow_mut()
            .register_command(name, handler.id);
        Ok(())
    }

    fn register_tool(
        &self,
        metadata: &str,
        prepare: Option<&Callback>,
        execute: &Callback,
    ) -> ExtensionResult<()> {
        self.observed
            .borrow_mut()
            .register_tool(metadata, prepare.map(|p| p.id), execute.id)
    }

    fn tool_update(&self, update: &Update, partial: &str) -> ExtensionResult<()> {
        let value: serde_json::Value = serde_json::from_str(partial).map_err(|e| e.to_string())?;
        update.observed.borrow_mut().updates.push(partial.into());
        if value["details"] == "fail update" {
            return Err("failed update: Ω".into());
        }
        Ok(())
    }

    fn append_entry(&self, custom_type: &str, data: Option<&str>) -> ExtensionResult<()> {
        self.observed.borrow_mut().entry(custom_type, data);
        Ok(())
    }

    fn aborted(&self, signal: &Signal) -> bool {
        signal.flag.0.get()
    }

    fn cwd(&self, context: &Context) -> ExtensionResult<String> {
        context.session.cwd()
    }

    fn command_cwd(&self, context: &CommandContext) -> ExtensionResult<String> {
        context.session.cwd()
    }

    fn wait_for_idle<'a>(&'a self, context: &'a CommandContext) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let cwd = &context.session.cwd;
            let hold = {
                let mut observed = self.observed.borrow_mut();
                observed.log(format!("wait-for-idle {cwd}"));
                observed.take_hold()
            };
            pause(hold).await;
            Ok(())
        })
    }

    fn new_session<'a>(
        &'a self,
        context: &'a CommandContext,
        data: NewSessionCommandData,
        with_session: Option<&'a Callback>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async move {
            let parent = data.parent_session.as_deref().unwrap_or("none");
            self.observed
                .borrow_mut()
                .log(format!("new-session start parent={parent}"));
            if self.observed.borrow_mut().take_rejection() {
                return Err("session rejected".to_owned());
            }
            if let Some(callback) = with_session {
                self.continue_with(callback, "/replacement").await?;
            }
            context.session.stale.set(true);
            self.observed
                .borrow_mut()
                .log("new-session done cancelled=false");
            Ok(SessionChangeResult { cancelled: false })
        })
    }

    fn command(&self, context: &ReplacedContext) -> CommandContext {
        self.command_context(&context.session.cwd)
    }
}
