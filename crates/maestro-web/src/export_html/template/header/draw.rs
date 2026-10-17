//! Canvas controls sharing the native reading surface's measured slots.
use super::{CONTROLS, HeaderAction, HeaderState, HeaderView, SessionDocument, prepare};
use crate::browser::attachment_content::{
    self, ReadingContent, reading::controlled::ControlledReading,
};
use std::{io, rc::Rc};

/// Owned draw operands for one prepared native surface.
struct Placement<'a> {
    /// Retained feature registration.
    state: &'a mut HeaderState,
    /// Owned native factory for this draw.
    content: Rc<ReadingContent>,
    /// Real canvas button sizes.
    sizes: Vec<(egui::Id, egui::Vec2)>,
    /// Caller-selected synchronous style.
    view: HeaderView<'a>,
}
impl Placement<'_> {
    /// Measure and publish through the shared mixed-content flow.
    fn draw(&mut self, ui: &mut egui::Ui) -> io::Result<Vec<HeaderAction>> {
        let layout = ControlledReading {
            host: self.state.host.as_ref(),
            id: self.state.id,
            content: &self.content,
            sizes: &self.sizes,
            modal: None,
            presentation: self.view.presentation,
        }
        .draw(ui)
        .map_err(io::Error::other)?;
        Ok(self.controls(ui, layout.response.rect, &layout.controls))
    }
    /// Paint full slots while retaining their visible intersections for gesture admission.
    fn controls(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        controls: &[(egui::Id, egui::Rect)],
    ) -> Vec<HeaderAction> {
        let mut visible_slots = Vec::new();
        let mut actions = Vec::new();
        for ((label, tooltip, action), (id, _)) in CONTROLS.iter().zip(&self.sizes) {
            let Some((_, full)) = controls.iter().find(|(slot, _)| slot == id) else {
                continue;
            };
            let visible = full.intersect(rect).intersect(ui.clip_rect());
            if !visible.is_positive() {
                continue;
            }
            let mut child = ui.new_child(egui::UiBuilder::new().id_salt(id).max_rect(*full));
            child.set_clip_rect(visible);
            let response = child.put(*full, button(label)).on_hover_text(*tooltip);
            if response.enabled() {
                visible_slots.push((*action, visible, response.layer_id));
            }
            if response.clicked() && !response.clicked_by(egui::PointerButton::Primary) {
                actions.push(*action);
            }
        }
        let events = ui.input(|input| input.events.clone());
        actions.extend(
            crate::browser::attachment_content::reading::controls::pointer_actions(
                &events,
                &mut self.state.pressed,
                &visible_slots,
                ui.ctx(),
            ),
        );
        actions
    }
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
                let mut placement = Placement {
                    sizes: sizes(ui, state.id),
                    state,
                    content: Rc::clone(&content),
                    view,
                };
                placement.draw(ui)
            },
        )
        .inner?;
    state.content = Some((prepared, content));
    Ok(actions)
}
