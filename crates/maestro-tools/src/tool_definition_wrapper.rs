//! Bridge between tool definitions and executable agent tools.

use crate::types::{ToolContextFactory, ToolDefinition, ToolDefinitionRecord, ToolExecution};
use maestro_agent::ExecuteTool;
use maestro_models::DiagnosticErrorInfo;
use maestro_models::SharedHandle;
use maestro_models::{Local, Threaded};
use serde::{Serialize, de::DeserializeOwned};
use std::rc::Rc;
/// Default executable agent tool.
type AgentTool = maestro_agent::AgentTool;

/// The original typed definition owner retained across wrapper invocations.
pub type RetainedToolDefinition<C, P, D, S, E = Threaded, I = E> =
    <E as maestro_models::Execution>::Shared<
        std::sync::RwLock<ToolDefinitionRecord<C, P, D, S, E, I>>,
    >;

/// An owned SDK definition or the original mutable retained registration.
pub enum ToolDefinitionInput<
    C,
    P,
    D,
    S,
    E: ToolExecution = Threaded,
    I: maestro_agent::AgentExecution = E,
> {
    /// A standalone owned definition.
    Owned(Box<ToolDefinitionRecord<C, P, D, S, E, I>>),
    /// The original shared mutable definition.
    Retained(RetainedToolDefinition<C, P, D, S, E, I>),
}
impl<C, P, D, S, E: ToolExecution, I: maestro_agent::AgentExecution>
    From<ToolDefinitionRecord<C, P, D, S, E, I>> for ToolDefinitionInput<C, P, D, S, E, I>
{
    fn from(definition: ToolDefinitionRecord<C, P, D, S, E, I>) -> Self {
        Self::Owned(Box::new(definition))
    }
}
impl<C, P, D, S, E: ToolExecution, I: maestro_agent::AgentExecution>
    ToolDefinitionInput<C, P, D, S, E, I>
{
    /// Project only an internal field read; no supplied callback executes under a guard.
    fn read<T>(&self, select: impl FnOnce(&ToolDefinitionRecord<C, P, D, S, E, I>) -> T) -> T {
        match self {
            Self::Owned(definition) => select(definition),
            Self::Retained(definition) => select(
                &definition
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            ),
        }
    }
}

/// One wrapper implementation for the two concrete callback families.
macro_rules! wrappers {
    ($mode:ty, $ptr:ident, [$($extra:tt)*], $storage:ty, $prepare:expr, $schema:expr) => {
        /// Wrap a definition as an agent tool.
        ///
        /// Once the arguments decode (see `AgentTool::typed` for rejected arguments), each
        /// execution asks the factory for a fresh context; a factory failure is the
        /// execution's error and the definition body does not run.
        #[must_use]
        pub fn wrap_tool_definition<C, P, D, S, $($extra)*>(
            definition: impl Into<ToolDefinitionInput<C, P, D, S, $storage, $mode>>,
            context_factory: Option<ToolContextFactory<C>>,
        ) -> AgentTool
        where
            C: 'static,
            S: 'static,
            P: DeserializeOwned + 'static,
            D: Serialize + 'static,
        {
            let definition = definition.into();
            let (model, label, preparation, mode) = definition.read(|definition| (
                ($schema)(&definition.definition), definition.label.clone(),
                definition.prepare_arguments.clone(), definition.execution_mode,
            ));
            let run: ExecuteTool<P, D, $mode> = $ptr::new(move |id, params, signal, update| {
                let execute = definition.read(|definition| definition.execute.clone());
                let context = match context_factory.as_ref().map(|factory| factory()) {
                    Some(Err(error)) => return Box::pin(async move { Err(error) }),
                    Some(Ok(context)) => Some(context),
                    None => None,
                };
                execute(id, params, signal, update, context)
            });
            let mut tool = AgentTool::typed(model, label, run);
            tool.prepare_arguments = preparation.map($prepare);
            tool.execution_mode = mode;
            tool
        }

        /// Wrap each definition in order, sharing one context factory.
        #[must_use]
        pub fn wrap_tool_definitions<C, P, D, S, $($extra)*>(
            definitions: Vec<impl Into<ToolDefinitionInput<C, P, D, S, $storage, $mode>>>,
            context_factory: Option<ToolContextFactory<C>>,
        ) -> Vec<AgentTool>
        where
            C: 'static,
            S: 'static,
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
                execute: $ptr::new(move |id, params, signal, update, _| {
                    execute(id, params, signal, update)
                }),
                render_call: None,
                render_result: None,
            };
            Ok(definition)
        }
    };
}
wrappers!(
    Threaded,
    SharedHandle,
    [],
    Threaded,
    std::convert::identity,
    Clone::clone
);
/// Caller-local bridges over the existing typed agent boundary.
pub mod local {
    use super::{
        DeserializeOwned, DiagnosticErrorInfo, ExecuteTool, Local, Rc, Serialize,
        ToolDefinitionInput, ToolExecution,
    };
    use crate::types::local::{ToolContextFactory, ToolDefinition};
    /// Caller-local executable agent tool.
    type AgentTool = maestro_agent::AgentToolRecord<Local>;
    wrappers!(Local, Rc, [H: ToolExecution], H, H::local_prepare, H::local_definition);
}
