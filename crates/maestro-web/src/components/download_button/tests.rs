//! Byte control activation and geometry.
#[cfg(not(target_arch = "wasm32"))]
use super::*;
#[cfg(not(target_arch = "wasm32"))]
use crate::browser::attachment_content::*;
#[cfg(not(target_arch = "wasm32"))]
use std::{cell::RefCell, rc::Rc};
/// Byte-only controlled effect boundary.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub(crate) struct Host(RefCell<Vec<(String, String, Vec<u8>)>>);
#[cfg(not(target_arch = "wasm32"))]
impl AttachmentHost for Host {
    fn copy_to_clipboard(
        &self,
        _: &str,
        _: crate::browser::attachment_content::ClipboardMode,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool>>> {
        unreachable!("unexpected clipboard operation")
    }
    fn capture_focus(
        &self,
    ) -> Result<
        Option<std::rc::Rc<crate::browser::attachment_content::FocusTarget>>,
        AttachmentUiError,
    > {
        unreachable!("no dialog focus in this adapter")
    }
    fn restore_focus(
        &self,
        _: std::rc::Rc<crate::browser::attachment_content::FocusTarget>,
    ) -> Result<crate::browser::timer::Timer, AttachmentUiError> {
        unreachable!("no dialog focus in this adapter")
    }
    fn dialog_bounds(&self, _: egui::Id, _: [&str; 2]) -> Result<egui::Rect, AttachmentUiError> {
        unreachable!("no dialog measurement in this adapter")
    }
    fn native_key(&self, _: &web_sys::KeyboardEvent, _: &egui::Context) {
        unreachable!("no native dialog key in this adapter")
    }

    fn download_bytes(
        &self,
        bytes: &[u8],
        name: &str,
        mime: &str,
    ) -> Result<(), AttachmentUiError> {
        self.0
            .borrow_mut()
            .push((name.into(), mime.into(), bytes.to_vec()));
        Ok(())
    }
    fn reading_layout(&self, _: egui::Id) -> Result<ReadingLayout, AttachmentUiError> {
        unreachable!()
    }
    fn present(
        &self,
        _: AttachmentSurface<'_>,
        _: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<(), AttachmentUiError> {
        unreachable!()
    }
    fn set_retained_visibility(
        &self,
        _: egui::Id,
        _: bool,
        _: Option<egui::Id>,
    ) -> Result<(), AttachmentUiError> {
        Ok(())
    }
    fn set_overlay(&self, _: egui::Id, _: bool) -> Result<(), AttachmentUiError> {
        Ok(())
    }
    fn hover_capable(&self) -> Result<bool, AttachmentUiError> {
        unreachable!()
    }
    fn remove(&self, _: egui::Id) {
        unreachable!()
    }
    fn take_escape(&self) -> bool {
        unreachable!()
    }
    fn timer(
        &self,
        _: i32,
        _: bool,
        _: Rc<dyn Fn()>,
    ) -> Result<crate::browser::timer::Timer, AttachmentUiError> {
        unreachable!()
    }
    fn spawn_local(&self, _: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>) {
        unreachable!()
    }
    fn download(&self, _: Rc<RefCell<crate::Attachment>>) -> Result<(), AttachmentUiError> {
        unreachable!()
    }
}
/// Mouse and keyboard activation preserve the selected slice and original media type.
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn download_control_preserves_original_bytes_and_slice_bounds() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/generic_artifacts.json"
    ))
    .unwrap();
    let pack = maestro_theme::load_brand_pack(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/brand/brand.json"),
        &maestro_theme::theme::NativeThemeOperations,
    )
    .unwrap();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    let i18n = crate::I18n::new(Rc::new(Language)).unwrap();
    for (index, case) in corpus["download"].as_array().unwrap().iter().enumerate() {
        let input = &case["input"];
        let bytes: Vec<u8> = serde_json::from_value(input["bytes"].clone()).unwrap();
        let selected = match input["kind"].as_str().unwrap() {
            "bytes" => &bytes[..],
            "slice" => {
                &bytes[input["start"].as_u64().unwrap().try_into().unwrap()
                    ..input["end"].as_u64().unwrap().try_into().unwrap()]
            }
            _ => panic!("unknown recorded content"),
        };
        let host = Host::default();
        let context = egui::Context::default();
        let draw_frame = |events| {
            frame(
                &context,
                events,
                ArtifactView {
                    highlighter: &highlighter::NoHighlight,
                    i18n: &i18n,
                    presentation: &presentation,
                },
                (&host, input, selected),
            );
        };
        draw_frame(vec![]);
        for events in activation(index) {
            draw_frame(events);
        }
        let expected: Vec<u8> = serde_json::from_value(case["output"]["bytes"].clone()).unwrap();
        assert_eq!(
            host.0.borrow().as_slice(),
            &[(
                input["filename"].as_str().unwrap().into(),
                case["output"]["mime"].as_str().unwrap().into(),
                expected
            )]
        );
    }
}
/// English-only host for the byte-control title.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct Language;
#[cfg(not(target_arch = "wasm32"))]
impl crate::LanguageHost for Language {
    fn stored_language(&self) -> Result<Option<String>, crate::I18nError> {
        Ok(None)
    }
    fn browser_language(&self) -> Result<Option<String>, crate::I18nError> {
        Ok(None)
    }
    fn legacy_language(&self) -> Result<Option<String>, crate::I18nError> {
        Ok(None)
    }
    fn store_language(&self, _: &str) -> Result<(), crate::I18nError> {
        unreachable!()
    }
    fn reload(&self) -> Result<(), crate::I18nError> {
        unreachable!()
    }
    fn unknown_translation(&self, _: &crate::I18nDiagnostic) {
        unreachable!()
    }
}

/// Draw the actual control and preserve existing ghost caller geometry.
#[cfg(not(target_arch = "wasm32"))]
fn frame(
    context: &egui::Context,
    events: Vec<egui::Event>,
    view: ArtifactView<'_>,
    operands: (&Host, &serde_json::Value, &[u8]),
) {
    let (host, input, selected) = operands;
    let mut glyphs = [egui::Rect::NOTHING; 2];
    let mut output = context.run_ui(
        egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400., 300.),
            )),
            ..Default::default()
        },
        |ui| {
            let start = ui.next_widget_position();
            draw(
                ui,
                view,
                host,
                input["filename"].as_str().unwrap(),
                (input["mimeType"].as_str().unwrap(), selected),
            )
            .unwrap();
            assert_eq!(ui.min_rect().right_bottom() - start, egui::vec2(36., 32.));
            glyphs[0] =
                egui::Rect::from_min_size(start + egui::vec2(10., 8.), egui::vec2(16., 16.));
            let old = attachment_content::ghost(ui, view.presentation, "Download").unwrap();
            assert_eq!(old.rect.size(), egui::vec2(32., 32.));
            glyphs[1] = egui::Rect::from_center_size(old.rect.center(), egui::vec2(16., 16.));
        },
    );
    output.textures_delta.clear();
    for event in &output.platform_output.events {
        assert_eq!(event.widget_info().label.as_deref(), Some("Download"));
    }
    for bounds in glyphs {
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Mesh(mesh) if mesh.calc_bounds() == bounds)), "missing glyph at {bounds:?}");
    }
}

/// Keyboard focus traversal and pointer press/release activation.
#[cfg(not(target_arch = "wasm32"))]
fn activation(index: usize) -> Vec<Vec<egui::Event>> {
    if index == 1 {
        [egui::Key::Tab, egui::Key::Enter]
            .into_iter()
            .map(|key| {
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }]
            })
            .collect()
    } else {
        let pos = egui::pos2(18., 16.);
        vec![vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]]
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[path = "../../../tests/highlighter_support/mod.rs"]
pub(crate) mod highlighter;
