//! Callback-bearing registrations use the original native facade and record owners.
use crate::{
    api_reads::Endpoint,
    guest_calls::Handle,
    local_resources::{Message, Registry},
};
use maestro_extensions::{
    ExtensionAPI, Local,
    types::{ExtensionShortcutOptions, RegisteredCommandOptions},
};

/// Native registration completion, preserving the facade's exact error text.
type Reply = std::sync::mpsc::Sender<Result<(), String>>;

/// Owned canonical registration operands, never native callback substitutes.
pub(crate) enum Request {
    /// Registers the actual Local renderer without a reporter projection.
    Renderer(String, Handle, Reply),
    /// Registers a concrete known event or retains an opaque unknown callback.
    On(String, Handle, Reply),
    /// Registers an authored tool with its real native definition owner.
    Tool {
        /// Exact authored metadata document.
        metadata: String,
        /// Independently optional preparation callback.
        prepare: Option<Handle>,
        /// Actual execution callback.
        execute: Handle,
        /// Catchable native registration result.
        reply: Reply,
    },
    /// Registers one command in the native owner's ordered slot.
    Command {
        /// Literal command identity.
        name: String,
        /// Independently optional description.
        description: Option<String>,
        /// Optional actual authored completion callback.
        completions: Option<Handle>,
        /// Actual authored invocation callback.
        handler: Handle,
        /// Catchable native registration result.
        reply: Reply,
    },
    /// Registers a raw native shortcut identifier.
    Shortcut {
        /// Literal shortcut, not a parsed platform key event.
        key: String,
        /// Independently optional description.
        description: Option<String>,
        /// Actual authored ordinary-context callback.
        handler: Handle,
        /// Catchable native registration result.
        reply: Reply,
    },
}

impl Request {
    /// Calls native registration outside all resource-table borrows.
    pub(crate) fn answer(self, api: &ExtensionAPI<Local>, registry: &Registry) {
        let (result, reply) = match self {
            Self::Renderer(name, handler, reply) => {
                let renderer = handler.renderer(registry.clone(), api.events.clone());
                (api.register_message_renderer(name, renderer), reply)
            }
            Self::On(name, handler, reply) => {
                let result =
                    crate::event_registration::register(api, name, handler, registry.clone());
                let _ = reply.send(result);
                return;
            }
            Self::Tool {
                metadata,
                prepare,
                execute,
                reply,
            } => {
                let result = crate::tool_registration::definition(
                    &metadata,
                    prepare,
                    execute,
                    registry.clone(),
                    api.events.clone(),
                )
                .and_then(|definition| {
                    api.register_tool(definition)
                        .map_err(|error| error.to_string())
                });
                let _ = reply.send(result);
                return;
            }
            Self::Command {
                name,
                description,
                completions,
                handler,
                reply,
            } => {
                let options = RegisteredCommandOptions {
                    description,
                    get_argument_completions: completions.map(|handler| handler.completions()),
                    handler: handler.command(registry.clone()),
                };
                (api.register_command(name, options), reply)
            }
            Self::Shortcut {
                key,
                description,
                handler,
                reply,
            } => {
                let options = ExtensionShortcutOptions {
                    description,
                    handler: handler.shortcut(registry.clone()),
                };
                (api.register_shortcut(key, options), reply)
            }
        };
        let _ = reply.send(result.map_err(|error| error.to_string()));
    }

    /// Settles the caller when its original API capability has ended.
    pub(crate) fn reject(self, error: &str) {
        let (Self::Command { reply, .. }
        | Self::Shortcut { reply, .. }
        | Self::Tool { reply, .. }
        | Self::On(_, _, reply)
        | Self::Renderer(_, _, reply)) = self;
        let _ = reply.send(Err(error.to_owned()));
    }
}

impl Endpoint {
    /// Uses the sole local receiver, retaining the actual registration facade.
    pub(crate) fn registration(
        &self,
        request: impl FnOnce(Reply) -> Request,
    ) -> Result<(), String> {
        self.0
            .request(|id, reply| Message::Registration(id, request(reply)))
    }
}
