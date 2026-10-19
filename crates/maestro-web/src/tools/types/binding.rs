//! Typed renderer handles using the existing row-state erasure owner.
use super::{ToolRenderResult, ToolRenderer, ToolResultMessageView};
use crate::tools::types::ToolUiError;
use maestro_app::{JsonObject, ToolRendererState};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{any::TypeId, rc::Rc};

/// Retained native renderer with optional ordinary-JSON admission.
#[derive(Clone)]
pub struct ToolRendererBinding {
    /// Existing typed handle owner; native and JSON views share it.
    owner: ToolRendererState,
}
/// One concrete native or decoded renderer handle.
struct RendererSlot<P, D>(Option<Rc<dyn ToolRenderer<P, D>>>);
impl<P, D> Default for RendererSlot<P, D> {
    fn default() -> Self {
        Self(None)
    }
}
impl ToolRendererBinding {
    /// Retain a renderer without requiring either JSON conversion trait.
    #[must_use]
    pub fn from_raw<P: 'static, D: 'static>(renderer: Rc<dyn ToolRenderer<P, D>>) -> Self {
        let binding = Self {
            owner: ToolRendererState::default(),
        };
        binding.owner.state::<RendererSlot<P, D>>().borrow_mut().0 = Some(renderer);
        binding
    }
    /// Additionally admit present JSON operands through their selected decoders.
    #[must_use]
    pub fn from_typed<P: DeserializeOwned + 'static, D: DeserializeOwned + 'static>(
        renderer: Rc<dyn ToolRenderer<P, D>>,
    ) -> Self {
        let binding = Self::from_raw(Rc::clone(&renderer));
        if TypeId::of::<(P, D)>() != TypeId::of::<(Value, Value)>() {
            binding
                .owner
                .state::<RendererSlot<Value, Value>>()
                .borrow_mut()
                .0 = Some(Rc::new(DecodedRenderer {
                renderer: Rc::clone(&renderer),
            }));
        }
        if TypeId::of::<(P, D)>() != TypeId::of::<(JsonObject, Value)>() {
            binding
                .owner
                .state::<RendererSlot<JsonObject, Value>>()
                .borrow_mut()
                .0 = Some(Rc::new(DecodedRenderer { renderer }));
        }
        binding
    }
    /// Retain an output-only callback without restricting callers' native details.
    pub(crate) fn from_details_agnostic(renderer: Rc<dyn ToolRenderer<JsonObject, ()>>) -> Self {
        let binding = Self::from_raw(Rc::clone(&renderer));
        binding.owner.state::<DetailsAgnostic>().borrow_mut().0 = Some(renderer);
        binding
    }
    /// Clone the renderer handle for these concrete operand types.
    #[must_use]
    pub fn renderer<P: 'static, D: 'static>(&self) -> Option<Rc<dyn ToolRenderer<P, D>>> {
        let slot = self.owner.state::<RendererSlot<P, D>>();
        if slot.borrow().0.is_none()
            && (TypeId::of::<P>() == TypeId::of::<JsonObject>()
                || TypeId::of::<P>() == TypeId::of::<Value>())
        {
            let callback = self.owner.state::<DetailsAgnostic>().borrow().0.clone();
            if let Some(renderer) = callback {
                slot.borrow_mut().0 = Some(Rc::new(DetailsAdapter::<P, D> {
                    renderer,
                    marker: std::marker::PhantomData,
                }));
            }
        }
        slot.borrow().0.clone()
    }
}
/// JSON adapter retaining the same native renderer.
struct DecodedRenderer<P, D> {
    /// Original typed callback.
    renderer: Rc<dyn ToolRenderer<P, D>>,
}
impl<P: DeserializeOwned, D: DeserializeOwned> ToolRenderer<Value, Value>
    for DecodedRenderer<P, D>
{
    fn render(
        &self,
        params: Option<&Value>,
        result: Option<ToolResultMessageView<'_, Value>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult, ToolUiError> {
        let params = params
            .map(P::deserialize)
            .transpose()
            .map_err(|e| ToolUiError::Presentation(e.to_string()))?;
        self.finish(params.as_ref(), result.as_ref(), is_streaming)
    }
}
impl<P: DeserializeOwned, D: DeserializeOwned> ToolRenderer<JsonObject, Value>
    for DecodedRenderer<P, D>
{
    fn render(
        &self,
        params: Option<&JsonObject>,
        result: Option<ToolResultMessageView<'_, Value>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult, ToolUiError> {
        let params = params
            .map(|object| {
                let entries = object.iter().map(|(key, value)| (key.as_str(), value));
                P::deserialize(
                    serde::de::value::MapDeserializer::<_, serde_json::Error>::new(entries),
                )
            })
            .transpose()
            .map_err(|error| ToolUiError::Presentation(error.to_string()))?;
        self.finish(params.as_ref(), result.as_ref(), is_streaming)
    }
}
impl<P, D: DeserializeOwned> DecodedRenderer<P, D> {
    /// Decode details and invoke the native callback for either JSON carrier.
    fn finish(
        &self,
        params: Option<&P>,
        result: Option<&ToolResultMessageView<'_, Value>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult, ToolUiError> {
        let details = result
            .and_then(ToolResultMessageView::details)
            .map(D::deserialize)
            .transpose()
            .map_err(|e| ToolUiError::Presentation(e.to_string()))?;
        let projected = result.map(|result| result.with_details(details.as_ref()));
        self.renderer.render(params, projected, is_streaming)
    }
}

/// The existing typed owner also retains output-only callbacks.
#[derive(Default)]
struct DetailsAgnostic(Option<Rc<dyn ToolRenderer<JsonObject, ()>>>);
/// One cached native-operand adapter; original message fields remain borrowed.
struct DetailsAdapter<P, D> {
    /// Callback which never reads native details.
    renderer: Rc<dyn ToolRenderer<JsonObject, ()>>,
    /// Operand types retained only in the adapter's type identity.
    marker: std::marker::PhantomData<fn(&P, &D)>,
}
impl<P: 'static, D> ToolRenderer<P, D> for DetailsAdapter<P, D> {
    fn render(
        &self,
        params: Option<&P>,
        result: Option<ToolResultMessageView<'_, D>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult, ToolUiError> {
        let empty = JsonObject::new();
        let params = params
            .map(|params| -> Result<Option<&JsonObject>, ToolUiError> {
                let any = params as &dyn std::any::Any;
                if let Some(object) = any.downcast_ref::<JsonObject>() {
                    return Ok(Some(object));
                }
                let value = any.downcast_ref::<Value>().ok_or_else(|| {
                    ToolUiError::Presentation("Renderer operand types do not match".into())
                })?;
                Ok(if let Some(object) = value.as_object() {
                    Some(object)
                } else if maestro_app::is_truthy(value) {
                    Some(&empty)
                } else {
                    None
                })
            })
            .transpose()?
            .flatten();
        let projected = result
            .as_ref()
            .map(|result| result.with_details::<()>(None));
        self.renderer.render(params, projected, is_streaming)
    }
}
