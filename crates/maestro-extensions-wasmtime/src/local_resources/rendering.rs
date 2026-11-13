//! Original native rows and components share this registry and its release leases.
use super::{Capability, Registry};
use crate::{
    import_route::Value as Outcome,
    native_renders::{ComponentEndpoint, Render, Row, RowEndpoint, Slot},
};
use maestro_extensions::{ComponentHandle, ToolRenderContext, ToolRenderResultLike};
use serde_json::Value;
use std::rc::Rc;

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
impl Registry {
    /// Calls a native renderer without borrowing the registry during author code.
    pub(crate) fn render_builtin(
        &self,
        definition: &Rc<dyn crate::builtin_tools::Definition>,
        render: Render,
    ) -> Result<Outcome, String> {
        let row = render
            .row
            .ok_or("native tool renderer requires a native row")?;
        let row = self.capability(row.0.id);
        let Some(Capability::NativeRow(row)) = row.as_deref() else {
            return Err("extension native row resource ended".into());
        };
        let theme = self.capability(render.theme.0.id);
        let Some(Capability::Theme(theme)) = theme.as_deref() else {
            return Err("extension theme resource ended".into());
        };
        let last = match render.last {
            Some(crate::native_renders::Previous::Native(last)) => Some(self.component(&last)?),
            Some(crate::native_renders::Previous::Authored(last)) => Some(Rc::new(
                crate::guest_component::GuestComponent::new(last, self.clone(), row.events.clone()),
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
        self.insert(Capability::NativeComponent(component))
            .map(|lease| Outcome::Component(ComponentEndpoint(lease)))
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
