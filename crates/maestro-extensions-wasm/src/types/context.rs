//! Contexts handlers receive, and the cancellation signals they can retain.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use crate::ReadonlySessionManager;
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

use super::extension_result::{ExtensionFuture, ExtensionResult};
use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};

/// Host capability reporting whether an operation was cancelled.
pub trait SignalPort {
    /// Whether the operation was cancelled.
    fn aborted(&self) -> bool;
}

/// Cancellation flag of an operation. Clones share the host flag and may outlive the
/// callback that received them.
#[derive(Clone)]
pub struct AbortSignal(Rc<dyn SignalPort>);

impl AbortSignal {
    /// Wraps the host's flag.
    #[must_use]
    pub fn new(port: Rc<dyn SignalPort>) -> Self {
        Self(port)
    }

    /// Whether the operation was cancelled when this is called.
    #[must_use]
    pub fn aborted(&self) -> bool {
        self.0.aborted()
    }
}

impl fmt::Debug for AbortSignal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AbortSignal")
            .field("aborted", &self.aborted())
            .finish()
    }
}

port! {
    /// Live capabilities of the host available to every callback.
    ContextPort for ExtensionContext via 0 {
        /// The working directory of the current session.
        fn cwd() -> String;
        /// Acquires the reader currently bound to this context.
        fn session_manager() -> ReadonlySessionManager;
    }
}

/// Context passed to event handlers. It has no session-replacing operations, so a handler
/// cannot start a session or wait for the agent:
///
/// ```compile_fail,E0599
/// fn handler(ctx: maestro_extensions_wasm::ExtensionContext) {
///     let _ = ctx.wait_for_idle();
/// }
/// ```
///
/// ```compile_fail,E0599
/// fn handler(ctx: maestro_extensions_wasm::ExtensionContext) {
///     let _ = ctx.new_session(None);
/// }
/// ```
#[derive(Clone)]
pub struct ExtensionContext(Rc<dyn ContextPort>);

impl ExtensionContext {
    /// Wraps the host's context.
    #[must_use]
    pub fn new(port: Rc<dyn ContextPort>) -> Self {
        Self(port)
    }
}

/// Continuation run against the replacement session of an operation.
pub type WithSession = Box<dyn FnOnce(ReplacedSessionContext) -> ExtensionFuture<'static, ()>>;

/// Plain data and continuation of a new session started from a command.
pub struct NewSessionCommandOptions {
    /// Plain data of the new session.
    pub data: NewSessionCommandData,
    /// The host runs it against a context bound to the replacement session.
    pub with_session: Option<WithSession>,
}

/// Session-replacing operations of a command context; each method forwards one operation and
/// resolves to the host's message when the host rejects it.
pub trait CommandContextPort: ContextPort {
    /// Waits for the agent to finish streaming.
    fn wait_for_idle(&self) -> ExtensionFuture<'_, ()>;
    /// Starts a new session.
    fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionChangeResult>;
}

/// Context of commands: the ordinary context plus the session-replacing operations.
#[derive(Clone)]
pub struct ExtensionCommandContext {
    /// The host's command capabilities.
    port: Rc<dyn CommandContextPort>,
    /// The ordinary view over the same capabilities.
    ordinary: ExtensionContext,
}

impl ExtensionCommandContext {
    /// Wraps the host's command context.
    #[must_use]
    pub fn new(port: Rc<dyn CommandContextPort>) -> Self {
        let ordinary = ExtensionContext::new(Rc::clone(&port) as Rc<dyn ContextPort>);
        Self { port, ordinary }
    }

    /// Waits for the agent to finish streaming.
    #[must_use]
    pub fn wait_for_idle(&self) -> ExtensionFuture<'_, ()> {
        self.port.wait_for_idle()
    }

    /// Starts a new session; the host runs its `with_session`, if any, against the
    /// replacement session.
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

/// Operations of a [`ReplacedSessionContext`] beyond its command context.
pub trait ReplacedSessionContextPort {
    /// The command context bound to the replacement session.
    fn command(&self) -> Rc<dyn CommandContextPort>;
}

/// Fresh command-capable context bound to the replacement session of an operation.
#[derive(Clone)]
pub struct ReplacedSessionContext {
    /// The command view bound to the replacement session.
    command: ExtensionCommandContext,
}

impl ReplacedSessionContext {
    /// Wraps the host's replacement context.
    #[must_use]
    pub fn new(port: &dyn ReplacedSessionContextPort) -> Self {
        Self {
            command: ExtensionCommandContext::new(port.command()),
        }
    }
}

impl Deref for ReplacedSessionContext {
    type Target = ExtensionCommandContext;

    fn deref(&self) -> &ExtensionCommandContext {
        &self.command
    }
}
