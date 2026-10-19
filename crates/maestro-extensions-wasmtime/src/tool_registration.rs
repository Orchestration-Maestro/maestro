//! Authored metadata becomes the real native local tool definition.
use crate::{guest_calls::Handle, local_resources::Registry, writer_reads::decode};
use maestro_extensions::{
    EventBus, Local, Presence, Tool, ToolExecutionMode, ToolRenderShell, local::ToolDefinition,
};
use serde::Deserialize;
use serde_json::Value;

/// Canonical metadata document, independent of retained callback ownership.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    /// Literal model-facing name.
    name: String,
    /// Literal display label.
    label: String,
    /// Literal model-facing description.
    description: String,
    /// Authored semantic JSON schema.
    parameters: Value,
    /// Independently optional prompt text.
    #[serde(default)]
    prompt_snippet: Presence<String>,
    /// Independently optional prompt guidance.
    #[serde(default)]
    prompt_guidelines: Presence<Vec<String>>,
    /// Independently optional frame policy.
    #[serde(default)]
    render_shell: Presence<Shell>,
    /// Independently optional scheduling preference.
    #[serde(default)]
    execution_mode: Presence<ToolExecutionMode>,
}

/// Wire spelling of the existing native frame choice.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Shell {
    /// Host-owned frame.
    Default,
    /// Authored frame.
    #[serde(rename = "self")]
    SelfRendered,
}

/// Converts only present authored metadata into the native optional operand.
fn optional<T>(value: Presence<T>) -> Option<T> {
    match value {
        Presence::Missing | Presence::Null => None,
        Presence::Present(value) => Some(value),
    }
}

/// Retains actual prepare/execute closures without supplying a context factory at registration.
pub(crate) fn definition(
    metadata: &str,
    prepare: Option<Handle>,
    execute: Handle,
    registry: Registry,
    events: EventBus<Local>,
) -> Result<ToolDefinition<maestro_extensions::ExtensionContext, Value, Value, ()>, String> {
    let metadata: Metadata = decode(metadata)?;
    let prepare_arguments = prepare.map(|prepare| prepare.prepare(registry.clone(), events));
    Ok(ToolDefinition {
        definition: Tool {
            name: metadata.name,
            description: metadata.description,
            parameters: metadata.parameters,
        },
        label: metadata.label,
        prompt_snippet: optional(metadata.prompt_snippet),
        prompt_guidelines: optional(metadata.prompt_guidelines),
        render_shell: optional(metadata.render_shell).map(|shell| match shell {
            Shell::Default => ToolRenderShell::Default,
            Shell::SelfRendered => ToolRenderShell::SelfRendered,
        }),
        execution_mode: optional(metadata.execution_mode),
        prepare_arguments,
        execute: execute.execute(registry),
        render_call: None,
        render_result: None,
    })
}
