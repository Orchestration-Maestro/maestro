//! Native transcript containment with retained ordered canvas row geometry.
use super::scroll::FollowState;
use crate::{
    browser::attachment_content::{
        self, AttachmentContent, AttachmentHost, AttachmentSurface, ReadingContent, ReadingPhase,
        ReadingPlacement, ReadingRoot,
    },
    tools::types::{ToolRenderView, ToolUiError},
};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

/// Retained positional row geometry in canvas points.
struct Row {
    /// Native slot identity, independent of content.
    id: egui::Id,
    /// Ordered offset from the padded content origin.
    top: f32,
    /// Last successfully drawn height, including its following gap.
    height: f32,
}
/// The single native scroll owner around stable and streaming rows.
pub(super) struct TranscriptViewport {
    /// Existing effect owner.
    host: Rc<dyn AttachmentHost>,
    /// Native containing surface.
    id: egui::Id,
    /// Retained root factory.
    content: Rc<ReadingContent>,
    /// Ordered geometry used by clean scroll frames.
    rows: Vec<Row>,
    /// Constant-time lookup of native resize observations.
    positions: HashMap<egui::Id, usize>,
    /// Native placement from the previous successful publication.
    placement: Option<(egui::Rect, egui::Rect, u32)>,
    /// Last successfully reconciled native presentation properties.
    presentation: String,
    /// Last successfully resolved document rem.
    rem: f32,
    /// Reader-follow transitions.
    pub(super) follow: FollowState,
}
impl TranscriptViewport {
    /// Keep native text attached independently of canvas culling.
    pub(super) fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            host,
            id: attachment_content::identity(),
            content: content(),
            rows: vec![],
            positions: HashMap::new(),
            placement: None,
            presentation: String::new(),
            rem: 0.,
            follow: FollowState::default(),
        }
    }
    /// Draw changed native rows once, then only intersecting rows on clean frames.
    pub(super) fn render(
        &mut self,
        ui: &mut egui::Ui,
        (count, refresh, extra_dirty): (usize, bool, Option<usize>),
        view: ToolRenderView<'_>,
        draw: impl FnMut(usize, &mut egui::Ui) -> Result<(), ToolUiError>,
    ) -> Result<egui::Response, ToolUiError> {
        let rect = ui.available_rect_before_wrap();
        let placement = (rect, ui.clip_rect(), ui.ctx().pixels_per_point().to_bits());
        let moved = self.placement != Some(placement);
        if moved {
            self.place(ui, rect, (&[], vec![]), view)?;
            self.placement = Some(placement);
        }
        let changes = attachment_content::browser::invalidation::take(ui.ctx(), self.id);
        let scroll = self.host.reading_scroll(self.id)?;
        if changes.scrolled {
            self.follow.observe(Some(scroll));
        }
        let mut presentation = String::new();
        crate::browser::append_brand_properties(&mut presentation, view.presentation);
        let rem = self.host.root_rem()?;
        let refresh = refresh
            || moved
            || changes.canvas_resized
            || self.presentation != presentation
            || self.rem.to_bits() != rem.to_bits();
        if self.rows.len() != count {
            self.resize(count)?;
        }
        let width = (rect.width().min(48. * rem) - 2. * rem).max(0.);
        let origin = egui::pos2(
            rect.center().x - width / 2.,
            rect.top() + rem - attachment_content::points(scroll.top),
        );
        let visible = egui::Rect::from_min_size(rect.min, rect.size());
        drop(self.host.take_reading_apertures(self.id));
        let sizes = self.draw_rows(
            ui,
            (origin, width, visible, rem),
            (refresh, &changes.slots, extra_dirty),
            draw,
        )?;
        let apertures = self.host.take_reading_apertures(self.id);
        self.place(ui, rect, (&sizes, apertures), view)?;
        if self.follow.enabled && (refresh || !sizes.is_empty()) {
            self.host
                .set_reading_scroll(self.id, self.host.reading_scroll(self.id)?.height)?;
        }
        self.presentation = presentation;
        self.rem = rem;
        Ok(ui.allocate_rect(rect, egui::Sense::hover()))
    }
    /// Visit visible canvas rows plus explicitly invalidated native rows, never all clean rows.
    fn draw_rows(
        &mut self,
        ui: &mut egui::Ui,
        (origin, width, visible, rem): (egui::Pos2, f32, egui::Rect, f32),
        (refresh, dirty_slots, extra_dirty): (bool, &HashSet<egui::Id>, Option<usize>),
        mut draw: impl FnMut(usize, &mut egui::Ui) -> Result<(), ToolUiError>,
    ) -> Result<Vec<(egui::Id, egui::Vec2)>, ToolUiError> {
        let first = self
            .rows
            .partition_point(|row| origin.y + row.top + row.height < visible.top());
        let last = self
            .rows
            .partition_point(|row| origin.y + row.top <= visible.bottom());
        let dirty_start = dirty_slots
            .iter()
            .filter_map(|id| self.positions.get(id).copied())
            .chain(extra_dirty)
            .min();
        let start = if refresh {
            0
        } else {
            first.min(dirty_start.unwrap_or(first))
        };
        let count = self.rows.len();
        let end = if refresh || dirty_start.is_some() {
            count
        } else {
            last
        };
        let mut top = self.rows.get(start).map_or(0., |row| row.top);
        let mut sizes = Vec::new();
        for position in start..end {
            let row = &mut self.rows[position];
            let dirty = refresh || dirty_slots.contains(&row.id) || extra_dirty == Some(position);
            let row_origin = origin + egui::vec2(0., top);
            let in_view =
                row_origin.y <= visible.bottom() && row_origin.y + row.height >= visible.top();
            if dirty || in_view {
                let height = draw_row(
                    ui,
                    (&self.host, self.id),
                    (row.id, row_origin, width),
                    |ui| draw(position, ui),
                )?;
                let gap = f32::from(u8::from(position + 1 < count)) * 0.75 * rem;
                sizes.extend(row.resize(width, height + gap, dirty));
            }
            row.top = top;
            top += row.height;
        }
        Ok(sizes)
    }
    /// Detach the containing tree when no session remains, without changing the follow override.
    pub(super) fn clear(&mut self) {
        self.host.remove(self.id);
        self.rows.clear();
        self.positions.clear();
        self.placement = None;
        self.presentation.clear();
    }
    /// Publish positional additions before admitting children; factories run outside registry borrows.
    fn resize(&mut self, count: usize) -> Result<(), ToolUiError> {
        self.host
            .resize_reading_slots(self.id, count, &|document, position| {
                let element = document
                    .create_element("div")
                    .map_err(|e| attachment_content::browser::host_error(&e))?;
                element
                    .set_attribute("class", "maestro-transcript-slot")
                    .map_err(|e| attachment_content::browser::host_error(&e))?;
                Ok((self.id.with(position), element))
            })?;
        for row in &self.rows[count.min(self.rows.len())..] {
            self.positions.remove(&row.id);
        }
        self.rows.truncate(count);
        while self.rows.len() < count {
            self.positions
                .insert(self.id.with(self.rows.len()), self.rows.len());
            self.rows.push(Row {
                id: self.id.with(self.rows.len()),
                top: 0.,
                height: 0.,
            });
        }
        Ok(())
    }
    /// Stage once and publish all changed slot sizes in a single registry transaction.
    fn place(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        (sizes, apertures): (&[(egui::Id, egui::Vec2)], Vec<egui::Rect>),
        view: ToolRenderView<'_>,
    ) -> Result<(), ToolUiError> {
        for phase in [ReadingPhase::Measure, ReadingPhase::Publish] {
            self.host.present(
                AttachmentSurface {
                    id: self.id,
                    rect,
                    clip_rect: ui.clip_rect(),
                    pixels_per_point: ui.ctx().pixels_per_point(),
                    opacity: 1.,
                    occluders: apertures.clone(),
                    content: AttachmentContent::Reading(ReadingPlacement {
                        content: Rc::clone(&self.content),
                        phase,
                        intrinsic_width: false,
                        sizes,
                        modal: view.modal,
                        context: ui.ctx(),
                    }),
                },
                view.presentation,
            )?;
        }
        Ok(())
    }
}
impl Drop for TranscriptViewport {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
impl Row {
    /// Retain exact measured geometry, returning only changed native dimensions.
    fn resize(&mut self, width: f32, height: f32, dirty: bool) -> Option<(egui::Id, egui::Vec2)> {
        if !dirty && self.height.to_bits() == height.to_bits() {
            return None;
        }
        self.height = height;
        Some((self.id, egui::vec2(width, height)))
    }
}
/// Invoke a recipe once under the planned row origin, independent of stale native slot bounds.
fn draw_row(
    ui: &mut egui::Ui,
    (host, container): (&Rc<dyn AttachmentHost>, egui::Id),
    (id, origin, width): (egui::Id, egui::Pos2, f32),
    draw: impl FnOnce(&mut egui::Ui) -> Result<(), ToolUiError>,
) -> Result<f32, ToolUiError> {
    let _scope = host.enter_reading_container(container, Some(id), Some(origin))?;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(id)
            .max_rect(egui::Rect::from_min_size(
                origin,
                egui::vec2(width, f32::INFINITY),
            ))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(ui.clip_rect());
    draw(&mut child)?;
    Ok(child.min_rect().height())
}
/// Mount only the containing layout; each row keeps its independently owned native subtree.
fn content() -> Rc<ReadingContent> {
    Rc::new(ReadingContent {
        mount: Rc::new(|document| {
            use attachment_content::browser::{append, element};
            let root = element(document, "div", "maestro-transcript")?;
            let style = element(document, "style", "")?;
            style.set_text_content(Some(include_str!("../transcript.css")));
            append(&root, &style)?;
            Ok(ReadingRoot {
                frame_style: "",
                decoration: None,
                element: root,
                slots: vec![],
                owner: None,
                commit: None,
            })
        }),
    })
}
