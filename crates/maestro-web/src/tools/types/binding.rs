//! Typed renderer handles using the existing row-state erasure owner.
use super::{ToolRenderResult, ToolRenderer, ToolResultMessageView};
use crate::browser::attachment_content::AttachmentUiError;
use maestro_app::ToolRendererState;
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
                .0 = Some(Rc::new(DecodedRenderer { renderer }));
        }
        binding
    }
    /// Clone the renderer handle for these concrete operand types.
    #[must_use]
    pub fn renderer<P: 'static, D: 'static>(&self) -> Option<Rc<dyn ToolRenderer<P, D>>> {
        let slot = self.owner.state::<RendererSlot<P, D>>();
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
    ) -> Result<ToolRenderResult, AttachmentUiError> {
        let params = params
            .map(P::deserialize)
            .transpose()
            .map_err(|e| AttachmentUiError::Presentation(e.to_string()))?;
        let details = result
            .as_ref()
            .and_then(ToolResultMessageView::details)
            .map(D::deserialize)
            .transpose()
            .map_err(|e| AttachmentUiError::Presentation(e.to_string()))?;
        let projected = result
            .as_ref()
            .map(|result| result.with_details(details.as_ref()));
        self.renderer
            .render(params.as_ref(), projected, is_streaming)
    }
}
