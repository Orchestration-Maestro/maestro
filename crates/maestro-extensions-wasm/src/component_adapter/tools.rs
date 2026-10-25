//! Preparation and asynchronous execution through host imports.
use super::exports::encode_value;
use super::{Exports, Imports, callbacks, contexts};
use crate::bindings::exports::maestro::extension::guest::ToolInvocation;
use crate::{
    AgentToolUpdateCallback, ExtensionResult, ParameterSchema, PrepareArguments, ToolExecute,
};
use serde::Deserialize;
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
    let mut deserializer = serde_json::Deserializer::from_str(text);
    deserializer.disable_recursion_limit();
    let tools = Vec::<crate::ToolInfo>::deserialize(&mut deserializer)
        .map_err(|error| error.to_string())?;
    deserializer.end().map_err(|error| error.to_string())?;
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

#[cfg(test)]
mod tests {
    /// Empty inventories and native trailing-input rejection keep their outcomes.
    #[test]
    fn metadata_enumeration_preserves_native_admission() {
        assert!(super::decode_tools("[]").unwrap().is_empty());
        assert!(super::decode_tools("[] \n\t").unwrap().is_empty());
        assert_eq!(
            super::decode_tools("[] {}").unwrap_err(),
            "trailing characters at line 1 column 4"
        );
    }

    /// Complete metadata retains the supplied schema and all of its declared kinds.
    #[test]
    fn metadata_enumeration_admits_nested_parameters() {
        for depth in [16, 32, 62, 63, 64] {
            let parameters = crate::tests::author::nested_schema_fixture::parameters(&format!(
                "authored-{depth}"
            ))
            .unwrap();
            let text = serde_json::json!([{
                "name": "nested", "description": "", "parameters": parameters,
                "sourceInfo": {"path":"/author","source":"custom","scope":"temporary","origin":"top-level"},
                "schemaKinds": parameters.conversions(),
            }]).to_string();
            let listed =
                super::decode_tools(&text).unwrap_or_else(|error| panic!("depth {depth}: {error}"));
            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].parameters.data(), parameters);
        }
    }
}
