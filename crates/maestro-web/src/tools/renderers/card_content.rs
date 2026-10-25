//! Owned card captions and mutually exclusive retained bodies.
use super::collapsible_card::{CollapsibleCard, Content};
use crate::components::native_text::{NativeText, TextKind};
use crate::tools::{
    renderer_registry::{ToolHeaderState, ToolHeaderText, render_header},
    types::{ToolContent, ToolRenderResult, ToolRenderView, ToolResultMessageView, ToolUiError},
};
use crate::{I18n, browser};
use maestro_app::{JsonObject, ToolRendererState, display_truthy};
use std::{borrow::Cow, rc::Rc};
/// Private localized or authored caption representation.
pub(in crate::tools) enum Caption {
    /// Caller-authored text.
    Literal(String),
    /// Localized artifact label followed by an optional captured filename.
    Artifact(
        &'static str,
        Option<crate::tools::artifacts::artifact_pill::Pill>,
    ),
    /// Translated key followed by a literal suffix.
    Localized(&'static str, String),
}
impl Caption {
    /// Translate only the fixed key.
    pub(super) fn resolve(&self, i18n: &I18n) -> Result<String, ToolUiError> {
        match self {
            Self::Literal(text) => Ok(text.clone()),
            Self::Artifact(key, _) => Ok(i18n.text(key)?),
            Self::Localized(key, suffix) => Ok(format!("{}{suffix}", i18n.text(key)?)),
        }
    }
}
/// One mutually exclusive below-header branch.
pub(in crate::tools) enum Body {
    /// Header only.
    Empty,
    /// Literal command and output delegated to the console owner.
    Console(String, crate::components::console_block::ConsoleVariant),
    /// Plain destructive result text.
    Error(String),
}
/// One owned drawing recipe shared by the three stateless renderer modules.
pub(in crate::tools) struct CardContent {
    /// Result status, independent of argument streaming.
    pub(in crate::tools) state: ToolHeaderState,
    /// Tool glyph catalog name.
    pub(in crate::tools) glyph: &'static str,
    /// Owned literal or localized caption.
    pub(in crate::tools) caption: Caption,
    /// Selected below-header branch.
    pub(in crate::tools) body: Body,
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
crate::tools::types::retained_tool_content!(CardContent, Retained);
impl CardContent {
    /// Reconcile native owners before drawing the current branch.
    fn draw(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        retained: &mut Retained,
        overlays: &mut Vec<crate::AttachmentOverlay>,
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
            header
                .render(ui, view, &retained.header)
                .collect(overlays)?;
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
    pub(super) fn result<C: CardOutput>(self) -> ToolRenderResult<C> {
        ToolRenderResult {
            content: C::card(self),
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
pub(in crate::tools) fn field<'a>(
    params: Option<&'a JsonObject>,
    key: &str,
) -> Result<Option<Cow<'a, str>>, ToolUiError> {
    display_truthy(params.and_then(|params| params.get(key)))
        .map_err(|error| ToolUiError::Presentation(error.to_string()))
}

/// Native inline artifact caption, sharing only its header's action admission.
struct ArtifactCaption {
    /// Already localized caption text.
    text: String,
    /// Optional owned filename/panel recipe.
    pill: Option<crate::tools::artifacts::artifact_pill::Pill>,
    /// Existing containing disclosure activation.
    actions: Rc<crate::tools::renderer_registry::header::InlineActions>,
}
/// Independent native roots within a rich caption.
#[derive(Default)]
struct CaptionState {
    /// Literal label lifetime.
    label: Option<NativeText>,
    /// Filename lifetime, absent when that child disappears.
    pill: maestro_app::ToolRendererState,
}
impl Caption {
    /// Select literal text or the existing rich-header child path.
    pub(super) fn header(
        &self,
        i18n: &I18n,
        actions: Rc<crate::tools::renderer_registry::header::InlineActions>,
    ) -> Result<ToolHeaderText, ToolUiError> {
        if let Self::Artifact(_, pill) = self {
            let mut pill = pill.clone();
            if let Some(pill) = &mut pill {
                pill.actions = Some(Rc::clone(&actions));
            }
            Ok(ToolHeaderText::Content(Box::new(ArtifactCaption {
                text: self.resolve(i18n)?,
                pill,
                actions,
            })))
        } else {
            Ok(ToolHeaderText::Text(self.resolve(i18n)?))
        }
    }
}
crate::tools::types::retained_tool_content!(ArtifactCaption, CaptionState);
impl ArtifactCaption {
    /// Draw the caption and its independently owned optional file action.
    fn draw(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        retained: &mut CaptionState,
        overlays: &mut Vec<crate::AttachmentOverlay>,
    ) -> Result<egui::Response, ToolUiError> {
        let row = ui.horizontal_wrapped(|ui| -> Result<(), ToolUiError> {
            let label = retained
                .label
                .get_or_insert_with(|| NativeText::new(Rc::clone(view.host)));
            label.render(ui, (&self.text, TextKind::InlineCaption), view)?;
            label.set_action(Some(
                crate::tools::renderer_registry::header::InlineActions::action(&self.actions),
            ));
            if let Some(pill) = &self.pill {
                pill.render(ui, view, &retained.pill).collect(overlays)?;
            } else {
                retained.pill = ToolRendererState::default();
            }
            Ok(())
        });
        row.inner.map(|()| row.response)
    }
}

/// Content construction shared by branch-selecting renderer implementations.
pub(in crate::tools) trait CardOutput: Sized {
    /// Retain the selected branch in this representation.
    fn card(card: CardContent) -> Self;
    /// Retain renderer `R`'s selected disclosure branch in this representation.
    fn collapsible<R: 'static>(card: CollapsibleCard<Self>) -> Self;
}
impl CardOutput for Box<dyn ToolContent> {
    fn card(card: CardContent) -> Self {
        Box::new(card)
    }
    fn collapsible<R: 'static>(card: CollapsibleCard<Self>) -> Self {
        Box::new(Content::<R, Self>::from(card))
    }
}
#[cfg(target_arch = "wasm32")]
pub(super) mod dom;
