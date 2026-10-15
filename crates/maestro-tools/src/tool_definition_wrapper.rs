//! Bridge between tool definitions and executable agent tools.

use crate::types::{ToolContextFactory, ToolDefinition};
use maestro_agent::{AgentTool, ExecuteTool};
use maestro_models::DiagnosticErrorInfo;
use serde::{Serialize, de::DeserializeOwned};
use std::sync::Arc;

/// Wrap a definition as an agent tool.
///
/// Once the arguments decode (see `AgentTool::typed` for rejected arguments), each
/// execution asks the factory for a fresh context; a factory failure is the
/// execution's error and the definition body does not run.
#[must_use]
pub fn wrap_tool_definition<C, P, D, S>(
    definition: ToolDefinition<C, P, D, S>,
    context_factory: Option<ToolContextFactory<C>>,
) -> AgentTool
where
    C: 'static,
    P: DeserializeOwned + 'static,
    D: Serialize + 'static,
{
    let execute = definition.execute;
    let run: ExecuteTool<P, D> = Arc::new(move |id, params, signal, update| {
        let context = match context_factory.as_ref().map(|factory| factory()) {
            Some(Err(error)) => return Box::pin(async move { Err(error) }),
            Some(Ok(context)) => Some(context),
            None => None,
        };
        execute(id, params, signal, update, context)
    });
    let mut tool = AgentTool::typed(definition.definition, definition.label, run);
    tool.prepare_arguments = definition.prepare_arguments;
    tool.execution_mode = definition.execution_mode;
    tool
}

/// Wrap each definition in order, sharing one context factory.
#[must_use]
pub fn wrap_tool_definitions<C, P, D, S>(
    definitions: Vec<ToolDefinition<C, P, D, S>>,
    context_factory: Option<ToolContextFactory<C>>,
) -> Vec<AgentTool>
where
    C: 'static,
    P: DeserializeOwned + 'static,
    D: Serialize + 'static,
{
    definitions
        .into_iter()
        .map(move |definition| wrap_tool_definition(definition, context_factory.clone()))
        .collect()
}

/// Snapshot an agent tool as a definition with no prompt text or renderers.
///
/// # Errors
/// Returns the live description callback's original diagnostic.
pub fn create_tool_definition_from_agent_tool<C: 'static>(
    tool: AgentTool,
) -> Result<ToolDefinition<C>, DiagnosticErrorInfo> {
    let execute = tool.execute;
    let definition = ToolDefinition {
        definition: AgentTool::resolve_definition(tool.definition, tool.description)?,
        label: tool.label,
        prompt_snippet: None,
        prompt_guidelines: None,
        render_shell: None,
        prepare_arguments: tool.prepare_arguments,
        execution_mode: tool.execution_mode,
        execute: Arc::new(move |id, params, signal, update, _| execute(id, params, signal, update)),
        render_call: None,
        render_result: None,
    };
    Ok(definition)
}
