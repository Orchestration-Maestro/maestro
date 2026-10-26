//! Host context and signal resources behind the author ports.

use std::rc::Rc;

use super::callbacks::Scoped;
use super::imports::Imports;
use super::session::reader;
use super::{model_registry::registry, session::decode};
use crate::bindings::maestro::extension::session::SessionChangeResult;
use crate::types::{
    AbortSignal, CommandContextPort, ContextPort, ExtensionCommandContext, ExtensionContext,
    ExtensionFuture, ExtensionResult, NewSessionCommandOptions, ReplacedSessionContext,
    ReplacedSessionContextPort,
};
use crate::{ContextUsage, ExtensionUIContext, Model, ModelRegistry, ReadonlySessionManager};

use super::signals::signal;

/// Host ordinary context behind the facade.
struct Ordinary<I: Imports> {
    /// The host the context belongs to.
    imports: I,
    /// The host's context.
    context: I::Context,
}

/// Implements shared context capabilities using the selected resource imports.
macro_rules! context_port {
    ($context:ident {
        $compact:ident,
        $ui:ident,
        $has_ui:ident,
        $model_registry:ident,
        $model:ident,
        $is_idle:ident,
        $signal:ident,
        $abort:ident,
        $has_pending_messages:ident,
        $shutdown:ident,
        $get_context_usage:ident,
        $get_system_prompt:ident,
        $session_manager:ident,
        $cwd:ident,
    }) => {
        impl<I: Imports> ContextPort for $context<I> {
            fn compact(&self, options: Option<crate::CompactOptions>) -> ExtensionResult<()> {
                super::compaction::compact(
                    &self.imports,
                    options,
                    |instructions, complete, error| {
                        self.imports
                            .$compact(&self.context, instructions, complete, error)
                    },
                )
            }
            fn ui(&self) -> ExtensionResult<ExtensionUIContext> {
                Ok(super::ui::ui(
                    self.imports.clone(),
                    self.imports.$ui(&self.context)?,
                ))
            }
            fn has_ui(&self) -> ExtensionResult<bool> {
                self.imports.$has_ui(&self.context)
            }
            fn model_registry(&self) -> ExtensionResult<ModelRegistry> {
                Ok(registry(
                    self.imports.clone(),
                    self.imports.$model_registry(&self.context)?,
                ))
            }
            fn model(&self) -> ExtensionResult<Option<Model>> {
                self.imports
                    .$model(&self.context)?
                    .as_deref()
                    .map(decode)
                    .transpose()
            }
            fn is_idle(&self) -> ExtensionResult<bool> {
                self.imports.$is_idle(&self.context)
            }
            fn signal(&self) -> ExtensionResult<Option<AbortSignal>> {
                Ok(self
                    .imports
                    .$signal(&self.context)?
                    .map(|handle| signal(&self.imports, handle)))
            }
            fn abort(&self) -> ExtensionResult<()> {
                self.imports.$abort(&self.context)
            }
            fn has_pending_messages(&self) -> ExtensionResult<bool> {
                self.imports.$has_pending_messages(&self.context)
            }
            fn shutdown(&self) -> ExtensionResult<()> {
                self.imports.$shutdown(&self.context)
            }
            fn get_context_usage(&self) -> ExtensionResult<Option<ContextUsage>> {
                self.imports.$get_context_usage(&self.context)
            }
            fn get_system_prompt(&self) -> ExtensionResult<String> {
                self.imports.$get_system_prompt(&self.context)
            }

            fn session_manager(&self) -> ExtensionResult<ReadonlySessionManager> {
                Ok(reader(
                    self.imports.clone(),
                    self.imports.$session_manager(&self.context)?,
                ))
            }

            fn cwd(&self) -> ExtensionResult<String> {
                self.imports.$cwd(&self.context)
            }
        }
    };
}

context_port! { Ordinary {
    compact,
    ui,
    has_ui,
    model_registry,
    model,
    is_idle,
    signal,
    abort,
    has_pending_messages,
    shutdown,
    get_context_usage,
    get_system_prompt,
    session_manager,
    cwd,
} }

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

context_port! { Command {
    command_compact,
    command_ui,
    command_has_ui,
    command_model_registry,
    command_model,
    command_is_idle,
    command_signal,
    command_abort,
    command_has_pending_messages,
    command_shutdown,
    command_get_context_usage,
    command_get_system_prompt,
    command_session_manager,
    command_cwd,
} }

impl<I: Imports> CommandContextPort for Command<I> {
    fn fork<'a>(
        &'a self,
        entry_id: &'a str,
        options: Option<crate::ForkOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async move {
            let data = options
                .as_ref()
                .map(|options| super::exports::encode_value(&options.data))
                .transpose()?;
            let continuation = options
                .and_then(|options| options.with_session)
                .map(|f| Scoped::new(&self.imports, f));
            self.imports
                .fork(
                    &self.context,
                    entry_id,
                    data,
                    continuation.as_ref().map(|c| &c.handle),
                )
                .await
        })
    }
    fn navigate_tree<'a>(
        &'a self,
        target_id: &'a str,
        options: Option<crate::NavigateTreeOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async move {
            self.imports
                .navigate_tree(
                    &self.context,
                    target_id,
                    options
                        .as_ref()
                        .map(super::exports::encode_value)
                        .transpose()?,
                )
                .await
        })
    }
    fn switch_session<'a>(
        &'a self,
        path: &'a str,
        options: Option<crate::SwitchSessionOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async move {
            let continuation =
                options.map(|options| options.with_session.map(|f| Scoped::new(&self.imports, f)));
            self.imports
                .switch_session(
                    &self.context,
                    path,
                    continuation.as_ref().map(|_| ()),
                    continuation
                        .as_ref()
                        .and_then(|c| c.as_ref().map(|c| &c.handle)),
                )
                .await
        })
    }
    fn reload(&self) -> ExtensionFuture<'_, ()> {
        self.imports.reload(&self.context)
    }

    fn wait_for_idle(&self) -> ExtensionFuture<'_, ()> {
        self.imports.wait_for_idle(&self.context)
    }

    fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionChangeResult> {
        Box::pin(async move {
            let (data, setup, with_session) = options.map_or(
                (None, None, None),
                |NewSessionCommandOptions {
                     data,
                     setup,
                     with_session,
                 }| { (Some(data), setup, with_session) },
            );
            let setup = setup.map(|closure| Scoped::new(&self.imports, closure));
            let with_session = with_session.map(|closure| Scoped::new(&self.imports, closure));
            self.imports
                .new_session(
                    &self.context,
                    data,
                    setup.as_ref().map(|scoped| &scoped.handle),
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
    fn send_message(
        &self,
        message: crate::CustomMessageInput,
        options: Option<crate::SendMessageOptions>,
    ) -> ExtensionFuture<'_, ()> {
        Box::pin(async move {
            self.imports
                .replaced_send_message(
                    &self.context,
                    super::exports::encode_value(&message)?,
                    options
                        .as_ref()
                        .map(super::exports::encode_value)
                        .transpose()?,
                )
                .await
        })
    }
    fn send_user_message(
        &self,
        content: crate::UserContent,
        options: Option<crate::SendUserMessageOptions>,
    ) -> ExtensionFuture<'_, ()> {
        Box::pin(async move {
            self.imports
                .replaced_send_user_message(
                    &self.context,
                    super::exports::encode_value(&content)?,
                    options
                        .as_ref()
                        .map(super::exports::encode_value)
                        .transpose()?,
                )
                .await
        })
    }

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
    ReplacedSessionContext::new(Rc::new(Replaced {
        imports: imports.clone(),
        context,
    }))
}
