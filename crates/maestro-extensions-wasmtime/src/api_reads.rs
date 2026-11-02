//! API requests delegate to the real registration facade and its live runtime.
use crate::{
    local_resources::{Lease, Message},
    writer_reads::decode,
};
use maestro_extensions::{
    ExtensionAPI, Local,
    types::{ExtensionFlagOptions, FlagValue},
};
use std::sync::{Arc, mpsc};

/// A catchable result after native registration or action dispatch.
type Reply<T> = mpsc::Sender<Result<T, String>>;

/// Operations on the original extension API and its current runtime.
pub(crate) enum Request {
    /// Registers a provider through the facade's loading or bound action route.
    RegisterProvider(
        String,
        String,
        crate::provider_registration::Callbacks,
        Reply<()>,
    ),
    /// Removes every queued occurrence or the bound provider.
    UnregisterProvider(String, Reply<()>),
    /// Offers JSON input through the original base registration owner's load-time channel.
    ContributeLoadTime(String, Reply<()>),
    /// Registers metadata and seeds only the native owner's missing flag value.
    RegisterFlag(String, ExtensionFlagOptions, Reply<()>),
    /// Reads the host-resolved value for a flag registered by this extension.
    Flag(String, Reply<Option<FlagValue>>),
    /// Appends custom data, preserving absent versus JSON null.
    Append(String, Option<String>, Reply<()>),
    /// Sends the exact custom message and independently optional delivery choices.
    SendMessage(String, Option<String>, Reply<()>),
    /// Sends text or ordered blocks with independently optional delivery choices.
    SendUserMessage(String, Option<String>, Reply<()>),
    /// Sets the exact session name through the bound operation.
    SetSessionName(String, Reply<()>),
    /// Reads the selected operation's optional name.
    SessionName(Reply<Option<String>>),
    /// Sets or clears an exact entry label.
    Label(String, Option<String>, Reply<()>),
    /// Reads active tool identities in their native order.
    ActiveTools(Reply<Vec<String>>),
    /// Selects the authored tool identities without normalization.
    SetActiveTools(Vec<String>, Reply<()>),
    /// Reads current command metadata with provenance.
    Commands(Reply<String>),
    /// Reads the native thinking-level spelling.
    Thinking(Reply<String>),
    /// Sets the supplied native thinking level.
    SetThinking(String, Reply<()>),
}

/// Requests answered against the original API after releasing capability storage.
pub(crate) trait ApiRequest {
    /// Dispatch against the original facade and capability owner.
    fn answer(self, api: &ExtensionAPI<Local>, registry: &crate::local_resources::Registry);
    /// Settle when that original capability has ended.
    fn reject(self, error: &str);
}

impl ApiRequest for Request {
    /// Calls the real API after the pump releases its capability-table borrow.
    fn answer(self, api: &ExtensionAPI<Local>, registry: &crate::local_resources::Registry) {
        match self {
            Self::RegisterProvider(name, config, callbacks, reply) => {
                let result = crate::provider_registration::config(&config, callbacks, registry)
                    .and_then(|config| {
                        api.register_provider(name, config)
                            .map_err(|e| e.to_string())
                    });
                let _ = reply.send(result);
            }
            Self::UnregisterProvider(name, reply) => send(&reply, api.unregister_provider(&name)),
            Self::ContributeLoadTime(payload, reply) => {
                let result = decode(&payload).and_then(|inputs| {
                    api.contribute_load_time(inputs)
                        .map_err(|error| error.to_string())
                });
                let _ = reply.send(result);
            }
            Self::RegisterFlag(name, options, reply) => {
                send(&reply, api.register_flag(name, options));
            }
            Self::Flag(name, reply) => send(&reply, api.get_flag(&name)),
            Self::Append(name, data, reply) => {
                let result = optional(data.as_deref())
                    .and_then(|data| api.append_entry(name, data).map_err(|e| e.to_string()));
                let _ = reply.send(result);
            }
            Self::SendMessage(message, options, reply) => {
                let result =
                    delivery(&message, options.as_deref()).and_then(|(message, options)| {
                        api.send_message(message, options)
                            .map_err(|e| e.to_string())
                    });
                let _ = reply.send(result);
            }
            Self::SendUserMessage(content, options, reply) => {
                let result =
                    delivery(&content, options.as_deref()).and_then(|(content, options)| {
                        api.send_user_message(content, options)
                            .map_err(|e| e.to_string())
                    });
                let _ = reply.send(result);
            }
            Self::SetSessionName(name, reply) => send(&reply, api.set_session_name(name)),
            Self::SessionName(reply) => send(&reply, api.get_session_name()),
            Self::Label(id, label, reply) => send(&reply, api.set_label(id, label)),
            Self::ActiveTools(reply) => send(&reply, api.get_active_tools()),
            Self::SetActiveTools(names, reply) => send(&reply, api.set_active_tools(names)),
            Self::Commands(reply) => send_json(&reply, api.get_commands()),
            Self::Thinking(reply) => send_json(&reply, api.get_thinking_level()),
            Self::SetThinking(level, reply) => {
                let result = decode(&level)
                    .and_then(|level| api.set_thinking_level(level).map_err(|e| e.to_string()));
                let _ = reply.send(result);
            }
        }
    }

    /// Settles missing API resources without stranding the waiting Store caller.
    fn reject(self, error: &str) {
        match self {
            Self::RegisterProvider(_, _, _, reply)
            | Self::UnregisterProvider(_, reply)
            | Self::RegisterFlag(_, _, reply)
            | Self::ContributeLoadTime(_, reply)
            | Self::Append(_, _, reply)
            | Self::SendMessage(_, _, reply)
            | Self::SendUserMessage(_, _, reply)
            | Self::SetSessionName(_, reply)
            | Self::Label(_, _, reply)
            | Self::SetActiveTools(_, reply)
            | Self::SetThinking(_, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Flag(_, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::SessionName(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::ActiveTools(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Commands(reply) | Self::Thinking(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
        }
    }
}

/// Decodes an optional record without collapsing absent and present-empty values.
pub(crate) fn optional<T: serde::de::DeserializeOwned>(
    value: Option<&str>,
) -> Result<Option<T>, String> {
    value.map(decode).transpose()
}

/// Decodes message content and independently optional delivery choices once.
pub(crate) fn delivery<T: serde::de::DeserializeOwned, O: serde::de::DeserializeOwned>(
    value: &str,
    options: Option<&str>,
) -> Result<(T, Option<O>), String> {
    Ok((decode(value)?, optional(options)?))
}

/// Preserves the domain's exact authored failure text.
fn send<T>(reply: &Reply<T>, result: Result<T, maestro_extensions::EventError>) {
    let _ = reply.send(result.map_err(|error| error.to_string()));
}

/// Encodes native metadata without applying a second inventory projection.
fn send_json<T: serde::Serialize>(
    reply: &Reply<String>,
    result: Result<T, maestro_extensions::EventError>,
) {
    let result = result
        .map_err(|error| error.to_string())
        .and_then(|value| serde_json::to_string(&value).map_err(|error| error.to_string()));
    let _ = reply.send(result);
}

/// Store-safe ownership of the original local API, not a new registration owner.
#[derive(Clone)]
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Select live schema origins through the actual native declarations.
    pub(crate) fn schema_listing(&self) -> Result<crate::schema_registration::ToolListing, String> {
        self.0.request(Message::SchemaListing)
    }
    /// Retains the same local pump while a native callback remains callable.
    pub(crate) fn admission(&self) -> crate::local_resources::Admission {
        self.0.admission()
    }

    /// Allocates an operation's signal through the original local owner.
    pub(crate) fn new_signal(&self) -> Result<crate::local_resources::SignalEndpoint, String> {
        self.0.request(|_, reply| Message::NewSignal(None, reply))
    }

    /// Admits explicit execution on the original local runtime.
    pub(crate) async fn exec(&self, request: crate::exec::Request) -> Result<String, String> {
        self.0
            .request_async(|id, reply, admission| Message::Exec(id, request, reply, admission))
            .await
    }

    /// Admits the selected model operation on the original local runtime.
    pub(crate) async fn set_model(&self, model: maestro_extensions::Model) -> Result<bool, String> {
        self.0
            .request_async(|id, reply, admission| {
                Message::ApiModel(id, Box::new(model), reply, admission)
            })
            .await
    }
}
