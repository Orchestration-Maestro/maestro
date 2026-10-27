//! Retained native definitions; execution stays in the tools-owned wrapper.
use maestro_extensions::{
    AgentToolResult, Cancellation, Local, Model, ReadContext, ToolDefinitionInput,
    local::{ToolDefinition, wrap_tool_definition},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Only the model observation that a native built-in read consumes.
pub(crate) struct Context(pub(crate) Option<Arc<Model>>);
impl ReadContext for Context {
    fn model(&self) -> Option<Arc<Model>> {
        self.0.clone()
    }
}

/// Transport operations on the original typed definition, not another definition record.
pub(crate) trait Definition {
    /// Serializes metadata through the shared schema transport.
    fn metadata(&self) -> Result<String, String>;
    /// Invokes native argument preparation before any execution.
    fn prepare(&self, args: &mut Value) -> Result<Value, String>;
    /// Projects the original callbacks without retaining a second executable record.
    fn renderers(&self) -> Result<maestro_extensions::ToolRendererBinding, String>;
    /// Executes through the existing typed wrapper, retaining this definition for the call.
    fn execute(self: Rc<Self>, invocation: Invocation, model: Option<Arc<Model>>) -> Work;
}

/// Owned invocation operands, including its independent cancellation.
pub(crate) struct Invocation {
    /// Literal tool call identity.
    pub(crate) id: String,
    /// Original JSON arguments before typed execution.
    pub(crate) args: Value,
    /// Original native cancellation, when supplied.
    pub(crate) signal: Option<Cancellation>,
    /// Original independently optional progress observer.
    pub(crate) update: Option<maestro_extensions::AgentToolUpdateCallback<Value, Local>>,
}
/// Native local work with the native result and diagnostic preserved.
type Work = std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<AgentToolResult<Value>, maestro_extensions::DiagnosticErrorInfo>,
            >,
    >,
>;

impl<P, D, S> Definition for RwLock<ToolDefinition<Context, P, D, S>>
where
    P: DeserializeOwned + 'static,
    D: Serialize + 'static,
    S: Default + 'static,
{
    fn renderers(&self) -> Result<maestro_extensions::ToolRendererBinding, String> {
        let definition = self
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        maestro_extensions::ToolRendererBinding::from_raw(&*definition)
            .map_err(|error| error.to_string())
    }
    fn metadata(&self) -> Result<String, String> {
        let definition = self
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let metadata = Metadata {
            model: &definition.definition,
            label: &definition.label,
            prompt_snippet: &definition.prompt_snippet,
            prompt_guidelines: &definition.prompt_guidelines,
            render_shell: definition.render_shell.map(|shell| match shell {
                maestro_extensions::ToolRenderShell::Default => "default",
                maestro_extensions::ToolRenderShell::SelfRendered => "self",
            }),
            execution_mode: definition.execution_mode,
        };
        maestro_extensions::schema_transport::encode_metadata(
            &metadata,
            &definition.definition.parameters,
        )
    }
    fn prepare(&self, args: &mut Value) -> Result<Value, String> {
        let prepare = self
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .prepare_arguments
            .clone();
        match prepare {
            Some(prepare) => prepare(args).map_err(|error| error.to_string()),
            None => Ok(args.clone()),
        }
    }
    fn execute(self: Rc<Self>, invocation: Invocation, model: Option<Arc<Model>>) -> Work {
        let factory = model.map(|model| {
            Rc::new(move || Ok(Context(Some(model.clone()))))
                as maestro_extensions::local::ToolContextFactory<Context>
        });
        let tool = wrap_tool_definition(
            ToolDefinitionInput::<Context, P, D, S, Local>::Retained(self),
            factory,
        );
        (tool.execute)(
            invocation.id,
            invocation.args,
            invocation.signal,
            invocation.update,
        )
    }
}

/// Borrowed metadata projection; optional native fields remain omitted.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Metadata<'a> {
    /// Native model-facing name, description and schema.
    #[serde(flatten)]
    model: &'a maestro_extensions::Tool<maestro_extensions::SchemaCode>,
    /// Native display label.
    label: &'a str,
    /// Native prompt snippet.
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_snippet: &'a Option<String>,
    /// Native prompt guidance.
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_guidelines: &'a Option<Vec<String>>,
    /// Native framing policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    render_shell: Option<&'static str>,
    /// Native scheduling preference.
    #[serde(skip_serializing_if = "Option::is_none")]
    execution_mode: Option<maestro_extensions::ToolExecutionMode>,
}
