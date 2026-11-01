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
/// One concrete native or decoded renderer handle for one content representation.
struct RendererSlot<P, D, C>(Option<Rc<dyn ToolRenderer<P, D, C>>>);
impl<P, D, C> Default for RendererSlot<P, D, C> {
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
        binding.with_raw(renderer)
    }
    /// Retain an additional native callback view in this binding.
    #[must_use]
    pub fn with_raw<P: 'static, D: 'static>(self, renderer: Rc<dyn ToolRenderer<P, D>>) -> Self {
        self.with_slot(renderer)
    }
    /// Retain a page-content renderer for these native operand types.
    #[must_use]
    pub fn from_dom<P: 'static, D: 'static>(
        renderer: Rc<dyn ToolRenderer<P, D, Box<dyn crate::dom::DomContent>>>,
    ) -> Self {
        let binding = Self {
            owner: ToolRendererState::default(),
        };
        binding.with_slot(renderer)
    }
    /// Replace the handle stored for one operand and content combination.
    fn with_slot<P: 'static, D: 'static, C: 'static>(
        self,
        renderer: Rc<dyn ToolRenderer<P, D, C>>,
    ) -> Self {
        let previous = self
            .owner
            .state::<RendererSlot<P, D, C>>()
            .borrow_mut()
            .0
            .replace(renderer);
        drop(previous);
        self
    }
    /// Additionally admit present JSON operands through their selected decoders.
    #[must_use]
    pub fn from_typed<P: DeserializeOwned + 'static, D: DeserializeOwned + 'static>(
        renderer: Rc<dyn ToolRenderer<P, D>>,
    ) -> Self {
        Self::typed(renderer)
    }
    /// Install typed and decoded views through the same content-specific slot owner.
    fn typed<P: DeserializeOwned + 'static, D: DeserializeOwned + 'static, C: 'static>(
        renderer: Rc<dyn ToolRenderer<P, D, C>>,
    ) -> Self {
        let binding = Self {
            owner: ToolRendererState::default(),
        }
        .with_slot(Rc::clone(&renderer));
        let binding = if TypeId::of::<(P, D)>() == TypeId::of::<(Value, Value)>() {
            binding
        } else {
            binding.with_slot::<Value, Value, C>(Rc::new(DecodedRenderer {
                renderer: Rc::clone(&renderer),
            }))
        };
        if TypeId::of::<(P, D)>() != TypeId::of::<(JsonObject, Value)>() {
            return binding
                .with_slot::<JsonObject, Value, C>(Rc::new(DecodedRenderer { renderer }));
        }
        binding
    }
    /// Retain an output-only callback without restricting callers' native details.
    pub(crate) fn from_details_agnostic(renderer: Rc<dyn ToolRenderer<JsonObject, ()>>) -> Self {
        Self::output_only(renderer)
    }
    /// Store an output-only callback in the selected content representation.
    fn output_only<C: 'static>(renderer: Rc<dyn ToolRenderer<JsonObject, (), C>>) -> Self {
        let binding = Self {
            owner: ToolRendererState::default(),
        }
        .with_slot(Rc::clone(&renderer));
        binding.owner.state::<DetailsAgnostic<C>>().borrow_mut().0 = Some(renderer);
        binding
    }
    /// Clone the renderer handle for these concrete operand types.
    #[must_use]
    pub fn renderer<P: 'static, D: 'static>(&self) -> Option<Rc<dyn ToolRenderer<P, D>>> {
        self.renderer_for::<P, D, crate::dom::PreparedContent>()
    }
    /// Resolve a concrete content slot, admitting arbitrary details only for output-only callbacks.
    fn renderer_for<P: 'static, D: 'static, C: 'static>(
        &self,
    ) -> Option<Rc<dyn ToolRenderer<P, D, C>>> {
        let slot = self.owner.state::<RendererSlot<P, D, C>>();
        if slot.borrow().0.is_none()
            && (TypeId::of::<P>() == TypeId::of::<JsonObject>()
                || TypeId::of::<P>() == TypeId::of::<Value>())
        {
            let callback = self.owner.state::<DetailsAgnostic<C>>().borrow().0.clone();
            if let Some(renderer) = callback {
                slot.borrow_mut().0 = Some(Rc::new(DetailsAdapter::<P, D, _> {
                    renderer,
                    marker: std::marker::PhantomData,
                }));
            }
        }
        slot.borrow().0.clone()
    }
}
/// JSON adapter retaining the same native renderer.
struct DecodedRenderer<P, D, C> {
    /// Original typed callback.
    renderer: Rc<dyn ToolRenderer<P, D, C>>,
}
impl<P: DeserializeOwned, D: DeserializeOwned, C> ToolRenderer<Value, Value, C>
    for DecodedRenderer<P, D, C>
{
    fn render(
        &self,
        params: Option<&Value>,
        result: Option<ToolResultMessageView<'_, Value>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
        let params = params
            .map(P::deserialize)
            .transpose()
            .map_err(|e| ToolUiError::Presentation(e.to_string()))?;
        self.finish(params.as_ref(), result.as_ref(), is_streaming)
    }
}
impl<P: DeserializeOwned, D: DeserializeOwned, C> ToolRenderer<JsonObject, Value, C>
    for DecodedRenderer<P, D, C>
{
    fn render(
        &self,
        params: Option<&JsonObject>,
        result: Option<ToolResultMessageView<'_, Value>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
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
impl<P, D: DeserializeOwned, C> DecodedRenderer<P, D, C> {
    /// Decode details and invoke the native callback for either JSON carrier.
    fn finish(
        &self,
        params: Option<&P>,
        result: Option<&ToolResultMessageView<'_, Value>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
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
struct DetailsAgnostic<C>(Option<Rc<dyn ToolRenderer<JsonObject, (), C>>>);
impl<C> Default for DetailsAgnostic<C> {
    fn default() -> Self {
        Self(None)
    }
}
/// One cached native-operand adapter; original message fields remain borrowed.
struct DetailsAdapter<P, D, C> {
    /// Callback which never reads native details.
    renderer: Rc<dyn ToolRenderer<JsonObject, (), C>>,
    /// Operand types retained only in the adapter's type identity.
    marker: std::marker::PhantomData<fn(&P, &D)>,
}
impl<P: 'static, D, C> ToolRenderer<P, D, C> for DetailsAdapter<P, D, C> {
    fn render(
        &self,
        params: Option<&P>,
        result: Option<ToolResultMessageView<'_, D>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
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
                Ok(super::json_params(Some(value), &empty))
            })
            .transpose()?
            .flatten();
        let projected = result
            .as_ref()
            .map(|result| result.with_details::<()>(None));
        self.renderer.render(params, projected, is_streaming)
    }
}
