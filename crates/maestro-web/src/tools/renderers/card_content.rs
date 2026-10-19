//! Owned card captions and mutually exclusive retained bodies.
use crate::components::native_text::{NativeText, TextKind};
use crate::tools::{
    renderer_registry::{ToolHeaderState, ToolHeaderText, render_header},
    types::{ToolContent, ToolRenderResult, ToolRenderView, ToolResultMessageView, ToolUiError},
};
use crate::{I18n, browser};
use maestro_app::{JsonObject, ToolRendererState, display_truthy};
use std::{borrow::Cow, rc::Rc};
/// Private localized or authored caption representation.
pub(super) enum Caption {
    /// Caller-authored text.
    Literal(String),
    /// Translated key followed by a literal suffix.
    Localized(&'static str, String),
}
impl Caption {
    /// Translate only the fixed key.
    fn resolve(&self, i18n: &I18n) -> Result<String, ToolUiError> {
        match self {
            Self::Literal(text) => Ok(text.clone()),
            Self::Localized(key, suffix) => Ok(format!("{}{suffix}", i18n.text(key)?)),
        }
    }
}
/// One mutually exclusive below-header branch.
pub(super) enum Body {
    /// Header only.
    Empty,
    /// Literal command and output delegated to the console owner.
    Console(String, crate::components::console_block::ConsoleVariant),
    /// Plain destructive result text.
    Error(String),
}
/// One owned drawing recipe shared by the three stateless renderer modules.
pub(super) struct CardContent {
    /// Result status, independent of argument streaming.
    pub(super) state: ToolHeaderState,
    /// Tool glyph catalog name.
    pub(super) glyph: &'static str,
    /// Owned literal or localized caption.
    pub(super) caption: Caption,
    /// Selected below-header branch.
    pub(super) body: Body,
}
/// Per-row owners are separate from immutable recipes.
#[derive(Default)]
struct Retained {
    /// Continuously retained shared header state.
    header: ToolRendererState,
    /// Present only while the console branch is selected.
    console: Option<crate::ConsoleBlock>,
    /// Present only while plain error text is selected.
    error: Option<NativeText>,
}
impl ToolContent for CardContent {
    fn render(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        state: &ToolRendererState,
    ) -> Result<egui::Response, ToolUiError> {
        let slot = state.state::<Retained>();
        let mut retained = std::mem::take(&mut *slot.borrow_mut());
        let result = self.draw(ui, view, &mut retained);
        let old = slot.replace(retained);
        drop(old);
        result
    }
}
impl CardContent {
    /// Reconcile native owners before drawing the current branch.
    fn draw(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        retained: &mut Retained,
    ) -> Result<egui::Response, ToolUiError> {
        let glyph =
            browser::attachment_content::glyph_node(self.glyph).map_err(ToolUiError::Control)?;
        let header = render_header(
            self.state,
            glyph,
            ToolHeaderText::Text(self.caption.resolve(view.i18n)?),
        );
        if !matches!(self.body, Body::Console(..)) {
            retained.console = None;
        }
        if !matches!(self.body, Body::Error(..)) {
            retained.error = None;
        }
        let row = ui.vertical(|ui| -> Result<(), ToolUiError> {
            ui.spacing_mut().item_spacing.y = 12.;
            header.render(ui, view, &retained.header)?;
            match &self.body {
                Body::Empty => {}
                Body::Console(content, variant) => {
                    let console = retained
                        .console
                        .get_or_insert_with(|| crate::ConsoleBlock::new(Rc::clone(view.host)));
                    console.content.clone_from(content);
                    console.variant = *variant;
                    console
                        .render(ui, view.i18n, view.presentation, view.modal)
                        .map_err(ToolUiError::Reading)?;
                }
                Body::Error(text) => {
                    retained
                        .error
                        .get_or_insert_with(|| NativeText::new(Rc::clone(view.host)))
                        .render(ui, (text, TextKind::Error(0)), view)?;
                }
            }
            Ok(())
        });
        row.inner?;
        Ok(row.response)
    }
    /// Prepare an ordinary outer card with this owned recipe.
    pub(super) fn result(self) -> ToolRenderResult {
        ToolRenderResult {
            content: Box::new(self),
            is_custom: false,
        }
    }
}
/// Status deliberately does not use the default renderer's streaming rule.
pub(super) fn state<D>(result: Option<&ToolResultMessageView<'_, D>>) -> ToolHeaderState {
    match result {
        Some(result) if result.is_error() => ToolHeaderState::Error,
        Some(_) => ToolHeaderState::Complete,
        None => ToolHeaderState::Inprogress,
    }
}
/// One fallible projection shared by all selected-field reads.
pub(super) fn field<'a>(
    params: Option<&'a JsonObject>,
    key: &str,
) -> Result<Option<Cow<'a, str>>, ToolUiError> {
    display_truthy(params.and_then(|params| params.get(key)))
        .map_err(|error| ToolUiError::Presentation(error.to_string()))
}
