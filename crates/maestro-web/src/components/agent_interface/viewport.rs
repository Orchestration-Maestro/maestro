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
    /// Whether the last draw emitted an enabled focusable control; false until first drawn.
    focusable: bool,
    /// Focusable widget identities from this row's last draw, retired with the row.
    controls: Vec<egui::Id>,
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
    /// Constant-time lookup of native slots and their focusable controls.
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
        let (mut rows, input_clip) = canvas_rows(ui, self.id, rect);
        let sizes = self.draw_rows(
            &mut rows,
            (origin, width, visible, rem),
            (refresh, &changes.slots, extra_dirty),
            draw,
        );
        ui.ctx()
            .data_mut(|data| data.remove::<egui::Rect>(input_clip));
        let sizes = sizes?;
        drop(rows);
        self.commit(ui, rect, (&sizes, scroll.top, refresh), view)?;
        self.presentation = presentation;
        self.rem = rem;
        Ok(ui.allocate_rect(rect, egui::Sense::hover()))
    }
    /// Publish changed native sizes, follow, then commit final canvas and aperture geometry.
    fn commit(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        (sizes, start_top, refresh): (&[(egui::Id, egui::Vec2)], f64, bool),
        view: ToolRenderView<'_>,
    ) -> Result<(), ToolUiError> {
        let layer = egui::LayerId::new(ui.layer_id().order, self.id.with("canvas"));
        let parent = ui
            .ctx()
            .layer_transform_to_global(ui.layer_id())
            .unwrap_or_default();
        self.place(ui, rect, (sizes, vec![]), view)?;
        if self.follow.enabled && (refresh || !sizes.is_empty()) {
            self.host
                .set_reading_scroll(self.id, self.host.reading_scroll(self.id)?.height)?;
        }
        let final_top = self.host.reading_scroll(self.id)?.top;
        let transform = parent
            * egui::emath::TSTransform::from_translation(egui::vec2(
                0.,
                attachment_content::points(start_top - final_top),
            ));
        let apertures = commit_geometry(
            ui.ctx(),
            layer,
            transform,
            parent.mul_rect(rect.intersect(ui.clip_rect())),
        );
        self.place(ui, rect, (&[], apertures), view)?;
        Ok(())
    }
    /// Visit visible or invalidated rows plus the focused row and its focusable neighbours.
    fn draw_rows(
        &mut self,
        ui: &mut egui::Ui,
        (origin, width, visible, rem): (egui::Pos2, f32, egui::Rect, f32),
        (refresh, dirty_slots, extra_dirty): (bool, &HashSet<egui::Id>, Option<usize>),
        mut draw: impl FnMut(usize, &mut egui::Ui) -> Result<(), ToolUiError>,
    ) -> Result<Vec<(egui::Id, egui::Vec2)>, ToolUiError> {
        let dirty_start = dirty_slots
            .iter()
            .filter_map(|id| self.positions.get(id).copied())
            .chain(extra_dirty)
            .min();
        let focused = self.focus_rows(ui.ctx());
        let range = self.visit_range((origin.y, visible), (refresh, dirty_start), &focused);
        let start = range.start;
        let count = self.rows.len();
        let mut top = self.rows.get(start).map_or(0., |row| row.top);
        let mut sizes = Vec::new();
        for position in range {
            let row = &mut self.rows[position];
            let dirty = refresh || dirty_slots.contains(&row.id) || extra_dirty == Some(position);
            let row_origin = origin + egui::vec2(0., top);
            let in_view =
                row_origin.y <= visible.bottom() && row_origin.y + row.height >= visible.top();
            if dirty
                || in_view
                || focused.contains(&position)
                || (self.follow.enabled && dirty_start.is_some())
            {
                let first = ui
                    .ctx()
                    .viewport(|v| v.this_pass.widgets.get_layer(ui.layer_id()).count());
                let height = draw_row(
                    ui,
                    (&self.host, self.id),
                    (row.id, row_origin, width),
                    |ui| draw(position, ui),
                )?;
                row.record_controls(ui, first, (&mut self.positions, position));
                let gap = f32::from(u8::from(position + 1 < count)) * 0.75 * rem;
                sizes.extend(row.resize(width, height + gap, dirty));
            }
            row.top = top;
            top += row.height;
        }
        Ok(sizes)
    }
    /// Admit only the focused row and its nearest known focusable neighbours beyond normal culling.
    fn focus_rows(&self, context: &egui::Context) -> Vec<usize> {
        let Some(focused) = context
            .memory(egui::Memory::focused)
            .and_then(|id| self.positions.get(&id).copied())
        else {
            return vec![];
        };
        (0..focused)
            .rev()
            .find(|&i| self.rows[i].focusable)
            .into_iter()
            .chain(Some(focused))
            .chain((focused + 1..self.rows.len()).find(|&i| self.rows[i].focusable))
            .collect()
    }
    /// Walk one ordered geometry interval, drawing only visible, dirty or focus-admitted rows.
    fn visit_range(
        &self,
        (origin, visible): (f32, egui::Rect),
        (refresh, dirty_start): (bool, Option<usize>),
        focused: &[usize],
    ) -> std::ops::Range<usize> {
        let first = self
            .rows
            .partition_point(|row| origin + row.top + row.height < visible.top());
        let last = self
            .rows
            .partition_point(|row| origin + row.top <= visible.bottom());
        let start = if refresh {
            0
        } else {
            first.min(dirty_start.unwrap_or(first))
        };
        let end = if refresh || dirty_start.is_some() {
            self.rows.len()
        } else {
            last
        };
        focused
            .iter()
            .min()
            .copied()
            .map_or(start, |i| start.min(i))
            ..focused
                .iter()
                .max()
                .copied()
                .map_or(end, |i| end.max(i + 1))
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
            for id in &row.controls {
                self.positions.remove(id);
            }
        }
        self.rows.truncate(count);
        while self.rows.len() < count {
            self.positions
                .insert(self.id.with(self.rows.len()), self.rows.len());
            self.rows.push(Row {
                id: self.id.with(self.rows.len()),
                top: 0.,
                height: 0.,
                focusable: false,
                controls: vec![],
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
    /// Refresh focus provenance from this row's emitted widgets without retaining widget records.
    fn record_controls(
        &mut self,
        ui: &egui::Ui,
        first: usize,
        (positions, position): (&mut HashMap<egui::Id, usize>, usize),
    ) {
        for id in self.controls.drain(..) {
            positions.remove(&id);
        }
        self.controls = ui.ctx().viewport(|v| {
            v.this_pass
                .widgets
                .get_layer(ui.layer_id())
                .skip(first)
                .filter(|w| w.enabled && w.sense.is_focusable())
                .map(|w| w.id)
                .collect()
        });
        self.focusable = !self.controls.is_empty();
        positions.extend(self.controls.iter().map(|&id| (id, position)));
    }
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
            .id(id)
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

/// Commit emitted paint, hit rectangles and native apertures at the same final transform.
fn commit_geometry(
    context: &egui::Context,
    layer: egui::LayerId,
    transform: egui::emath::TSTransform,
    viewport: egui::Rect,
) -> Vec<egui::Rect> {
    let clip = transform.inverse().mul_rect(viewport);
    context.set_transform_layer(layer, transform);
    context.graphics_mut(|graphics| {
        let paint = graphics.entry(layer);
        let end = paint.next_idx().0;
        for index in 0..end {
            paint.mutate_shape(egui::layers::ShapeIdx(index), |shape| {
                shape.clip_rect = shape.clip_rect.intersect(clip);
            });
        }
    });
    let widgets = context.viewport(|v| {
        v.this_pass
            .widgets
            .get_layer(layer)
            .copied()
            .collect::<Vec<_>>()
    });
    let mut apertures = vec![];
    for widget in widgets {
        if let Some(mut response) = context.read_response(widget.id) {
            response.interact_rect = widget.interact_rect.intersect(clip);
            if widget.enabled && widget.sense.senses_click() && response.interact_rect.is_positive()
            {
                apertures.push(transform.mul_rect(response.interact_rect));
            }
            let _ = response.interact(widget.sense);
        }
    }
    apertures
}

/// Collect row paint without culling at the old scroll offset, but admit input only inside the viewport.
fn canvas_rows(ui: &mut egui::Ui, id: egui::Id, rect: egui::Rect) -> (egui::Ui, egui::Id) {
    let layer = egui::LayerId::new(ui.layer_id().order, id.with("canvas"));
    let parent = ui
        .ctx()
        .layer_transform_to_global(ui.layer_id())
        .unwrap_or_default();
    ui.ctx().set_sublayer(ui.layer_id(), layer);
    ui.ctx().set_transform_layer(layer, parent);
    let input_clip = attachment_content::reading::controls::set_pointer_clip(
        ui.ctx(),
        layer,
        parent.mul_rect(rect.intersect(ui.clip_rect())),
    );
    let mut rows = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(rect));
    rows.set_clip_rect(egui::Rect::EVERYTHING);
    (rows, input_clip)
}
