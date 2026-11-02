#![cfg(test)]
use crate::{brand::pack, effects_support::Host, support};
use maestro_web::{I18n, components::copy_button::CopyButton};
use std::rc::Rc;

pub struct Canvas {
    pub host: Rc<Host>,
    pub button: CopyButton,
    pub context: egui::Context,
    pub clip: egui::Rect,
    pub enabled: bool,
    pub rect: egui::Rect,
    pub paint: crate::canvas_paint::Paint,
}
impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}
impl Canvas {
    /// Create an independent controlled copy surface.
    /// # Panics
    /// Panics if the supplied localization cannot initialize.
    #[must_use]
    pub fn new() -> Self {
        let host = Rc::new(Host::default());
        let i18n: I18n = I18n::new(Rc::new(support::Host::default())).unwrap();
        Self {
            button: CopyButton::new(host.clone(), &i18n).unwrap(),
            host,
            context: egui::Context::default(),
            clip: egui::Rect::EVERYTHING,
            enabled: true,
            rect: egui::Rect::NOTHING,
            paint: crate::canvas_paint::Paint::default(),
        }
    }
    /// Draw a controlled frame.
    /// # Panics
    /// Panics if the component fails to render.
    pub fn frame(&mut self, events: Vec<egui::Event>) -> (egui::Response, egui::FullOutput) {
        let button = &mut self.button;
        let result = draw(
            &self.context,
            &mut self.paint,
            events,
            (self.clip, self.enabled),
            |ui, i18n, presentation| button.render(ui, i18n, presentation).unwrap(),
        );
        self.rect = result.0.rect;
        result
    }
    pub fn activate(&mut self) -> egui::FullOutput {
        self.frame(vec![]);
        self.frame(pointer(self.rect.center(), true));
        self.frame(pointer(self.rect.center(), false)).1
    }
    pub fn settle(&self, index: usize, success: bool) {
        self.host.results.borrow()[index].set(Some(success));
        self.host.run_work();
    }
}
pub use crate::support::pointer::pointer;
#[must_use]
pub fn copied(output: &egui::FullOutput, canvas: &Canvas) -> bool {
    canvas
        .paint
        .text_visible(&canvas.context, &output.shapes, "Copied!")
}

/// Draw a control recipe through the shared frame, presentation and final-clip paint route.
/// # Panics
/// Panics if the checked-in brand, localization or component cannot render.
pub fn draw(
    context: &egui::Context,
    paint: &mut crate::canvas_paint::Paint,
    events: Vec<egui::Event>,
    policy: (egui::Rect, bool),
    render: impl FnOnce(&mut egui::Ui, &I18n, &maestro_theme::BrandPresentation<'_>) -> egui::Response,
) -> (egui::Response, egui::FullOutput) {
    let mut render = Some(render);
    let pack = pack("brand.json").unwrap();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    let i18n: I18n = I18n::new(Rc::new(support::Host::default())).unwrap();
    let mut response = None;
    let context = context.clone();
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800., 600.),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            ui.set_clip_rect(policy.0);
            if !policy.1 {
                ui.disable();
            }
            response = Some(render.take().unwrap()(ui, &i18n, &presentation));
        },
    );
    paint.update(&output);
    output.textures_delta.clear();
    (response.unwrap(), output)
}
