//! Retained native definitions forwarded through the component adapter.
use super::super::generated::{Generated, host, route};
use super::super::{callbacks::Identity, signals::Mirror, theme::Retained};
use crate::types::finite::encode_value;
use crate::{
    Component, ComponentView, ExtensionResult, NativeComponent, NativeRow, ReadToolOptions,
    ToolDefinition, ToolRenderContext,
};
use std::rc::Rc;

/// Constructs the native read with independently retained replacement observations.
pub(super) fn read(cwd: &str, options: ReadToolOptions) -> ExtensionResult<ToolDefinition> {
    definition(native_read(cwd, options)?)
}

/// Projects only executable metadata and a context-free invocation onto the same owner.
pub(super) fn executable_read(cwd: &str, options: ReadToolOptions) -> ExtensionResult<crate::Tool> {
    let tool = native_read(cwd, options)?;
    let metadata = metadata(&tool)?;
    Ok(crate::Tool {
        name: metadata.name,
        label: metadata.label,
        description: metadata.description,
        parameters: metadata.parameters,
        prepare_arguments: None,
        execution_mode: metadata.execution_mode,
        execute: Rc::new(move |id, args, signal, _update| {
            let tool = tool.clone();
            Box::pin(async move { invoke(&tool, id, args, signal, || Ok(None)).await })
        }),
    })
}

/// Constructs one native resource, retaining supplied observations only after admission.
fn native_read(cwd: &str, options: ReadToolOptions) -> ExtensionResult<Rc<host::BuiltinTool>> {
    let operations = options.operations.map(|operations| {
        let identity = Identity::new(&Generated);
        identity.stage(operations);
        identity
    });
    let tool = host::create_read_tool(
        cwd,
        options.auto_resize_images,
        operations.as_ref().map(|identity| &identity.handle),
    );
    if let Some(identity) = operations {
        identity.settle(tool.is_ok());
    }
    Ok(Rc::new(tool?))
}

/// Reads the original shared schema transport without converting it to JSON text.
fn metadata(tool: &host::BuiltinTool) -> ExtensionResult<crate::ToolMetadata> {
    let (mut metadata, parameters): (crate::ToolMetadata, _) =
        maestro_request::parameter_schema::transport::decode_metadata(&tool.metadata()?)?;
    metadata.parameters = parameters;
    Ok(metadata)
}

/// Projects read metadata and callbacks from its retained native definition.
fn definition(tool: Rc<host::BuiltinTool>) -> ExtensionResult<ToolDefinition> {
    let metadata = metadata(&tool)?;
    Ok(ToolDefinition {
        metadata,
        prepare_arguments: None,
        execute: execute(tool.clone()),
        render_call: Some(call(tool.clone())),
        render_result: Some(result(tool)),
    })
}

/// Executes on the native owner with call-time model and arbitrary caller cancellation.
fn execute(tool: Rc<host::BuiltinTool>) -> crate::ToolExecute {
    Rc::new(move |id, args, signal, _update, context| {
        let tool = tool.clone();
        Box::pin(async move { invoke(&tool, id, args, signal, || context.model()).await })
    })
}

/// Attaches cancellation before reading the supplied model and invoking the native owner.
async fn invoke(
    tool: &host::BuiltinTool,
    id: String,
    args: serde_json::Value,
    signal: Option<crate::AbortSignal>,
    model: impl FnOnce() -> ExtensionResult<Option<crate::Model>>,
) -> ExtensionResult<crate::AgentToolResult> {
    let mirror = Mirror::new(&Generated, signal.as_ref())?;
    let model = model()?.as_ref().map(encode_value).transpose()?;
    let result = tool
        .execute(id, encode_value(&args)?, model, mirror.signal())
        .await;
    let text = mirror.settle(result)?;
    super::super::session::decode(&text)
}

/// Forwards call operands independently of the row's public observations.
fn call(tool: Rc<host::BuiltinTool>) -> crate::ToolRenderCall {
    Rc::new(move |args, theme, context, _| {
        let operands = Operands::new(context, &theme)?;
        let args = args.as_ref().map(encode_value).transpose()?;
        component(&tool.render_call(
            args.as_deref(),
            &operands.info,
            &operands.theme.theme,
            Some(&operands.row.handle),
            operands.native.as_ref().map(|last| &*last.handle),
            operands.authored.as_ref(),
        ))
    })
}

/// Forwards complete rendering results without changing the execution-result record.
fn result(tool: Rc<host::BuiltinTool>) -> crate::ToolRenderResult {
    Rc::new(move |result, options, theme, context, _| {
        let operands = Operands::new(context, &theme)?;
        let options = host::ToolRenderResultOptions {
            expanded: options.expanded,
            is_partial: options.is_partial,
        };
        component(&tool.render_result(
            &encode_value(&result)?,
            options,
            &operands.info,
            &operands.theme.theme,
            Some(&operands.row.handle),
            operands.native.as_ref().map(|last| &*last.handle),
            operands.authored.as_ref(),
        ))
    })
}

/// Public row observations travel beside opaque original effects and component identities.
struct Operands {
    /// Original native row effects and shared state.
    row: NativeRow,
    /// Independently selected original host theme.
    theme: Rc<Retained<Generated>>,
    /// The guest's current public observations, including edits made by its wrapper.
    info: host::ToolRenderInfo,
    /// Original previous native component, when passed directly.
    native: Option<NativeComponent>,
    /// Original previous authored component, retained on the existing callback table.
    authored: Option<host::Callback>,
}
impl Operands {
    /// Retains only supplied capabilities; absent previous components remain absent.
    fn new(context: &ToolRenderContext, theme: &crate::Theme) -> ExtensionResult<Self> {
        let row = context
            .native_row
            .clone()
            .ok_or("native tool renderer requires a native row")?;
        let theme = theme
            .host::<Retained<Generated>>()
            .ok_or("native renderer requires a host theme")?;
        let info = host::ToolRenderInfo {
            tool_call_id: context.tool_call_id.clone(),
            args: context.args.as_ref().map(encode_value).transpose()?,
            cwd: context.cwd.clone(),
            phase: host::ToolRenderPhase {
                execution_started: context.phase.execution_started,
                args_complete: context.phase.args_complete,
                is_partial: context.phase.is_partial,
            },
            expanded: context.expanded,
            show_images: context.show_images,
            is_error: context.is_error,
        };
        let native = context
            .last_component
            .as_ref()
            .and_then(Component::native_component);
        let authored = context
            .last_component
            .as_ref()
            .filter(|_| native.is_none())
            .map(|component| Identity::new(&Generated).transfer(component.clone()));
        Ok(Self {
            row,
            theme,
            info,
            native,
            authored,
        })
    }
}

/// Retains the component returned after the route's nested callbacks finish.
fn component(ticket: &host::ImportRoute) -> ExtensionResult<Component> {
    match route::value(ticket)? {
        host::RouteValue::Component(component) => Ok(Component::new(View(NativeComponent {
            handle: Rc::new(component),
        }))),
        _ => Err("extension route result kind mismatch".into()),
    }
}

/// A native view never substitutes a guest proxy for its original identity.
struct View(NativeComponent);
impl ComponentView for View {
    fn native_component(&self) -> Option<NativeComponent> {
        Some(self.0.clone())
    }
    fn render(&self, width: u32, _: crate::CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        match route::value(&self.0.handle.render(width))? {
            host::RouteValue::Lines(lines) => Ok(lines),
            _ => Err("extension route result kind mismatch".into()),
        }
    }
    fn handle_input(&self, data: &str, _: crate::CallbackEmitter<'_>) -> ExtensionResult<()> {
        route::drive(&self.0.handle.handle_input(data))
    }
    fn invalidate(&self, _: crate::CallbackEmitter<'_>) -> ExtensionResult<()> {
        route::drive(&self.0.handle.invalidate())
    }
    fn wants_key_release(&self, _: crate::CallbackEmitter<'_>) -> ExtensionResult<bool> {
        match route::value(&self.0.handle.wants_key_release())? {
            host::RouteValue::Key(key) => Ok(key),
            _ => Err("extension route result kind mismatch".into()),
        }
    }
}
