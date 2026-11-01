//! Registered echo definition shared by native tool witnesses.
#![cfg(test)]
use maestro_extensions::ExtensionContext;
use maestro_models::{Local, Tool};
use maestro_tools::ToolDefinitionRecord;
use serde_json::{Value, json};
use std::rc::Rc;
/// A retained echo tool returning the supplied arguments as details.
pub fn definition(
    name: &str,
    label: &str,
) -> ToolDefinitionRecord<ExtensionContext, Value, Value, (), Local, Local> {
    ToolDefinitionRecord {
        definition: Tool {
            name: name.into(),
            description: "registered".into(),
            parameters: json!({"type":"object"}).into(),
        },
        label: label.into(),
        prompt_snippet: None,
        prompt_guidelines: None,
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: Rc::new(|_, args, _, _, _| {
            Box::pin(async move {
                Ok(maestro_agent::AgentToolResult {
                    content: vec![],
                    details: Some(args),
                    terminate: None,
                })
            })
        }),
        render_call: None,
        render_result: None,
    }
}
