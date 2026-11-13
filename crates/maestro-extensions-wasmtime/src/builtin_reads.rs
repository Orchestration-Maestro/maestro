//! Store-safe requests to native factory results in the existing capability registry.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    builtin_tools,
    local_resources::{Lease, SignalEndpoint, held::BuiltinTool},
};
use std::sync::Arc;

/// Independently retained native tool identity.
#[derive(Clone)]
pub struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Complete native metadata, including schema conversions.
    pub(crate) fn metadata(&self) -> Result<String, String> {
        self.0
            .run::<BuiltinTool, _>(|definition, _| definition.metadata())
    }

    /// Native argument preparation, without execution or validation.
    pub(crate) fn prepare(
        &self,
        args: String,
    ) -> Result<crate::bindings::maestro::extension::host::PreparedArguments, String> {
        self.0
            .run::<BuiltinTool, _>(move |definition, _| prepare_arguments(&**definition, &args))
    }
}

/// Carries the original mutation even when preparation itself returns a failure.
fn prepare_arguments(
    definition: &dyn builtin_tools::Definition,
    document: &str,
) -> Result<crate::bindings::maestro::extension::host::PreparedArguments, String> {
    let mut args = crate::writer_reads::decode(document)?;
    let prepared = definition
        .prepare(&mut args)
        .and_then(|value| serde_json::to_string(&value).map_err(|error| error.to_string()));
    Ok(
        crate::bindings::maestro::extension::host::PreparedArguments {
            retained: serde_json::to_string(&args).map_err(|error| error.to_string())?,
            prepared,
        },
    )
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
            .admit::<BuiltinTool, _, _>(move |definition, registry| {
                registry.execute_builtin(definition, execution)
            })
            .await
    }
}
