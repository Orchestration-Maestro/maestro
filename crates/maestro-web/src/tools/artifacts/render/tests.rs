//! Unbounded caller headers cannot displace the panel-owned close control.
use super::*;
use crate::components::download_button::tests::{Host, Language};
use crate::tools::artifacts::artifact_element::ArtifactElement;
use std::{cell::RefCell, rc::Rc};

/// External header with a control wider than the initial reservation.
struct WideHeader;
impl ArtifactElement for WideHeader {
    fn filename(&self) -> &'static str {
        "wide"
    }
    fn set_filename(&mut self, _: String) {}
    fn content(&self) -> &'static str {
        ""
    }
    fn set_content(&mut self, _: String) {}
    fn set_visibility(&mut self, _: bool, _: Option<egui::Id>) -> Result<(), AttachmentUiError> {
        Ok(())
    }
    fn get_header_buttons(
        &mut self,
        ui: &mut egui::Ui,
        _: ArtifactView<'_>,
    ) -> Result<(), AttachmentUiError> {
        ui.add_sized([200., 32.], egui::Button::new("Caller control"));
        Ok(())
    }
    fn render(&mut self, _: &mut egui::Ui, _: ArtifactView<'_>) -> Result<(), AttachmentUiError> {
        Ok(())
    }
}

/// A wide header is clipped, while close remains visible and accepts pointer input.
#[test]
fn artifact_panel_wide_header_cannot_displace_close() {
    let panel = ArtifactsPanel::new(Rc::new(Host::default()));
    panel.elements.borrow_mut().insert(
        "wide".into(),
        Rc::new(RefCell::new(Some(Box::new(WideHeader)))),
    );
    let pack = maestro_theme::load_brand_pack(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/brand/brand.json"),
        &maestro_theme::theme::NativeThemeOperations,
    )
    .unwrap();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    let i18n = crate::I18n::new(Rc::new(Language)).unwrap();
    let context = egui::Context::default();
    context.options_mut(|options| options.max_passes = 1.try_into().unwrap());
    let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400., 40.));
    let mut clicked = false;
    for pressed in [None, Some(true), Some(false)] {
        let mut input = egui::RawInput {
            screen_rect: Some(bounds),
            ..Default::default()
        };
        if let Some(pressed) = pressed {
            input.events = vec![
                egui::Event::PointerMoved(egui::pos2(384., 20.)),
                egui::Event::PointerButton {
                    pos: egui::pos2(384., 20.),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
        }
        let mut output = context.run_ui(input, |ui| {
            let (close, _) = panel
                .controls(
                    ui,
                    bounds.with_min_x(328.),
                    Some("wide"),
                    ArtifactView {
                        i18n: &i18n,
                        presentation: &presentation,
                    },
                )
                .unwrap();
            assert!(bounds.contains_rect(close.rect), "{:?}", close.rect);
            clicked |= close.clicked();
        });
        output.textures_delta.clear();
    }
    assert!(clicked);
}
