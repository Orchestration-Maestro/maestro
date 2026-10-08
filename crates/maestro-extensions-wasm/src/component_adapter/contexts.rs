//! Generated context, signal and opaque resources behind the author ports.

use std::rc::Rc;

use serde_json::Value;

use super::callbacks::{Identity, Scoped};
use super::session;
use crate::bindings::maestro::extension::host;
use crate::bindings::maestro::extension::host::{SendMessageOptions, SendUserMessageOptions};
use crate::bindings::maestro::extension::session::{ForkData, NavigateTreeOptions};
use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};
use crate::event_bus::CallbackEmitterPort;
use crate::messages::CustomMessageInput;
use crate::model_registry::ModelRegistry;
use crate::models::UserContent;
use crate::session_manager::ReadonlySessionManager;
use crate::types::{
    AbortSignal, AgentToolUpdateCallback, CommandContextPort, CompactOptions, ContextPort,
    ExtensionCommandContext, ExtensionContext, ExtensionFuture, ExtensionResult,
    ExtensionUIContext, ExtensionUIContextPort, ForkOptions, NewSessionCommandOptions,
    ReplacedSessionContext, ReplacedSessionContextPort, SignalPort, SwitchSessionOptions, Theme,
    ThemePort,
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

/// Generated user interface behind the facade.
struct GeneratedUi {
    /// The retained resource.
    _resource: host::UiContext,
}

impl ExtensionUIContextPort for GeneratedUi {}

/// Generated theme behind the facade.
struct GeneratedTheme {
    /// The retained resource.
    _resource: host::Theme,
}

impl ThemePort for GeneratedTheme {}

/// Wraps a generated theme.
pub(super) fn theme(theme: host::Theme) -> Theme {
    Theme::new(Rc::new(GeneratedTheme { _resource: theme }))
}

/// Runs `call` with the callbacks of a compaction request announced to the host; they stay
/// registered until the host releases them, and nothing is retained when the call fails.
fn compact(
    options: Option<CompactOptions>,
    call: impl FnOnce(
        Option<&str>,
        Option<&host::Callback>,
        Option<&host::Callback>,
    ) -> Result<(), String>,
) -> ExtensionResult<()> {
    let CompactOptions {
        custom_instructions,
        on_complete,
        on_error,
    } = options.unwrap_or_default();
    let complete = on_complete.map(|closure| (Identity::new(), closure));
    let failed = on_error.map(|closure| (Identity::new(), closure));
    call(
        custom_instructions.as_deref(),
        complete.as_ref().map(|(identity, _)| &identity.handle),
        failed.as_ref().map(|(identity, _)| &identity.handle),
    )?;
    if let Some((identity, closure)) = complete {
        identity.keep(closure);
    }
    if let Some((identity, closure)) = failed {
        identity.keep(closure);
    }
    Ok(())
}

/// Implements the ordinary context operations for a generated resource that has them.
macro_rules! context_port {
    ($generated:ident) => {
        impl ContextPort for $generated {
            fn ui(&self) -> ExtensionResult<ExtensionUIContext> {
                self.0
                    .ui()
                    .map(|ui| ExtensionUIContext::new(Rc::new(GeneratedUi { _resource: ui })))
            }

            fn has_ui(&self) -> ExtensionResult<bool> {
                self.0.has_ui()
            }

            fn cwd(&self) -> ExtensionResult<String> {
                self.0.cwd()
            }

            fn session_manager(&self) -> ExtensionResult<ReadonlySessionManager> {
                self.0.session_manager().map(session::reader)
            }

            fn model_registry(&self) -> ExtensionResult<ModelRegistry> {
                self.0.model_registry().map(session::registry)
            }

            fn model(&self) -> ExtensionResult<Option<crate::models::Model>> {
                self.0.model()
            }

            fn is_idle(&self) -> ExtensionResult<bool> {
                self.0.is_idle()
            }

            fn signal(&self) -> ExtensionResult<Option<AbortSignal>> {
                self.0.signal().map(|current| current.map(signal))
            }

            fn abort(&self) -> ExtensionResult<()> {
                self.0.abort()
            }

            fn has_pending_messages(&self) -> ExtensionResult<bool> {
                self.0.has_pending_messages()
            }

            fn shutdown(&self) -> ExtensionResult<()> {
                self.0.shutdown()
            }

            fn get_context_usage(
                &self,
            ) -> ExtensionResult<Option<crate::bindings::maestro::extension::session::ContextUsage>>
            {
                self.0.get_context_usage()
            }

            fn compact(&self, options: Option<CompactOptions>) -> ExtensionResult<()> {
                compact(options, |instructions, complete, failed| {
                    self.0.compact(instructions, complete, failed)
                })
            }

            fn get_system_prompt(&self) -> ExtensionResult<String> {
                self.0.get_system_prompt()
            }
        }
    };
}

/// Generated ordinary context behind the facade.
struct GeneratedContext(host::Context);

context_port!(GeneratedContext);

/// Wraps a generated ordinary context.
pub(super) fn context(ctx: host::Context) -> ExtensionContext {
    ExtensionContext::new(Rc::new(GeneratedContext(ctx)))
}

/// Generated command context behind the facade.
struct GeneratedCommand(host::CommandContext);

context_port!(GeneratedCommand);

/// The announcement of an optional continuation of one operation.
fn announced(scoped: Option<&Scoped>) -> Option<&host::Callback> {
    scoped.map(|scoped| &scoped.handle)
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
            let NewSessionCommandOptions {
                data,
                setup,
                with_session,
            } = options.unwrap_or_else(|| NewSessionCommandOptions {
                data: NewSessionCommandData {
                    parent_session: None,
                },
                setup: None,
                with_session: None,
            });
            let (setup, with_session) = (setup.map(Scoped::new), with_session.map(Scoped::new));
            self.0
                .new_session(
                    data,
                    announced(setup.as_ref()),
                    announced(with_session.as_ref()),
                )
                .await
        })
    }

    fn fork<'a>(
        &'a self,
        entry_id: &'a str,
        options: Option<ForkOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async move {
            let ForkOptions { data, with_session } = options.unwrap_or(ForkOptions {
                data: ForkData { position: None },
                with_session: None,
            });
            let with_session = with_session.map(Scoped::new);
            self.0
                .fork(entry_id.to_owned(), data, announced(with_session.as_ref()))
                .await
        })
    }

    fn navigate_tree<'a>(
        &'a self,
        target_id: &'a str,
        options: Option<NavigateTreeOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        let options = options.unwrap_or(NavigateTreeOptions {
            summarize: None,
            custom_instructions: None,
            replace_instructions: None,
            label: None,
        });
        Box::pin(self.0.navigate_tree(target_id.to_owned(), options))
    }

    fn switch_session<'a>(
        &'a self,
        path: &'a str,
        options: Option<SwitchSessionOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async move {
            let with_session = options
                .and_then(|options| options.with_session)
                .map(Scoped::new);
            self.0
                .switch_session(path.to_owned(), announced(with_session.as_ref()))
                .await
        })
    }

    fn reload(&self) -> ExtensionFuture<'_, ()> {
        Box::pin(self.0.reload())
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

    fn send_message(
        &self,
        message: CustomMessageInput,
        options: Option<SendMessageOptions>,
    ) -> ExtensionFuture<'_, ()> {
        Box::pin(self.0.send_message(message, options))
    }

    fn send_user_message(
        &self,
        content: UserContent,
        options: Option<SendUserMessageOptions>,
    ) -> ExtensionFuture<'_, ()> {
        Box::pin(self.0.send_user_message(content, options))
    }
}

/// Wraps a generated replacement context.
pub(super) fn replaced_context(ctx: host::ReplacedSessionContext) -> ReplacedSessionContext {
    ReplacedSessionContext::new(Rc::new(GeneratedReplaced(ctx)))
}

/// Generated emitter behind the facade.
pub(super) struct GeneratedEmitter<'a>(pub(super) &'a host::CallbackEmitter);

impl CallbackEmitterPort for GeneratedEmitter<'_> {
    fn emit(&self, channel: &str, data: Value) -> ExtensionResult<()> {
        self.0.emit(channel, &data.to_string())
    }
}

/// Wraps the progress channel of a tool call as a callback.
pub(super) fn progress(update: host::ToolUpdate) -> AgentToolUpdateCallback {
    Rc::new(move |partial| update.send(&partial))
}
