//! Generated context and signal resources behind the author ports.

use std::rc::Rc;

use super::callbacks::Scoped;
use crate::bindings::maestro::extension::host;
use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};
use crate::types::{
    AbortSignal, CommandContextPort, ContextPort, ExtensionCommandContext, ExtensionContext,
    ExtensionFuture, ExtensionResult, NewSessionCommandOptions, ReplacedSessionContext,
    ReplacedSessionContextPort, SignalPort,
};

/// Generated signal behind the facade.
struct GeneratedSignal(host::AbortSignal);

impl SignalPort for GeneratedSignal {
    fn aborted(&self) -> bool {
        self.0.aborted()
    }
}

/// Wraps a generated signal.
pub(super) fn signal(signal: host::AbortSignal) -> AbortSignal {
    AbortSignal::new(Rc::new(GeneratedSignal(signal)))
}

/// Generated ordinary context behind the facade.
struct GeneratedContext(host::Context);

impl ContextPort for GeneratedContext {
    fn cwd(&self) -> ExtensionResult<String> {
        self.0.cwd()
    }
}

/// Wraps a generated ordinary context.
pub(super) fn context(ctx: host::Context) -> ExtensionContext {
    ExtensionContext::new(Rc::new(GeneratedContext(ctx)))
}

/// Generated command context behind the facade.
struct GeneratedCommand(host::CommandContext);

impl ContextPort for GeneratedCommand {
    fn cwd(&self) -> ExtensionResult<String> {
        self.0.cwd()
    }
}

impl CommandContextPort for GeneratedCommand {
    fn wait_for_idle(&self) -> ExtensionFuture<'_, ()> {
        Box::pin(self.0.wait_for_idle())
    }

    fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionChangeResult> {
        Box::pin(async move {
            let NewSessionCommandOptions { data, with_session } =
                options.unwrap_or_else(|| NewSessionCommandOptions {
                    data: NewSessionCommandData {
                        parent_session: None,
                    },
                    with_session: None,
                });
            let with_session = with_session.map(Scoped::new);
            self.0
                .new_session(data, with_session.as_ref().map(|scoped| &scoped.handle))
                .await
        })
    }
}

/// Wraps a generated command context.
pub(super) fn command_context(ctx: host::CommandContext) -> ExtensionCommandContext {
    ExtensionCommandContext::new(Rc::new(GeneratedCommand(ctx)))
}

/// Generated replacement context behind the facade.
struct GeneratedReplaced(host::ReplacedSessionContext);

impl ReplacedSessionContextPort for GeneratedReplaced {
    fn command(&self) -> Rc<dyn CommandContextPort> {
        Rc::new(GeneratedCommand(self.0.command()))
    }
}

/// Wraps a generated replacement context.
pub(super) fn replaced_context(ctx: host::ReplacedSessionContext) -> ReplacedSessionContext {
    ReplacedSessionContext::new(&GeneratedReplaced(ctx))
}
