//! Native tool renderers enter their authored guest renderer with the row's state.
use super::{Call, Handle};
use crate::lifecycle::replies::Reply;
use crate::local_resources::{
    Registry,
    effects::{EmitterEndpoint, InvalidatorEndpoint, ThemeEndpoint},
};
use maestro_extensions::{
    ComponentHandle, DiagnosticErrorInfo, EventBus, LiveTheme, Local, ToolRenderContext,
    ToolRenderResultLike,
};
use serde_json::Value;
use std::rc::Rc;
use wasmtime::component::{ComponentType, Lift, Lower, Resource};

/// Which renderer a tool rendering enters.
#[derive(ComponentType, Lower, Clone, Copy)]
#[component(enum)]
#[repr(u8)]
pub(crate) enum Slot {
    /// The call renderer.
    #[component(name = "call")]
    Call,
    /// The result renderer.
    #[component(name = "result")]
    Result,
}

/// What a tool renderer reads about its row at this rendering.
#[derive(ComponentType, Lower, Clone)]
#[component(record)]
pub(crate) struct Info {
    /// Stable id of the tool call.
    #[component(name = "tool-call-id")]
    tool_call_id: String,
    /// Arguments so far, as JSON.
    args: Option<String>,
    /// Working directory of the session.
    cwd: String,
    /// Execution progress of the row.
    phase: Phase,
    /// Whether the row is expanded.
    expanded: bool,
    /// Whether images are shown.
    #[component(name = "show-images")]
    show_images: bool,
    /// Whether the result is an error.
    #[component(name = "is-error")]
    is_error: bool,
}

/// Execution progress of a tool row on the canonical boundary.
#[derive(ComponentType, Lower, Clone)]
#[component(record)]
pub(crate) struct Phase {
    /// Whether execution has started.
    #[component(name = "execution-started")]
    execution_started: bool,
    /// Whether the arguments are complete.
    #[component(name = "args-complete")]
    args_complete: bool,
    /// Whether the result is still streaming.
    #[component(name = "is-partial")]
    is_partial: bool,
}

/// One tool rendering on the canonical boundary.
#[derive(ComponentType, Lower, Clone)]
#[component(record)]
pub(crate) struct Render {
    /// The renderer slot.
    pub(crate) slot: Slot,
    /// The guest identity of the row's state, once it has one.
    pub(crate) row: Option<u32>,
    /// What the renderer reads.
    pub(crate) info: Info,
    /// The result document of a result rendering.
    pub(crate) result: Option<String>,
}

/// A rendering's component and the row's new guest state identity.
#[derive(ComponentType, Lift)]
#[component(record)]
pub(crate) struct Rendered {
    /// The returned component.
    pub(crate) component: Resource<super::Callback>,
    /// The row's state when the row had none.
    pub(crate) row: Option<Resource<super::Callback>>,
}

/// One tool rendering with the theme and redraw capability its row lends.
pub(crate) struct ToolRenderCall<H = Handle> {
    /// Retains the authored renderer and its admission.
    pub(crate) handler: H,
    /// The rendering operands.
    pub(crate) render: Render,
    /// Original live theme.
    pub(crate) theme: ThemeEndpoint,
    /// The row's redraw capability.
    pub(crate) invalidator: InvalidatorEndpoint,
    /// Original native callback frame.
    pub(crate) emitter: EmitterEndpoint,
    /// The component and the row's new state identity.
    pub(crate) reply: Reply<(Handle, Option<Handle>)>,
}

/// The guest state of one native row, released with the row.
#[derive(Default)]
struct GuestRow(Option<Handle>);

impl Handle {
    /// A native call renderer bound to the original extension's callback frame.
    pub(crate) fn render_call(
        self,
        registry: Registry,
        events: EventBus<Local>,
    ) -> maestro_extensions::ToolRenderCall<Value, Local> {
        Rc::new(move |_args, theme, context| {
            self.rendered(&registry, &events, (Slot::Call, None, theme), context)
        })
    }

    /// A native result renderer bound to the original extension's callback frame.
    pub(crate) fn render_result(
        self,
        registry: Registry,
        events: EventBus<Local>,
    ) -> maestro_extensions::ToolRenderResult<Value, Value, Local> {
        let render = move |result: &ToolRenderResultLike<'_, Value>,
                           _options: maestro_extensions::ToolRenderResultOptions,
                           theme: &LiveTheme,
                           context: &ToolRenderContext<'_, Value>| {
            let result = serde_json::json!({ "content": result.content, "details": result.details,
                "terminate": result.terminate, "isError": result.is_error });
            let result = Some(result.to_string());
            self.rendered(&registry, &events, (Slot::Result, result, theme), context)
        };
        let render: maestro_extensions::ToolRenderResultCallback<Value, Value, Local> =
            Rc::new(render);
        maestro_extensions::ToolRenderResult::Raw(render)
    }

    /// Runs the authored renderer for one row and keeps the row's new guest state with it.
    fn rendered(
        &self,
        registry: &Registry,
        events: &EventBus<Local>,
        (slot, result, theme): (Slot, Option<String>, &LiveTheme),
        context: &ToolRenderContext<'_, Value>,
    ) -> Result<ComponentHandle, DiagnosticErrorInfo> {
        let state = context
            .renderer_state()
            .map(maestro_extensions::ToolRendererState::state::<GuestRow>);
        let row = state
            .as_ref()
            .and_then(|state| state.borrow().0.as_ref().map(Handle::id));
        let render = Render {
            slot,
            row,
            info: info(context),
            result,
        };
        let built = events.invoke_callback(|emitter| {
            self.0.endpoint.synchronous(registry, |reply| {
                Ok(Call::ToolRender(ToolRenderCall {
                    handler: self.clone(),
                    render,
                    theme: registry.theme(theme.clone())?,
                    invalidator: registry.invalidator(Rc::clone(&context.invalidate))?,
                    emitter: registry.emitter(emitter.frame())?,
                    reply,
                }))
            })
        });
        let (component, row) = built.map_err(DiagnosticErrorInfo::error)?;
        if let (Some(state), Some(row)) = (state, row) {
            state.borrow_mut().0 = Some(row);
        }
        let view = crate::guest_component::GuestComponent::new(
            component,
            registry.clone(),
            events.clone(),
        );
        Ok(Rc::new(view))
    }
}

/// What a renderer reads about its row, from the native row context.
fn info(context: &ToolRenderContext<'_, Value>) -> Info {
    Info {
        tool_call_id: context.tool_call_id.clone(),
        args: context.args.map(Value::to_string),
        cwd: context.cwd.clone(),
        phase: Phase {
            execution_started: context.phase.execution_started,
            args_complete: context.phase.args_complete,
            is_partial: context.phase.is_partial,
        },
        expanded: context.expanded,
        show_images: context.show_images,
        is_error: context.is_error,
    }
}
