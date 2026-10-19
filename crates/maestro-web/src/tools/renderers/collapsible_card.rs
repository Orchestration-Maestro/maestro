//! Retained document-card disclosures and independently owned file tiles.
use super::card_content::Caption;
use crate::{
    Attachment, AttachmentTile,
    browser::{
        attachment_content,
        transition::{Transition, TransitionStyle},
    },
    components::{
        code_block::{CodeBlock, CodeBlockView},
        console_block::ConsoleVariant,
        native_text::{NativeText, TextKind},
    },
    tools::{
        renderer_registry::{
            ToolHeaderState, ToolHeaderText, render_collapsible_header, render_header,
        },
        types::{ToolContent, ToolContentOutput, ToolRenderResult, ToolRenderView, ToolUiError},
    },
};
use maestro_app::ToolRendererState;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Selected subtree identity: result arrival replaces the pending body.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::tools) enum Phase {
    /// Streamed parameters without a result.
    Pending,
    /// Completed execution, successful or failed.
    Result,
}
/// Owned body operands, independent of the preparation call.
pub(in crate::tools) struct Body {
    /// Pending or completed subtree.
    pub(in crate::tools) phase: Phase,
    /// Literal URL value, including a present empty pending URL.
    pub(in crate::tools) url: Option<String>,
    /// Literal listing with its explicit language.
    pub(in crate::tools) code: Option<(String, &'static str)>,
    /// Optional nonempty console output.
    pub(in crate::tools) console: Option<(String, ConsoleVariant)>,
}
/// One immutable card recipe shared by document tools.
pub(in crate::tools) struct CollapsibleCard {
    /// Result status takes precedence over streaming.
    pub(in crate::tools) state: ToolHeaderState,
    /// Catalog glyph.
    pub(in crate::tools) glyph: &'static str,
    /// Literal or localized caption.
    pub(in crate::tools) caption: Caption,
    /// Absent until parameters arrive.
    pub(in crate::tools) body: Option<Body>,
    /// Ordered downloadable file records.
    pub(in crate::tools) attachments: Vec<Download>,
}
/// Renderer-specific erased identity for one shared drawing implementation.
struct Content<R> {
    /// Owned selected card operands.
    card: CollapsibleCard,
    /// Nominal renderer identity used by retained row erasure.
    renderer: std::marker::PhantomData<fn() -> R>,
}
/// Mounted children retain their own native roots across disclosure changes.
struct BodyState {
    /// Branch for which these children were mounted.
    phase: Phase,
    /// Independent disclosure value.
    expanded: Rc<Cell<bool>>,
    /// Existing numeric property owner for height and top margin.
    transition: Transition<2>,
    /// Nested native placement lifetime.
    scope: (egui::Id, attachment_content::PlacementScope),
    /// Literal URL text.
    url: Option<NativeText>,
    /// Literal source listing.
    code: Option<CodeBlock>,
    /// Literal output console.
    console: Option<crate::ConsoleBlock>,
}
impl BodyState {
    /// Create a collapsed subtree using the shared placement owner.
    fn new(phase: Phase, view: ToolRenderView<'_>) -> Self {
        Self {
            phase,
            expanded: Rc::new(Cell::new(false)),
            transition: Transition::new([0., 0.]),
            scope: crate::tools::types::placement_scope(view.host),
            url: None,
            code: None,
            console: None,
        }
    }
}
/// Original download with its tool-selected text-preview policy.
pub(in crate::tools) struct Download {
    /// Original encoded record, before decoding a preview.
    pub(in crate::tools) attachment: Attachment,
    /// Whether this MIME type supports extracted text.
    pub(in crate::tools) text: bool,
}
impl Download {
    /// Decode only when the retained tile first observes changed original bytes.
    fn record(&self) -> Attachment {
        let mut record = self.attachment.clone();
        if self.text && !record.content.is_empty() {
            record.extracted_text = crate::utils::base64::decode(&record.content)
                .ok()
                .map(|bytes| maestro_app::decode_utf8(&bytes).into_owned());
        }
        record
    }
    /// Compare caller-visible input, excluding generated identity and derived text.
    fn matches(&self, record: &Attachment) -> bool {
        let original = &self.attachment;
        record.file_name == original.file_name
            && record.mime_type == original.mime_type
            && record.size == original.size
            && record.content == original.content
    }
}
/// One row's retained header, disclosure and positional tiles.
#[derive(Default)]
struct Retained {
    /// Independent header animation and caption.
    header: ToolRendererState,
    /// Child widgets while their branch exists.
    body: Option<BodyState>,
    /// Tiles remain outside the collapsed body.
    tiles: Vec<AttachmentTile>,
}
impl<R: 'static> ToolContent for Content<R> {
    fn render(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        state: &ToolRendererState,
    ) -> ToolContentOutput {
        let slot = state.state::<Retained>();
        let mut retained = std::mem::take(&mut *slot.borrow_mut());
        let mut overlays = Vec::new();
        let response = self.card.draw(ui, view, &mut retained, &mut overlays);
        drop(slot.replace(retained));
        ToolContentOutput { response, overlays }
    }
}
impl CollapsibleCard {
    /// Prepare an ordinary outer tool card.
    pub(in crate::tools) fn result<R: 'static>(self) -> ToolRenderResult {
        ToolRenderResult {
            content: Box::new(Content::<R> {
                card: self,
                renderer: std::marker::PhantomData,
            }),
            is_custom: false,
        }
    }
    /// Reconcile the selected subtree without replacing continuously mounted children.
    fn draw(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        retained: &mut Retained,
        overlays: &mut Vec<crate::AttachmentOverlay>,
    ) -> Result<egui::Response, ToolUiError> {
        if retained.body.as_ref().map(|b| b.phase) != self.body.as_ref().map(|b| b.phase) {
            retained.header = ToolRendererState::default();
            retained.body = self.body.as_ref().map(|b| BodyState::new(b.phase, view));
        }
        let glyph = attachment_content::glyph_node(self.glyph).map_err(ToolUiError::Control)?;
        let caption = ToolHeaderText::Text(self.caption.resolve(view.i18n)?);
        let header = if let Some(body) = &retained.body {
            render_collapsible_header(self.state, glyph, caption, Rc::clone(&body.expanded))
        } else {
            render_header(self.state, glyph, caption)
        };
        let row = ui.vertical(|ui| -> Result<(), ToolUiError> {
            ui.spacing_mut().item_spacing.y = 0.;
            header
                .render(ui, view, &retained.header)
                .collect(overlays)?;
            if let (Some(body), Some(state)) = (&self.body, &mut retained.body) {
                body.render(ui, view, state)?;
            }
            self.tiles(ui, view, retained, overlays)
        });
        row.inner?;
        Ok(row.response)
    }
    /// Reuse positional attachment widgets and forward opened owners immediately.
    fn tiles(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        retained: &mut Retained,
        overlays: &mut Vec<crate::AttachmentOverlay>,
    ) -> Result<(), ToolUiError> {
        retained.tiles.truncate(self.attachments.len());
        for (index, download) in self.attachments.iter().enumerate() {
            if retained
                .tiles
                .get(index)
                .is_some_and(|tile| download.matches(&tile.attachment.borrow()))
            {
                continue;
            }
            AttachmentTile::retain_at(
                &mut retained.tiles,
                index,
                Rc::new(RefCell::new(download.record())),
                view.host,
            );
        }
        if retained.tiles.is_empty() {
            return Ok(());
        }
        ui.add_space(12.);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8., 8.);
            for tile in &mut retained.tiles {
                overlays.extend(
                    tile.render(ui, view.i18n, view.presentation)
                        .map_err(ToolUiError::Attachment)?,
                );
            }
            Ok(())
        })
        .inner
    }
}
impl Body {
    /// Draw through the final animated clip while retaining hidden native owners.
    fn render(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        state: &mut BodyState,
    ) -> Result<(), ToolUiError> {
        let target = if state.expanded.get() {
            [2000., 12.]
        } else {
            [0., 0.]
        };
        let sample = state.transition.sample(
            ui.input(|i| i.time),
            target,
            TransitionStyle {
                duration: 0.3,
                curve: [0.4, 0., 0.2, 1.],
            },
        );
        if sample.active {
            ui.ctx().request_repaint();
        }
        let [height, margin] = sample.value;
        let rect = ui.available_rect_before_wrap();
        let origin = rect.min + egui::vec2(0., margin);
        let mut body = ui.new_child(egui::UiBuilder::new().id(state.scope.0).max_rect(
            egui::Rect::from_min_max(origin, egui::pos2(rect.max.x, f32::INFINITY)),
        ));
        body.set_clip_rect(ui.clip_rect().intersect(egui::Rect::from_min_size(
            origin,
            egui::vec2(rect.width(), height),
        )));
        let _active = view.host.enter_placement_scope(state.scope.0);
        let result = self.children(&mut body, view, state);
        ui.allocate_space(egui::vec2(
            rect.width(),
            margin + body.min_size().y.min(height),
        ));
        result
    }
    /// Reconcile each native child only when its selected branch disappears.
    fn children(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        state: &mut BodyState,
    ) -> Result<(), ToolUiError> {
        ui.spacing_mut().item_spacing.y = 12.;
        if let Some(url) = &self.url {
            let label = "URL:";
            state
                .url
                .get_or_insert_with(|| NativeText::new(Rc::clone(view.host)))
                .render(
                    ui,
                    (&format!("{label} {url}"), TextKind::Url(label.len())),
                    view,
                )?;
        } else {
            state.url = None;
        }
        if let Some((source, language)) = &self.code {
            let code = state
                .code
                .get_or_insert_with(|| CodeBlock::new_literal(Rc::clone(view.host)));
            code.code.clone_from(source);
            (*language).clone_into(&mut code.language);
            code.render(
                ui,
                CodeBlockView {
                    i18n: view.i18n,
                    presentation: view.presentation,
                    highlighter: view.highlighter,
                    modal: view.modal,
                },
            )
            .map_err(ToolUiError::reading)?;
        } else {
            state.code = None;
        }
        if let Some((content, variant)) = &self.console {
            let console = state
                .console
                .get_or_insert_with(|| crate::ConsoleBlock::new(Rc::clone(view.host)));
            console.content.clone_from(content);
            console.variant = *variant;
            console
                .render(ui, view.i18n, view.presentation, view.modal)
                .map_err(ToolUiError::reading)?;
        } else {
            state.console = None;
        }
        Ok(())
    }
}
