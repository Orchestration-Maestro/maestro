//! Preparation and asynchronous execution through host imports.
use super::exports::encode_value;
use super::{Exports, Imports, callbacks, contexts};
use crate::bindings::exports::maestro::extension::guest::ToolInvocation;
use crate::{
    AgentToolUpdateCallback, ExtensionResult, ParameterSchema, PrepareArguments, ToolExecute,
};
use std::rc::Rc;

/// Member carrying the declared schema kinds beside the plain payload.
const SCHEMA_KINDS: &str = "schemaKinds";

/// Encode registration metadata for the host, with the kinds its author declared.
pub(super) fn encode_metadata(metadata: &crate::ToolMetadata) -> ExtensionResult<String> {
    let mut value = serde_json::to_value(metadata).map_err(|error| error.to_string())?;
    let kinds = serde_json::to_value(metadata.parameters.conversions())
        .map_err(|error| error.to_string())?;
    match value.as_object_mut() {
        Some(object) => {
            object.insert(SCHEMA_KINDS.to_owned(), kinds);
        }
        None => return Err("tool metadata is not an object".into()),
    }
    encode_value(&value)
}

/// The declared schema kinds one transport record carries.
///
/// Every other member is skipped without building a value, so a deep or repeated record the
/// host selected keeps its own decoding outcome.
#[derive(Default, serde::Deserialize)]
struct CarriedKinds {
    /// Authored kinds by JSON pointer.
    #[serde(rename = "schemaKinds", default)]
    schema_kinds: std::collections::HashMap<String, crate::SchemaKind>,
}

/// Decode enumerated tools, restoring the schema kinds the transport carries.
///
/// Kinds travel only in the object form this adapter encodes; a record the host supplies in
/// another admitted shape carries none and stays plain.
pub(super) fn decode_tools(text: &str) -> ExtensionResult<Vec<crate::ToolInfo>> {
    let tools: Vec<crate::ToolInfo> =
        serde_json::from_str(text).map_err(|error| error.to_string())?;
    let carried = serde_json::from_str::<Vec<CarriedKinds>>(text).unwrap_or_default();
    if carried.len() != tools.len() {
        return Ok(tools);
    }
    Ok(tools
        .into_iter()
        .zip(carried)
        .map(|(mut tool, carried)| {
            tool.parameters =
                ParameterSchema::authored(tool.parameters.into_json(), carried.schema_kinds);
            tool
        })
        .collect())
}

/// Owned resources delivered to one tool invocation.
pub(crate) struct ToolCapabilities<I: Imports> {
    /// Ordinary context.
    pub(crate) ctx: I::Context,
    /// Optional cancellation resource.
    pub(crate) signal: Option<I::Signal>,
    /// Optional progress resource.
    pub(crate) update: Option<I::Update>,
}
impl<I: Imports> Exports<I> {
    /// Transforms arguments synchronously using the registered preparation.
    pub(crate) fn invoke_prepare(
        &self,
        handler: u32,
        args: &str,
        emitter: &I::Emitter,
    ) -> ExtensionResult<String> {
        let prepare = callbacks::find::<PrepareArguments>(handler)?;
        let args = serde_json::from_str(args).map_err(|error| error.to_string())?;
        let port = super::event_bus::Emitter {
            imports: &self.imports,
            handle: emitter,
        };
        encode_value(&prepare(args, crate::CallbackEmitter::new(&port))?)
    }
    /// Awaits the registered execution with owned resource facades.
    pub(crate) async fn invoke_tool(
        &self,
        handler: u32,
        invocation: ToolInvocation,
        resources: ToolCapabilities<I>,
    ) -> ExtensionResult<String> {
        let execute = callbacks::find::<ToolExecute>(handler)?;
        let params = serde_json::from_str(&invocation.params).map_err(|error| error.to_string())?;
        let signal = resources
            .signal
            .map(|signal| contexts::signal(&self.imports, signal));
        let update = resources.update.map(|update| {
            let imports = self.imports.clone();
            Rc::new(move |partial| imports.tool_update(&update, &encode_value(&partial)?))
                as AgentToolUpdateCallback
        });
        let result = execute(
            invocation.tool_call_id,
            params,
            signal,
            update,
            contexts::context(&self.imports, resources.ctx),
        )
        .await?;
        encode_value(&result)
    }
}
