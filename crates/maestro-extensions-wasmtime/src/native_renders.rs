//! Opaque native rendering operands retained on the original request pump.
use crate::local_resources::effects::ThemeEndpoint;
use crate::local_resources::{Lease, Message};
use maestro_extensions::{ComponentHandle, ToolRenderContext, ToolRendererState};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Store-safe original row ownership.
#[derive(Clone)]
pub(crate) struct RowEndpoint(pub(crate) Arc<Lease>);
/// Store-safe original native component ownership.
#[derive(Clone)]
pub(crate) struct ComponentEndpoint(pub(crate) Arc<Lease>);

/// Original native effects and typed state, never reconstructed from guest values.
pub(crate) struct Row {
    /// Original rendering effects.
    environment: Rc<maestro_extensions::ToolRenderEnvironment>,
    /// Original redraw capability.
    invalidate: Rc<dyn Fn()>,
    /// Original typed state shared by both renderer slots.
    state: ToolRendererState,
    /// Original callback bus for a lent authored previous component.
    pub(crate) events: maestro_extensions::EventBus<maestro_extensions::Local>,
}
impl Row {
    /// Retains original effects while public observations travel independently.
    pub(crate) fn new(
        context: &ToolRenderContext<'_, Value>,
        events: maestro_extensions::EventBus<maestro_extensions::Local>,
    ) -> Self {
        Self {
            environment: context.environment.clone(),
            invalidate: context.invalidate.clone(),
            state: context.renderer_state().cloned().unwrap_or_default(),
            events,
        }
    }
    /// Borrows guest-supplied observations beside the unchanged native effects and state.
    pub(crate) fn context<'a>(
        &'a self,
        info: &crate::guest_calls::tool_renders::Info,
        args: Option<&'a Value>,
        last: Option<ComponentHandle>,
    ) -> ToolRenderContext<'a, ToolRendererState> {
        ToolRenderContext {
            args,
            tool_call_id: info.tool_call_id.clone(),
            invalidate: self.invalidate.clone(),
            environment: self.environment.clone(),
            last_component: last,
            state: Rc::new(RefCell::new(self.state.clone())),
            renderer_state: Some(&self.state),
            cwd: info.cwd.clone(),
            phase: maestro_extensions::ToolRenderPhase {
                execution_started: info.phase.execution_started,
                args_complete: info.phase.args_complete,
                is_partial: info.phase.is_partial,
            },
            expanded: info.expanded,
            show_images: info.show_images,
            is_error: info.is_error,
        }
    }
}
/// A delegated native callback's ordinary operands remain independent of its row context.
pub(crate) struct Render {
    /// Original native row.
    pub(crate) row: Option<RowEndpoint>,
    /// Caller-selected theme.
    pub(crate) theme: ThemeEndpoint,
    /// Exact previous native component when the wrapper returned it.
    pub(crate) last: Option<Previous>,
    /// Current public row observations as supplied by the guest.
    pub(crate) info: crate::guest_calls::tool_renders::Info,
    /// Call or result with its independent ordinary operands.
    pub(crate) slot: Slot,
}
/// Owned inputs of a delegated call or result callback.
pub(crate) enum Slot {
    /// Arguments can be absent independently of the row's arguments.
    Call(Option<String>),
    /// Complete result and independent result options.
    Result(String, maestro_extensions::ToolRenderResultOptions),
}
/// Operations on one original native component.
#[derive(Clone)]
pub(crate) enum ComponentRequest {
    /// Paint at the caller's viewport width.
    Render(u32),
    /// Literal input, without key normalization.
    Input(String),
    /// Invalidate retained component state.
    Invalidate,
    /// Read current key-release preference.
    Key,
}
impl crate::builtin_reads::Endpoint {
    /// Enters a native renderer through the original nested-call route.
    pub(crate) fn render(&self, render: Render) -> crate::import_route::Route {
        self.0.route(|id, reply| {
            Message::BuiltinOperation(crate::local_resources::builtin::Operation::Render(
                id, render, reply,
            ))
        })
    }
}
impl ComponentEndpoint {
    /// Uses the original component through the same route, permitting callback reentry.
    pub(crate) fn call(&self, request: ComponentRequest) -> crate::import_route::Route {
        self.0.route(|id, reply| {
            Message::BuiltinOperation(crate::local_resources::builtin::Operation::Component(
                id, request, reply,
            ))
        })
    }
}

/// Exact previous component domain, without a native per-slot cache.
pub(crate) enum Previous {
    /// An original native view passed directly.
    Native(ComponentEndpoint),
    /// An authored view retained on the existing callback table.
    Authored(crate::guest_calls::Handle),
}
