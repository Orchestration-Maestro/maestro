//! Registration and invocation of the authored-agent tool.

use crate::agents;
use maestro_extensions_wasm::{
    AgentToolResult, ExtensionAPI, ExtensionContext, ExtensionResult, Presence, TextContent,
    ToolDefinition, ToolMetadata, UserBlock,
};
use serde_json::{Value, json};
use std::rc::Rc;

/// Builds the tool over the supplied host capabilities.
pub(crate) fn tool(api: ExtensionAPI) -> ToolDefinition {
    ToolDefinition {
        metadata: ToolMetadata {
            name: "subagent".into(),
            label: "Subagent".into(),
            description: "Delegate tasks to specialized agents with isolated context.".into(),
            parameters: json!({"type":"object"}).into(),
            prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing,
            render_shell: Presence::Missing,
            execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        render_call: None,
        render_result: None,
        execute: Rc::new(move |_, params, _, _, ctx| {
            let api = api.clone();
            Box::pin(async move { execute(&api, &params, &ctx) })
        }),
    }
}

/// Discovers current definitions before validating the invocation mode.
fn execute(
    api: &ExtensionAPI,
    params: &Value,
    ctx: &ExtensionContext,
) -> ExtensionResult<AgentToolResult> {
    let scope = params
        .get("agentScope")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let discovery = agents::discover_agents(api, &ctx.cwd()?, scope)?;
    let available = discovery
        .agents
        .iter()
        .map(|agent| format!("{} ({})", agent.name, agent.source))
        .collect::<Vec<_>>()
        .join(", ");
    let available = if available.is_empty() {
        "none"
    } else {
        &available
    };
    Ok(AgentToolResult {
        content: vec![UserBlock::Text(TextContent::new(format!(
            "Invalid parameters. Provide exactly one mode.\nAvailable agents: {available}"
        )))],
        details: Presence::Present(
            json!({"mode":"single", "agentScope":scope,
            "projectAgentsDir":discovery.project_agents_dir,"results":[]})
            .to_string()
            .into(),
        ),
        terminate: Presence::Missing,
    })
}
