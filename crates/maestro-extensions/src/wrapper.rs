//! Tools-owned wrapping with fresh runner contexts.
use crate::types::registration::tools::RetainedDefinition;
use crate::{ExtensionExecution, ExtensionRunner, types::RegisteredTool};
use maestro_models::Local;
use std::rc::Rc;
/// Delegate to the same lower wrapper and real context owner.
pub fn wrap_registered_tool<E: ExtensionExecution>(
    tool: &RegisteredTool<E>,
    runner: Rc<ExtensionRunner<E>>,
) -> maestro_agent::AgentToolRecord<Local> {
    tool.definition
        .wrap(Rc::new(move || Ok(runner.create_context())))
}
/// Preserve registration order through singular wrapping.
pub fn wrap_registered_tools<E: ExtensionExecution>(
    tools: &[RegisteredTool<E>],
    runner: &Rc<ExtensionRunner<E>>,
) -> Vec<maestro_agent::AgentToolRecord<Local>> {
    tools
        .iter()
        .map(|tool| wrap_registered_tool(tool, runner.clone()))
        .collect()
}
