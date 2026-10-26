//! Conversation records are adapted to the existing projection and serialization owners.
use super::{decode, encode};
use serde_json::Value;
use std::sync::{Arc, RwLock};
/// A caller-defined role the native projection deliberately does not consume.
struct ForeignRole(
    /// The authored open role identifier.
    String,
);
impl maestro_agent::CustomAgentMessages for ForeignRole {
    fn role(&self) -> &str {
        &self.0
    }
}
/// Original native application records, specialized only with the caller's open role.
type NativeCustom = maestro_session::messages::CustomAgentMessages<Value, ForeignRole>;
/// Original retained native record shape.
type NativeMessage = maestro_agent::AgentMessage<NativeCustom>;
/// Runs one native conversation operation.
pub(super) fn call(operation: &str, data: &str) -> Result<String, String> {
    match operation {
        "conversation.convert_to_llm" => {
            let (messages,): (Vec<Value>,) = decode(data)?;
            let entries = messages
                .into_iter()
                .map(entry)
                .collect::<Result<Vec<_>, _>>()?;
            encode(&maestro_session::messages::convert_to_llm(&entries))
        }
        "conversation.serialize_conversation" => {
            let (messages,): (Vec<maestro_models::Message>,) = decode(data)?;
            encode(
                &maestro_session::compaction::utils::serialize_conversation(&messages)
                    .map_err(|error| error.to_string())?,
            )
        }
        _ => Err(format!("Unknown native helper: {operation}")),
    }
}
/// Marshals ownership only; projection policy and every transcript template stay native.
fn entry(value: Value) -> Result<NativeMessage, String> {
    let role = value["role"]
        .as_str()
        .ok_or("Missing message role")?
        .to_owned();
    Ok(match role.as_str() {
        "user" => NativeMessage::User(Arc::new(RwLock::new(record(value)?))),
        "assistant" => NativeMessage::Assistant(Arc::new(RwLock::new(record(value)?))),
        "toolResult" => NativeMessage::ToolResult(Arc::new(RwLock::new(record(value)?))),
        "bashExecution" => custom(NativeCustom::BashExecution(record(value)?)),
        "custom" => custom(NativeCustom::Custom(record(value)?)),
        "branchSummary" => custom(NativeCustom::BranchSummary(record(value)?)),
        "compactionSummary" => custom(NativeCustom::CompactionSummary(record(value)?)),
        _ => custom(NativeCustom::Other(ForeignRole(role))),
    })
}
/// Retains a custom record in the domain's required ownership representation.
fn custom(value: NativeCustom) -> NativeMessage {
    NativeMessage::Custom(Arc::new(RwLock::new(value)))
}
/// Reads the selected native record without inventing defaults or weakening validation.
fn record<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|error| error.to_string())
}
