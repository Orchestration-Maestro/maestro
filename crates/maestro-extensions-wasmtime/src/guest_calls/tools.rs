//! Real native tool callbacks retain their original operands and invocation frame.
use super::{Call, Handle};
use crate::lifecycle::replies::Reply;
use crate::{
    context_reads::{Endpoint, Owner},
    local_resources::{
        Registry, SignalEndpoint,
        effects::{EmitterEndpoint, ToolUpdate},
    },
};
use maestro_extensions::{
    DiagnosticErrorInfo, EventBus, Local, PrepareArguments, local::ExecuteToolDefinition,
};
use serde_json::Value;
use std::rc::Rc;

/// Typed tool invocations consumed by the original Store driver.
pub(crate) enum ToolCall<H = Handle> {
    /// Synchronous argument preparation under the original native callback frame.
    Prepare {
        /// Retains authored closure ownership and admission.
        handler: H,
        /// Exact argument document.
        args: String,
        /// Original native bus and deferred deliveries.
        emitter: EmitterEndpoint,
        /// Returns the actual prepared document, independently of completion.
        reply: Reply<crate::guest_exports::tools::PreparedArguments>,
    },
    /// Asynchronous execution with independently retained caller capabilities.
    Execute {
        /// Retains authored closure ownership and admission.
        handler: H,
        /// Literal invocation identifier.
        id: String,
        /// Exact serialized arguments.
        params: String,
        /// Original context selected by the native wrapper for this invocation.
        context: Endpoint,
        /// Original independently optional cancellation.
        signal: Option<SignalEndpoint>,
        /// Original independently optional progress observer.
        update: Option<ToolUpdate>,
        /// Returns the guest's actual result or original failure.
        reply: Reply<String>,
    },
}

impl Handle {
    /// Enters preparation synchronously while the same receiver services nested imports.
    pub(crate) fn prepare(
        &self,
        registry: Registry,
        events: EventBus<Local>,
    ) -> PrepareArguments<Local> {
        let handler = self.clone();
        Rc::new(move |args| {
            events
                .invoke_callback(|emitter| {
                    handler.prepared(&registry, args, registry.emitter(emitter.frame())?)
                })
                .map_err(DiagnosticErrorInfo::error)
        })
    }

    /// Returns prepared arguments only after the original frame's guest invocation settles.
    fn prepared(
        &self,
        registry: &Registry,
        args: &mut Value,
        emitter: EmitterEndpoint,
    ) -> Result<Value, String> {
        let document = serde_json::to_string(args).map_err(|error| error.to_string())?;
        let result = self.0.endpoint.synchronous(registry, |reply| {
            Ok(Call::Tool(ToolCall::Prepare {
                handler: self.clone(),
                args: document,
                emitter,
                reply,
            }))
        })?;
        *args = crate::writer_reads::decode(&result.retained)?;
        crate::writer_reads::decode(&result.prepared?)
    }

    /// Keeps invocation local and requires the actual context supplied by the native wrapper.
    pub(crate) fn execute(
        self,
        registry: Registry,
    ) -> ExecuteToolDefinition<maestro_extensions::ExtensionContext, Value, Value> {
        Rc::new(move |id, params, signal, update, context| {
            let resources = execution_resources(&registry, context, signal, update);
            let params = serde_json::to_string(&params).map_err(|error| error.to_string());
            Box::pin(invoke_tool(
                self.clone(),
                registry.clone(),
                id,
                params,
                resources,
            ))
        })
    }
}

/// Independently retained per-call context, cancellation and update operands.
type Resources = (Endpoint, Option<SignalEndpoint>, Option<ToolUpdate>);

/// Registers each optional operand independently, without manufacturing a context.
fn execution_resources(
    registry: &Registry,
    context: Option<maestro_extensions::ExtensionContext>,
    signal: Option<maestro_extensions::Cancellation>,
    update: Option<maestro_extensions::AgentToolUpdateCallback<Value, Local>>,
) -> Result<Resources, String> {
    Ok((
        registry.context(Owner::Base(context.ok_or("extension tool context absent")?))?,
        signal.map(|signal| registry.signal(signal)).transpose()?,
        update
            .map(|update| registry.tool_update(update))
            .transpose()?,
    ))
}

/// Awaits the original authored tool under the shared lifecycle submission owner.
async fn invoke_tool(
    handler: Handle,
    registry: Registry,
    id: String,
    params: Result<String, String>,
    resources: Result<Resources, String>,
) -> Result<maestro_extensions::AgentToolResult<Value>, DiagnosticErrorInfo> {
    let (context, signal, update) = resources.map_err(DiagnosticErrorInfo::error)?;
    let params = params.map_err(|error| DiagnosticErrorInfo::error(&error))?;
    let result = handler
        .0
        .endpoint
        .call(&registry, |reply| {
            Call::Tool(ToolCall::Execute {
                handler: handler.clone(),
                id,
                params,
                context,
                signal,
                update,
                reply,
            })
        })
        .await
        .map_err(DiagnosticErrorInfo::error)?;
    crate::writer_reads::decode(&result).map_err(DiagnosticErrorInfo::error)
}
