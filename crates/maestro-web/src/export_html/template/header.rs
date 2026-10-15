//! Native offline session identity.
use super::SessionDocument;
use crate::browser::attachment_content::{
    self, AttachmentContent, AttachmentHost, AttachmentSurface, ReadingContent, ReadingPhase,
    ReadingPlacement,
};
use maestro_theme::BrandPresentation;
use std::{io, rc::Rc};
/// Prepare only the reached identity, leaving other document fields unread.
fn prepare(document: &SessionDocument) -> io::Result<String> {
    let id = &document.data["header"]["id"];
    let identity = if maestro_app::is_truthy(id) {
        id.as_str()
            .ok_or_else(|| super::document::invalid("nonstring session identity"))?
    } else {
        "unknown"
    };
    Ok(format!("Session: {identity}"))
}
/// Retained reading identity scoped to a replaceable native host.
pub struct HeaderState {
    /// Placement owner selected by the caller.
    host: Rc<dyn AttachmentHost>,
    /// Independent registry identity.
    id: egui::Id,
    /// Last successfully published identity and its owned factory.
    content: Option<(String, Rc<ReadingContent>)>,
}
impl HeaderState {
    /// Retain the supplied host without selecting a platform adapter.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            host,
            id: attachment_content::identity(),
            content: None,
        }
    }
}
impl Drop for HeaderState {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
/// Synchronous presentation operands, never retained by native callbacks.
#[derive(Clone, Copy)]
pub struct HeaderView<'a> {
    /// Caller-selected brand projection.
    pub presentation: &'a BrandPresentation<'a>,
}
/// Render a literal browser identity in a parent-independent bounded viewport.
/// # Errors
/// Propagates invalid identity, presentation and reached native placement failures.
pub fn render_header(
    ui: &mut egui::Ui,
    document: &SessionDocument,
    state: &mut HeaderState,
    view: HeaderView<'_>,
) -> io::Result<()> {
    let title = prepare(document)?;
    let content = match &state.content {
        Some((old, content)) if *old == title => Rc::clone(content),
        _ => {
            #[cfg(target_arch = "wasm32")]
            let owned = title.clone();
            Rc::new(ReadingContent {
                #[cfg(target_arch = "wasm32")]
                mount: Rc::new(move |document| browser::mount(document, &owned)),
            })
        }
    };
    let host = state.host.as_ref();
    ui.scope_builder(
        egui::UiBuilder::new().layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            attachment_content::style(ui, view.presentation)?;
            let place = |ui: &egui::Ui, rect, phase| {
                let surface = AttachmentSurface {
                    id: state.id,
                    rect,
                    clip_rect: ui.clip_rect(),
                    pixels_per_point: ui.ctx().pixels_per_point(),
                    opacity: 1.,
                    occluders: vec![],
                    content: AttachmentContent::Reading(ReadingPlacement {
                        content: Rc::clone(&content),
                        phase,
                        modal: None,
                        context: ui.ctx(),
                    }),
                };
                host.present(surface, view.presentation)
                    .map_err(io::Error::other)
            };
            let width = ui.available_width().max(0.);
            place(
                ui,
                egui::Rect::from_min_size(ui.next_widget_position(), egui::vec2(width, 0.)),
                ReadingPhase::Measure,
            )?;
            let layout = host.reading_layout(state.id).map_err(io::Error::other)?;
            let height = layout.height.min(ui.available_height()).max(0.);
            let rect = ui.allocate_space(egui::vec2(width, height)).1;
            place(ui, rect, ReadingPhase::Publish)
        },
    )
    .inner?;
    state.content = Some((title, content));
    Ok(())
}
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(test)]
mod tests;
