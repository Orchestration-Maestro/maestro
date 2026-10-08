//! Host context and signal resources behind the author ports.

use std::rc::Rc;

use super::callbacks::Scoped;
use super::imports::Imports;
use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};
use crate::types::{
    AbortSignal, CommandContextPort, ContextPort, ExtensionCommandContext, ExtensionContext,
    ExtensionFuture, ExtensionResult, NewSessionCommandOptions, ReplacedSessionContext,
    ReplacedSessionContextPort, SignalPort,
};

/// Host signal behind the facade.
struct Signal<I: Imports> {
    /// The host the signal belongs to.
    imports: I,
    /// The host's cancellation flag.
    signal: I::Signal,
}

impl<I: Imports> SignalPort for Signal<I> {
    fn aborted(&self) -> bool {
        self.imports.aborted(&self.signal)
    }
}

/// Wraps a host signal.
pub(super) fn signal<I: Imports>(imports: &I, signal: I::Signal) -> AbortSignal {
    AbortSignal::new(Rc::new(Signal {
        imports: imports.clone(),
        signal,
    }))
}

/// Host ordinary context behind the facade.
struct Ordinary<I: Imports> {
    /// The host the context belongs to.
    imports: I,
    /// The host's context.
    context: I::Context,
}

impl<I: Imports> ContextPort for Ordinary<I> {
    fn cwd(&self) -> ExtensionResult<String> {
        self.imports.cwd(&self.context)
    }
}

/// Wraps a host ordinary context.
pub(super) fn context<I: Imports>(imports: &I, context: I::Context) -> ExtensionContext {
    ExtensionContext::new(Rc::new(Ordinary {
        imports: imports.clone(),
        context,
    }))
}

/// Host command context behind the facade.
struct Command<I: Imports> {
    /// The host the context belongs to.
    imports: I,
    /// The host's command context.
    context: I::CommandContext,
}

impl<I: Imports> ContextPort for Command<I> {
    fn cwd(&self) -> ExtensionResult<String> {
        self.imports.command_cwd(&self.context)
    }
}

impl<I: Imports> CommandContextPort for Command<I> {
    fn wait_for_idle(&self) -> ExtensionFuture<'_, ()> {
        self.imports.wait_for_idle(&self.context)
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
            let with_session = with_session.map(|closure| Scoped::new(&self.imports, closure));
            self.imports
                .new_session(
                    &self.context,
                    data,
                    with_session.as_ref().map(|scoped| &scoped.handle),
                )
                .await
        })
    }
}

/// Wraps a host command context.
pub(super) fn command_context<I: Imports>(
    imports: &I,
    context: I::CommandContext,
) -> ExtensionCommandContext {
    ExtensionCommandContext::new(Rc::new(Command {
        imports: imports.clone(),
        context,
    }))
}

/// Host replacement context behind the facade.
struct Replaced<I: Imports> {
    /// The host the context belongs to.
    imports: I,
    /// The host's replacement context.
    context: I::ReplacedContext,
}

impl<I: Imports> ReplacedSessionContextPort for Replaced<I> {
    fn command(&self) -> Rc<dyn CommandContextPort> {
        Rc::new(Command {
            imports: self.imports.clone(),
            context: self.imports.command(&self.context),
        })
    }
}

/// Wraps a host replacement context.
pub(super) fn replaced_context<I: Imports>(
    imports: &I,
    context: I::ReplacedContext,
) -> ReplacedSessionContext {
    ReplacedSessionContext::new(&Replaced {
        imports: imports.clone(),
        context,
    })
}
