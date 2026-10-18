//! Retained independent input and output listings for the default recipe.
use super::{Listing, prepare};
use crate::browser::attachment_content;
use crate::tools::types::ToolUiError;
use crate::{
    components::code_block::{CodeBlock, CodeBlockView},
    tools::{
        renderer_registry::{
            ToolHeaderState, ToolHeaderText,
            header::browser::{NativeText, TextKind},
            render_header,
        },
        types::{ToolContent, ToolRenderView},
    },
};
use maestro_app::ToolRendererState;
use std::rc::Rc;

/// Call-owned immutable default content.
pub(super) struct DefaultContent {
    /// Status selected before formatting input.
    pub(super) state: ToolHeaderState,
    /// Optional already-formatted input.
    pub(super) params: Option<String>,
    /// Raw joined output; absence means no result yet.
    pub(super) output: Option<String>,
}
/// Each visible subsection owns its native roots separately.
struct Section {
    /// Localized subsection label.
    label: NativeText,
    /// Literal listing and its retained copy feedback.
    listing: CodeBlock,
}
impl Section {
    /// Create independent native owners for this subsection.
    fn new(view: ToolRenderView<'_>) -> Self {
        Self {
            label: NativeText::new(Rc::clone(view.host)),
            listing: CodeBlock::new_literal(Rc::clone(view.host)),
        }
    }
    /// Publish one subsection through the existing listing pipeline.
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        label: &str,
        listing: &Listing,
    ) -> Result<(), ToolUiError> {
        ui.scope_builder(egui::UiBuilder::new().id(self.label.id()), |ui| {
            ui.spacing_mut().item_spacing.y = 4.;
            self.label
                .render(ui, (&view.i18n.text(label)?, TextKind::Label), view)?;
            self.listing.code.clone_from(&listing.code);
            listing.language.clone_into(&mut self.listing.language);
            self.listing
                .render(
                    ui,
                    CodeBlockView {
                        i18n: view.i18n,
                        presentation: view.presentation,
                        highlighter: view.highlighter,
                        modal: view.modal,
                    },
                )
                .map_err(ToolUiError::reading)?;
            Ok(())
        })
        .inner
    }
}
/// Independent child state owners within one default tool row.
#[derive(Default)]
struct Retained {
    /// Header's animation and native caption lifetime.
    header: ToolRendererState,
    /// Present only while input is rendered.
    input: Option<Section>,
    /// Present only while a result is rendered.
    output: Option<Section>,
}
impl ToolContent for DefaultContent {
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
impl DefaultContent {
    /// Draw selected branches, releasing only subsections that disappear.
    fn draw(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        retained: &mut Retained,
    ) -> Result<egui::Response, ToolUiError> {
        let no_output = if self.output.as_deref() == Some("") {
            view.i18n.text("(no output)")?
        } else {
            String::new()
        };
        let prepared = prepare(
            self.params.as_deref(),
            self.output.as_deref(),
            self.state,
            &no_output,
        )?;
        if prepared.input.is_none() {
            retained.input = None;
        }
        if prepared.output.is_none() {
            retained.output = None;
        }
        let catalog = crate::icons::icons().map_err(|error| {
            ToolUiError::Control(attachment_content::ControlError::Host(error.to_string()))
        })?;
        let icon = catalog.get("Code").ok_or_else(|| {
            ToolUiError::Control(attachment_content::ControlError::Host(
                "Missing code glyph".into(),
            ))
        })?;
        let header = render_header(
            self.state,
            icon.clone(),
            ToolHeaderText::Text(prepared.caption.into()),
        );
        let body = ui.vertical(|ui| -> Result<(), ToolUiError> {
            ui.spacing_mut().item_spacing.y = 12.;
            header.render(ui, view, &retained.header)?;
            if let Some(input) = &prepared.input {
                retained
                    .input
                    .get_or_insert_with(|| Section::new(view))
                    .render(ui, view, "Input", input)?;
            }
            if let Some(output) = &prepared.output {
                retained
                    .output
                    .get_or_insert_with(|| Section::new(view))
                    .render(ui, view, "Output", output)?;
            }
            Ok(())
        });
        body.inner?;
        Ok(body.response)
    }
}
