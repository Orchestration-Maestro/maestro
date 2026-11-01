//! Private typed retention and tools-owned projections.
use super::{
    ContextFactory, ExtensionContext, ToolDefinitionInput, ToolExecution, ToolRendererBinding,
};
use crate::types::read;
use maestro_models::Local;
use serde::{Serialize, de::DeserializeOwned};
use std::sync::RwLock;
/// Declaration fields observed by tool enumeration and prompt construction.
pub struct ToolDefinitionMetadata {
    /// Model-facing name, description and schema.
    pub definition: maestro_models::Tool<maestro_models::SchemaCode>,
    /// Display label.
    pub label: String,
    /// Optional prompt snippet.
    pub prompt_snippet: Option<String>,
    /// Optional prompt guidance.
    pub prompt_guidelines: Option<Vec<String>>,
    /// Optional shell framing.
    pub render_shell: Option<maestro_tools::ToolRenderShell>,
    /// Optional scheduling preference.
    pub execution_mode: Option<maestro_agent::ToolExecutionMode>,
}
/// Metadata observation and the two existing tools-owned projections.
pub trait RetainedDefinition {
    /// Current definition name, not the stored map key.
    fn name(&self) -> String;
    /// Current metadata, sampled from the retained definition on each query.
    fn metadata(&self) -> ToolDefinitionMetadata;
    /// Retained typed owner for the query route.
    fn as_any(&self) -> &dyn std::any::Any;
    /// Existing tools wrapper with a fresh context per call.
    fn wrap(&self, factory: ContextFactory) -> maestro_agent::AgentToolRecord<Local>;
    /// Existing typed row binding.
    /// # Errors
    /// Returns the lower binding diagnostic for a typed slot on the raw route.
    fn renderer_binding(&self) -> Result<ToolRendererBinding, maestro_models::DiagnosticErrorInfo>;
}
/// Own the typed handle until its last registration/wrapper alias is released.
pub struct Definition<P, D, S, E: ToolExecution> {
    /// The definition owner.
    pub definition: E::Shared<RwLock<super::Definition<P, D, S, E>>>,
    /// Existing typed or raw renderer projection chosen by the native input.
    pub project_renderers: super::RendererProjection<P, D, S, E>,
}
impl<P: DeserializeOwned + 'static, D: Serialize + 'static, S: Default + 'static, E: ToolExecution>
    RetainedDefinition for Definition<P, D, S, E>
{
    fn metadata(&self) -> ToolDefinitionMetadata {
        let definition = read(&self.definition);
        ToolDefinitionMetadata {
            definition: E::local_definition(&definition.definition),
            label: definition.label.clone(),
            prompt_snippet: definition.prompt_snippet.clone(),
            prompt_guidelines: definition.prompt_guidelines.clone(),
            render_shell: definition.render_shell,
            execution_mode: definition.execution_mode,
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn name(&self) -> String {
        read(&self.definition).definition.name.clone()
    }
    fn wrap(&self, factory: ContextFactory) -> maestro_agent::AgentToolRecord<Local> {
        maestro_tools::tool_definition_wrapper::local::wrap_tool_definition(
            ToolDefinitionInput::<ExtensionContext, P, D, S, E, Local>::Retained(
                self.definition.clone(),
            ),
            Some(factory),
        )
    }
    fn renderer_binding(&self) -> Result<ToolRendererBinding, maestro_models::DiagnosticErrorInfo> {
        (self.project_renderers)(&read(&self.definition))
    }
}
