//! Owned browser tool content and call-scoped native operands.
mod ui_error;
use crate::{
    I18n,
    browser::attachment_content::{AttachmentHost, PlacementScope},
};
use maestro_app::{JsonObject, ToolRendererState};
use serde_json::Value;
use std::{
    any::{Any, TypeId},
    rc::Rc,
};
pub use ui_error::ToolUiError;
mod binding;
mod result_view;
pub use binding::ToolRendererBinding;
pub use result_view::ToolResultMessageView;

/// One frame's drawing outcome plus independently owned opened overlays.
#[must_use]
pub struct ToolContentOutput {
    /// Drawing result, independent of already-opened overlays.
    pub response: Result<egui::Response, ToolUiError>,
    /// Opened owners the caller retains even when the response failed.
    pub overlays: Vec<crate::AttachmentOverlay>,
}
impl ToolContentOutput {
    /// Transfer all opened owners before observing drawing failure.
    pub(crate) fn collect(
        self,
        overlays: &mut Vec<crate::AttachmentOverlay>,
    ) -> Result<egui::Response, ToolUiError> {
        overlays.extend(self.overlays);
        self.response
    }
}
/// Owned drawing recipe; mutable widgets belong to the caller's retained row.
pub trait ToolContent: Any {
    /// Draw this recipe using the row's typed state.
    /// # Errors
    /// Returns reached presentation or platform failures.
    fn render(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        state: &ToolRendererState,
    ) -> ToolContentOutput;
}
/// Borrowed presentation inputs for one frame.
#[derive(Clone, Copy)]
pub struct ToolRenderView<'a> {
    /// Platform effects.
    pub host: &'a Rc<dyn AttachmentHost>,
    /// Current localization.
    pub i18n: &'a I18n,
    /// Selected brand projection.
    pub presentation: &'a maestro_theme::BrandPresentation<'a>,
    /// Caller-selected syntax engine.
    pub highlighter: &'a dyn maestro_theme::SyntaxHighlighter,
    /// Containing modal.
    pub modal: Option<egui::Id>,
}
/// Rendered recipe and outer-card choice.
pub struct ToolRenderResult {
    /// Owned content, independent of call-scoped operands.
    pub content: Box<dyn ToolContent>,
    /// Whether the caller should omit its ordinary tool card.
    pub is_custom: bool,
}
/// Active recipe type and widget lifetime for one row.
#[derive(Default)]
pub struct ToolRenderState {
    /// Concrete recipe currently occupying the row.
    active: Option<TypeId>,
    /// Existing typed widget owner.
    state: ToolRendererState,
    /// Non-aliased placement lifetime, distinct from retained widget ownership.
    scope: Option<(egui::Id, PlacementScope)>,
}
impl ToolRenderResult {
    /// Draw the content, releasing prior widget state when its recipe type changes.
    /// # Errors
    /// Returns the content's rendering failure.
    pub fn render(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        retained: &mut ToolRenderState,
    ) -> ToolContentOutput {
        retained.draw(self.content.as_ref(), ui, view)
    }
}
impl ToolRenderState {
    /// Render one borrowed recipe through the same row-lifetime owner.
    pub(crate) fn draw(
        &mut self,
        content: &dyn ToolContent,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
    ) -> ToolContentOutput {
        let active = content.type_id();
        if self.active != Some(active) {
            self.scope = None;
            self.state = ToolRendererState::default();
            self.active = Some(active);
        }
        let (id, _) = self.scope.get_or_insert_with(|| placement_scope(view.host));
        let _active = view.host.enter_placement_scope(*id);
        content.render(ui, view, &self.state)
    }
}
/// Custom renderer receiving original native operands or explicitly decoded JSON.
pub trait ToolRenderer<P = JsonObject, D = Value> {
    /// Prepare owned content without retaining borrowed operands.
    /// # Errors
    /// Returns the renderer's preparation failure.
    fn render(
        &self,
        params: Option<&P>,
        result: Option<ToolResultMessageView<'_, D>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult, ToolUiError>;
}
/// Borrowed parameters with a deferred default-rendering projection.
pub struct ToolParams<'a, P = Value> {
    /// Original caller operand.
    pub(crate) value: &'a P,
    /// Invoked only after default rendering is selected.
    pub(crate) format: fn(&P) -> Result<Option<String>, ToolUiError>,
}
impl<'a> ToolParams<'a, JsonObject> {
    /// Borrow model arguments with deferred canonical object formatting.
    #[must_use]
    pub fn object(value: &'a JsonObject) -> Self {
        Self::typed(value, |object| {
            maestro_app::pretty_object(object)
                .map(Some)
                .map_err(|error| ToolUiError::Presentation(error.to_string()))
        })
    }
}
impl<'a> ToolParams<'a, Value> {
    /// Borrow ordinary JSON using the default formatter.
    #[must_use]
    pub fn json(value: &'a Value) -> Self {
        Self::typed(value, super::renderers::default_renderer::format_params)
    }
}
impl<'a, P> ToolParams<'a, P> {
    /// Borrow a native value with its optional literal input projection.
    #[must_use]
    pub fn typed(value: &'a P, format: fn(&P) -> Result<Option<String>, ToolUiError>) -> Self {
        Self { value, format }
    }
}

/// Separate a row's placement revocation from its cloneable widget ownership.
pub(crate) fn placement_scope(host: &Rc<dyn AttachmentHost>) -> (egui::Id, PlacementScope) {
    let id = crate::browser::attachment_content::identity();
    let host = Rc::downgrade(host);
    let lease = PlacementScope::new(move || {
        if let Some(host) = host.upgrade() {
            host.hide_placement_scope(id);
        }
    });
    (id, lease)
}
