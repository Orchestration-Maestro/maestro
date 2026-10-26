//! Original native rows and components share this registry and its release leases.
use super::{Capability, Registry, pump::State};
use crate::{
    import_route::Value as Outcome,
    native_renders::{ComponentEndpoint, ComponentRequest, Render, Row, RowEndpoint, Slot},
};
use maestro_extensions::{ComponentHandle, ToolRenderContext, ToolRenderResultLike};
use serde_json::Value;

impl Registry {
    /// Retains the actual native row environment and shared state without replacing effects.
    pub(crate) fn native_row(
        &self,
        context: &ToolRenderContext<'_, Value>,
        events: maestro_extensions::EventBus<maestro_extensions::Local>,
    ) -> Result<RowEndpoint, String> {
        self.insert(Capability::NativeRow(Row::new(context, events)))
            .map(RowEndpoint)
    }
    /// Resolves an original component outside the capability-table borrow.
    fn component(&self, component: &ComponentEndpoint) -> Result<ComponentHandle, String> {
        match self.capability(component.0.id).as_deref() {
            Some(Capability::NativeComponent(component)) => Ok(component.clone()),
            _ => Err("extension native component resource ended".into()),
        }
    }
}
impl State {
    /// Calls a native renderer without borrowing the registry during author code.
    pub(super) fn builtin_render(&self, id: u64, render: Render) -> Result<Outcome, String> {
        let definition = self.registry.capability(id);
        let Some(Capability::Builtin(definition)) = definition.as_deref() else {
            return Err("extension built-in tool resource ended".into());
        };
        let row = render
            .row
            .ok_or("native tool renderer requires a native row")?;
        let row = self.registry.capability(row.0.id);
        let Some(Capability::NativeRow(row)) = row.as_deref() else {
            return Err("extension native row resource ended".into());
        };
        let theme = self.registry.capability(render.theme.0.id);
        let Some(Capability::Theme(theme)) = theme.as_deref() else {
            return Err("extension theme resource ended".into());
        };
        let last = match render.last {
            Some(crate::native_renders::Previous::Native(last)) => {
                Some(self.registry.component(&last)?)
            }
            Some(crate::native_renders::Previous::Authored(last)) => Some(std::rc::Rc::new(
                crate::guest_component::GuestComponent::new(
                    last,
                    self.registry.clone(),
                    row.events.clone(),
                ),
            )
                as ComponentHandle),
            None => None,
        };
        let args = render
            .info
            .args
            .as_deref()
            .map(crate::writer_reads::decode)
            .transpose()?;
        let context = row.context(&render.info, args.as_ref(), last);
        let component = rendered(definition.renderers()?, render.slot, theme, &context)?;
        self.registry
            .insert(Capability::NativeComponent(component))
            .map(|lease| Outcome::Component(ComponentEndpoint(lease)))
    }
    /// Forwards every component method to the retained native consumer.
    pub(super) fn native_component(
        &self,
        id: u64,
        request: ComponentRequest,
    ) -> Result<Outcome, String> {
        let capability = self.registry.capability(id);
        let Some(Capability::NativeComponent(component)) = capability.as_deref() else {
            return Err("extension native component resource ended".into());
        };
        match request {
            ComponentRequest::Render(width) => component
                .try_render(width as usize)
                .map(Outcome::Lines)
                .map_err(|error| error.to_string()),
            ComponentRequest::Input(data) => {
                if let Some(input) = component.input_handler() {
                    input
                        .try_handle_input(&data)
                        .map_err(|error| error.to_string())?;
                }
                Ok(Outcome::Unit)
            }
            ComponentRequest::Invalidate => {
                component.invalidate();
                Ok(Outcome::Unit)
            }
            ComponentRequest::Key => Ok(Outcome::Key(component.wants_key_release())),
        }
    }
}

/// Invokes native callbacks with their ordinary operands, without reconstructing effects.
fn rendered(
    binding: maestro_extensions::ToolRendererBinding,
    slot: Slot,
    theme: &maestro_extensions::LiveTheme,
    context: &ToolRenderContext<'_, maestro_extensions::ToolRendererState>,
) -> Result<ComponentHandle, String> {
    match slot {
        Slot::Call(args) => {
            let args = args
                .as_deref()
                .map(crate::writer_reads::decode)
                .transpose()?;
            binding.render_call.ok_or("native call renderer absent")?(args.as_ref(), theme, context)
        }
        Slot::Result(result, options) => {
            let document: Value = crate::writer_reads::decode(&result)?;
            let value: maestro_extensions::AgentToolResult<Value> =
                serde::Deserialize::deserialize(&document)
                    .map_err(|error: serde_json::Error| error.to_string())?;
            let result = ToolRenderResultLike {
                content: &value.content,
                details: document.get("details"),
                terminate: value.terminate,
                is_error: document.get("isError").and_then(Value::as_bool),
            };
            binding
                .render_result
                .ok_or("native result renderer absent")?
                .prepare(result)
                .map_err(|error| error.to_string())?
                .render(options, theme, context)
        }
    }
    .map_err(|error| error.to_string())
}
