//! Context facades and the ports their adapters implement.

use std::ops::Deref;
use std::rc::Rc;

use crate::api::{ExtensionFuture, ExtensionResult};
use crate::bindings::maestro::extension::types::SessionChangeResult;

/// Cancellation flag owned by the host.
pub trait SignalPort {
    /// Whether the operation was cancelled.
    fn aborted(&self) -> bool;
}

/// A retained cancellation flag; it stays usable after the callback returns.
#[derive(Clone)]
pub struct AbortSignal(Rc<dyn SignalPort>);

impl AbortSignal {
    /// Wraps an adapter's signal.
    #[must_use]
    pub fn new(port: Rc<dyn SignalPort>) -> Self {
        Self(port)
    }

    /// Whether the operation was cancelled.
    #[must_use]
    pub fn aborted(&self) -> bool {
        self.0.aborted()
    }
}

/// Live session capabilities every callback receives.
pub trait ContextPort {
    /// Working directory of the session this context is bound to.
    ///
    /// # Errors
    /// Returns the host's message when the context is no longer valid.
    fn cwd(&self) -> ExtensionResult<String>;
    /// Signal of the running operation, when there is one.
    ///
    /// # Errors
    /// Returns the host's message when the context is no longer valid.
    fn signal(&self) -> ExtensionResult<Option<AbortSignal>>;
}

/// Ordinary context for handlers, tools and shortcuts; it has no session operations.
///
/// ```compile_fail,E0599
/// fn replace(ctx: maestro_extensions_wasm::ExtensionContext) {
///     let _ = ctx.new_session(None);
/// }
/// ```
#[derive(Clone)]
pub struct ExtensionContext(Rc<dyn ContextPort>);

impl ExtensionContext {
    /// Wraps an adapter's context.
    #[must_use]
    pub fn new(port: Rc<dyn ContextPort>) -> Self {
        Self(port)
    }

    /// Working directory of the bound session.
    ///
    /// # Errors
    /// Returns the host's message, for example when the session was replaced.
    pub fn cwd(&self) -> ExtensionResult<String> {
        self.0.cwd()
    }

    /// Signal of the running operation.
    ///
    /// # Errors
    /// Returns the host's message when the context is no longer valid.
    pub fn signal(&self) -> ExtensionResult<Option<AbortSignal>> {
        self.0.signal()
    }
}

/// Runs once with the context bound to the session an operation created.
pub type WithSession = Box<dyn FnOnce(ReplacedSessionContext) -> ExtensionFuture<'static, ()>>;

/// Plain options plus the continuation of a new session.
#[derive(Default)]
pub struct NewSessionCommandOptions {
    /// Session the new one continues from.
    pub parent_session: Option<String>,
    /// Continuation bound to the replacement target.
    pub with_session: Option<WithSession>,
}

/// Session-replacing operations of a command context.
pub trait CommandContextPort: ContextPort {
    /// Starts a new session and awaits its continuation.
    fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionChangeResult>;
}

/// Context only commands receive; it derefs to the ordinary context.
#[derive(Clone)]
pub struct ExtensionCommandContext {
    /// Ordinary capabilities of the command context.
    ordinary: ExtensionContext,
    /// Session-replacing operations.
    port: Rc<dyn CommandContextPort>,
}

impl ExtensionCommandContext {
    /// Wraps an adapter's command context.
    #[must_use]
    pub fn new(port: Rc<dyn CommandContextPort>) -> Self {
        Self {
            ordinary: ExtensionContext::new(Rc::clone(&port) as Rc<dyn ContextPort>),
            port,
        }
    }

    /// Starts a new session; the continuation runs against the replacement target.
    #[must_use]
    pub fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionChangeResult> {
        self.port.new_session(options)
    }
}

impl Deref for ExtensionCommandContext {
    type Target = ExtensionContext;

    fn deref(&self) -> &ExtensionContext {
        &self.ordinary
    }
}

/// Operations of the session an operation created.
pub trait ReplacedSessionContextPort {
    /// The command context bound to the replacement target.
    fn command(&self) -> Rc<dyn CommandContextPort>;
    /// Sends a user message to the replacement session.
    fn send_user_message(&self, text: &str) -> ExtensionFuture<'_, ()>;
}

/// Context of the replacement session; it derefs to its command context.
pub struct ReplacedSessionContext {
    /// Command context bound to the replacement target.
    command: ExtensionCommandContext,
    /// Operations only the replacement session offers.
    port: Rc<dyn ReplacedSessionContextPort>,
}

impl ReplacedSessionContext {
    /// Wraps an adapter's replacement context.
    #[must_use]
    pub fn new(port: Rc<dyn ReplacedSessionContextPort>) -> Self {
        Self {
            command: ExtensionCommandContext::new(port.command()),
            port,
        }
    }

    /// Sends a user message to the replacement session and awaits the host.
    #[must_use]
    pub fn send_user_message<'a>(&'a self, text: &'a str) -> ExtensionFuture<'a, ()> {
        self.port.send_user_message(text)
    }
}

impl Deref for ReplacedSessionContext {
    type Target = ExtensionCommandContext;

    fn deref(&self) -> &ExtensionCommandContext {
        &self.command
    }
}

/// Handler of a command: raw arguments and the command context.
pub type CommandHandler =
    Rc<dyn Fn(String, ExtensionCommandContext) -> ExtensionFuture<'static, ()>>;
