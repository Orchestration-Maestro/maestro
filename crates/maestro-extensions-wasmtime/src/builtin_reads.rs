//! Store-safe requests to native factory results in the existing capability registry.
use crate::{
    builtin_tools,
    local_resources::{Lease, Message, SignalEndpoint},
};
use std::sync::{Arc, mpsc::Sender};

/// Independently retained native tool identity.
#[derive(Clone)]
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);

/// Synchronous reads of the original native definition.
pub(crate) enum Request {
    /// Complete native metadata, including schema conversions.
    Metadata(Sender<Result<String, String>>),
    /// Native argument preparation, without execution or validation.
    Prepare(
        String,
        Sender<Result<crate::guest_exports::tools::PreparedArguments, String>>,
    ),
}
impl Request {
    /// Uses the native definition after leaving the capability-table borrow.
    pub(crate) fn answer(self, definition: &dyn builtin_tools::Definition) {
        match self {
            Self::Metadata(reply) => {
                let _ = reply.send(definition.metadata());
            }
            Self::Prepare(args, reply) => {
                let result = prepare_arguments(definition, &args);
                let _ = reply.send(result);
            }
        }
    }
    /// Settles a request whose original owner has ended.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::Metadata(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Prepare(_, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
        }
    }
}

/// Carries the original mutation even when preparation itself returns a failure.
fn prepare_arguments(
    definition: &dyn builtin_tools::Definition,
    document: &str,
) -> Result<crate::guest_exports::tools::PreparedArguments, String> {
    let mut args = crate::writer_reads::decode(document)?;
    let prepared = definition
        .prepare(&mut args)
        .and_then(|value| serde_json::to_string(&value).map_err(|error| error.to_string()));
    Ok(crate::guest_exports::tools::PreparedArguments {
        retained: serde_json::to_string(&args).map_err(|error| error.to_string())?,
        prepared,
    })
}

/// Owned execution operands; capability identities never enter the JSON document.
pub(crate) struct Execution {
    /// Literal call identity.
    pub(crate) id: String,
    /// Original argument document.
    pub(crate) args: String,
    /// Model observed from the supplied author context at execution time.
    pub(crate) model: Option<String>,
    /// Independently retained cancellation.
    pub(crate) signal: Option<SignalEndpoint>,
    /// Independently retained operation-scoped progress capability.
    pub(crate) update: Option<crate::guest_calls::Handle>,
    /// Original facade supplies the existing synchronous callback frame and error destination.
    pub(crate) api: crate::api_reads::Endpoint,
}
impl Endpoint {
    /// Admits asynchronous native execution on the original local pump.
    pub(crate) async fn execute(&self, execution: Execution) -> Result<String, String> {
        self.0
            .request_async(|id, reply, admission| {
                Message::BuiltinOperation(crate::local_resources::builtin::Operation::Execute(
                    id, execution, reply, admission,
                ))
            })
            .await
    }
}
