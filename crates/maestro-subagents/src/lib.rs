//! Authored agents running in separate Maestro processes.

use maestro_extensions_wasm::{
    AgentToolResult, ExtensionAPI, ExtensionFuture, Presence, ToolDefinition, ToolMetadata,
};
use serde_json::json;
use std::rc::Rc;

/// Registers the authored-agent tool.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_tool(ToolDefinition {
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
            execute: Rc::new(|_, _, _, _, _| {
                Box::pin(async {
                    Ok(AgentToolResult {
                        content: Vec::new(),
                        details: Presence::Missing,
                        terminate: Presence::Missing,
                    })
                })
            }),
        })
    })
}
