//! Canvas controls sharing the native reading surface's measured slots.
use super::{CONTROLS, HeaderAction, HeaderState, HeaderView, SessionDocument, prepare};
use crate::browser::attachment_content::{
    self, AttachmentContent, AttachmentSurface, ReadingContent, ReadingPhase, ReadingPlacement,
};
use std::{io, rc::Rc};

/// Owned draw operands for one prepared native surface.
struct Placement<'a> {
    /// Retained feature registration.
    state: &'a HeaderState,
    /// Owned native factory for this draw.
    content: Rc<ReadingContent>,
    /// Real canvas button sizes.
    sizes: Vec<(egui::Id, egui::Vec2)>,
    /// Caller-selected synchronous style.
    view: HeaderView<'a>,
}
impl Placement<'_> {
    /// Submit one explicit phase to the existing host.
    fn place(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        phase: ReadingPhase,
        holes: Vec<egui::Rect>,
    ) -> io::Result<()> {
        self.state
            .host
            .present(
                AttachmentSurface {
                    id: self.state.id,
                    rect,
                    clip_rect: ui.clip_rect(),
                    pixels_per_point: ui.ctx().pixels_per_point(),
                    opacity: 1.,
                    occluders: holes,
                    content: AttachmentContent::Reading(ReadingPlacement {
                        content: Rc::clone(&self.content),
                        phase,
                        sizes: &self.sizes,
                        modal: None,
                        context: ui.ctx(),
                    }),
                },
                self.view.presentation,
            )
            .map_err(io::Error::other)
    }
    /// Measure once, then use the same snapshot for publication and controls.
    fn draw(&self, ui: &mut egui::Ui) -> io::Result<Vec<HeaderAction>> {
        let available = ui.available_size().max(egui::Vec2::ZERO);
        self.place(
            ui,
            egui::Rect::from_min_size(ui.next_widget_position(), available),
            ReadingPhase::Measure,
            vec![],
        )?;
        let layout = self
            .state
            .host
            .reading_layout(self.state.id)
            .map_err(io::Error::other)?;
        let rect = ui
            .allocate_space(egui::vec2(
                available.x,
                layout.height.min(available.y).max(0.),
            ))
            .1;
        let controls: Vec<_> = layout
            .controls
            .into_iter()
            .map(|(id, r)| (id, r.translate(rect.min.to_vec2())))
            .collect();
        self.place(
            ui,
            rect,
            ReadingPhase::Publish,
            controls.iter().map(|(_, r)| *r).collect(),
        )?;
        Ok(CONTROLS
            .iter()
            .zip(&self.sizes)
            .filter_map(|((label, tooltip, action), (id, _))| {
                let full = controls.iter().find(|(slot, _)| slot == id)?.1;
                let visible = full.intersect(rect).intersect(ui.clip_rect());
                if !visible.is_positive() {
                    return None;
                }
                let mut child = ui.new_child(egui::UiBuilder::new().id_salt(id).max_rect(full));
                child.set_clip_rect(visible);
                let response = child.put(full, button(label)).on_hover_text(*tooltip);
                activated(&response, visible).then_some(*action)
            })
            .collect())
    }
}
/// Admit pointer clicks only within current visible canvas points, retaining keyboard clicks.
/// Native CSS pixels have already been converted per axis by the host's transform.
fn activated(response: &egui::Response, visible_canvas_points: egui::Rect) -> bool {
    response.clicked()
        && (!response.clicked_by(egui::PointerButton::Primary)
            || response
                .interact_pointer_pos()
                .is_some_and(|canvas_points| visible_canvas_points.contains(canvas_points)))
}
/// Use the same existing button for sizing and painting.
fn button(label: &str) -> egui::Button<'_> {
    egui::Button::new(label).wrap_mode(egui::TextWrapMode::Extend)
}
/// Measure actual widgets without paint, interaction or parent allocation.
fn sizes(ui: &mut egui::Ui, id: egui::Id) -> Vec<(egui::Id, egui::Vec2)> {
    CONTROLS
        .iter()
        .enumerate()
        .map(|(i, (label, _, _))| {
            let id = id.with(i);
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt((id, "measure"))
                    .invisible()
                    .sizing_pass()
                    .max_rect(ui.max_rect()),
            );
            (id, child.add(button(label)).rect.size())
        })
        .collect()
}
/// Draw literal native metadata with canvas controls using one live snapshot.
/// # Errors
/// Propagates metadata, presentation and reached native transaction errors.
pub fn render_header(
    ui: &mut egui::Ui,
    document: &SessionDocument,
    state: &mut HeaderState,
    view: HeaderView<'_>,
) -> io::Result<Vec<HeaderAction>> {
    let prepared = Rc::new(prepare(document, view.localize_date)?);
    let content = match &state.content {
        Some((old, content)) if **old == *prepared => Rc::clone(content),
        _ => {
            #[cfg(target_arch = "wasm32")]
            let owned = Rc::clone(&prepared);
            #[cfg(target_arch = "wasm32")]
            let ids = [
                state.id.with(0_usize),
                state.id.with(1_usize),
                state.id.with(2_usize),
            ];
            Rc::new(ReadingContent {
                #[cfg(target_arch = "wasm32")]
                mount: Rc::new(move |document| super::browser::mount(document, &owned, ids)),
            })
        }
    };
    let actions = ui
        .scope_builder(
            egui::UiBuilder::new().layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                attachment_content::style(ui, view.presentation)?;
                ui.visuals_mut().widgets.hovered.expansion = 0.;
                ui.visuals_mut().widgets.active.expansion = 0.;
                ui.visuals_mut().widgets.hovered.bg_stroke.color =
                    attachment_content::color(view.presentation, "accent")?;
                ui.visuals_mut().widgets.active.bg_stroke.color =
                    attachment_content::color(view.presentation, "accent")?;
                ui.spacing_mut().button_padding = egui::vec2(
                    attachment_content::points(view.presentation.spacing["sm"]),
                    attachment_content::points(view.presentation.spacing["xs"] / 2.),
                );
                if let Some(font) = ui.style_mut().text_styles.get_mut(&egui::TextStyle::Button) {
                    font.size *= 10. / 12.;
                }
                let placement = Placement {
                    state,
                    content: Rc::clone(&content),
                    sizes: sizes(ui, state.id),
                    view,
                };
                placement.draw(ui)
            },
        )
        .inner?;
    state.content = Some((prepared, content));
    Ok(actions)
}
