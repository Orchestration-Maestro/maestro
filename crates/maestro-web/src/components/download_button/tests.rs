//! Byte control activation and geometry.
#[cfg(not(target_arch = "wasm32"))]
use super::*;
#[cfg(not(target_arch = "wasm32"))]
use crate::browser::attachment_content::AttachmentHostError;
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

    fn download_bytes(
        &self,
        bytes: &[u8],
        name: &str,
        mime: &str,
    ) -> Result<(), AttachmentHostError> {
        self.0
            .borrow_mut()
            .push((name.into(), mime.into(), bytes.to_vec()));
        Ok(())
    }
    fn reading_layout(&self, _: egui::Id) -> Result<ReadingLayout, AttachmentHostError> {
        unreachable!()
    }
    fn present(
        &self,
        _: AttachmentSurface<'_>,
        _: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<(), AttachmentHostError> {
        unreachable!()
    }
    fn set_retained_visibility(
        &self,
        _: egui::Id,
        _: bool,
        _: Option<egui::Id>,
    ) -> Result<(), AttachmentHostError> {
        Ok(())
    }
    fn set_overlay(&self, _: egui::Id, _: bool) -> Result<(), AttachmentHostError> {
        Ok(())
    }
    fn hover_capable(&self) -> Result<bool, AttachmentHostError> {
        unreachable!()
    }
    fn remove(&self, _: egui::Id) {
        unreachable!()
    }
    fn timer(
        &self,
        _: i32,
        _: bool,
        _: Rc<dyn Fn()>,
    ) -> Result<crate::browser::timer::Timer, AttachmentHostError> {
        unreachable!()
    }
    fn spawn_local(&self, _: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>) {
        unreachable!()
    }
    fn download(&self, _: crate::SharedAttachment) -> Result<(), AttachmentHostError> {
        unreachable!()
    }
}
/// Native byte-control recording.
#[cfg(not(target_arch = "wasm32"))]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Selected byte-control cases.
    download: Vec<DownloadCase>,
}
/// Selected byte input with optional slice boundaries.
#[cfg(not(target_arch = "wasm32"))]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DownloadInput {
    /// Recorded byte or slice admission.
    pub kind: String,
    /// Original byte values.
    pub bytes: Vec<u8>,
    /// Inclusive slice start when present.
    pub start: Option<usize>,
    /// Exclusive slice end when present.
    pub end: Option<usize>,
    /// Supplied MIME type.
    pub mime_type: String,
    /// Literal authored filename.
    pub filename: String,
}
/// Selected download output.
#[cfg(not(target_arch = "wasm32"))]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownloadOutput {
    /// Original byte values.
    pub bytes: Vec<u8>,
    /// Expected Blob MIME type.
    pub mime: String,
}
/// Byte-control case.
#[cfg(not(target_arch = "wasm32"))]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownloadCase {
    /// Recorded operation operands.
    pub input: DownloadInput,
    /// Recorded observable result.
    pub output: DownloadOutput,
}
/// Decode native byte cases and reject malformed fixture records.
#[cfg(not(target_arch = "wasm32"))]
fn download_corpus() -> Corpus {
    let valid: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/generic_artifacts_native.json"
    ))
    .unwrap();
    for value in [serde_json::Value::Null, serde_json::json!([])] {
        let mut sibling = valid.clone();
        sibling["classifier"] = value.clone();
        assert!(serde_json::from_value::<Corpus>(sibling).is_err());
        let mut wrong = valid["download"][0]["input"].clone();
        wrong["kind"] = value;
        assert!(serde_json::from_value::<DownloadInput>(wrong).is_err());
    }
    let rejects: [fn(serde_json::Value) -> bool; 4] = [
        |value| serde_json::from_value::<Corpus>(value).is_err(),
        |value| serde_json::from_value::<DownloadCase>(value).is_err(),
        |value| serde_json::from_value::<DownloadInput>(value).is_err(),
        |value| serde_json::from_value::<DownloadOutput>(value).is_err(),
    ];
    let records: [(_, &[&str]); 4] = [
        (&valid, &["download"]),
        (&valid["download"][0], &["input", "output"]),
        (
            &valid["download"][0]["input"],
            &["kind", "bytes", "mimeType", "filename"],
        ),
        (&valid["download"][0]["output"], &["bytes", "mime"]),
    ];
    for ((record, required), rejects) in records.into_iter().zip(rejects) {
        assert!(!rejects(record.clone()));
        for value in [serde_json::Value::Null, serde_json::json!([])] {
            let mut extra = record.clone();
            extra["unexpected"] = value;
            assert!(rejects(extra));
        }
        for field in required {
            let mut missing = record.clone();
            missing
                .as_object_mut()
                .unwrap()
                .shift_remove(*field)
                .unwrap();
            assert!(rejects(missing), "missing {field}");
            let wrong = if record[field].is_array() || record[field].is_object() {
                serde_json::json!(false)
            } else {
                serde_json::json!([])
            };
            for value in [serde_json::Value::Null, wrong] {
                let mut invalid = record.clone();
                invalid[field] = value;
                assert!(rejects(invalid), "invalid {field}");
            }
        }
    }
    serde_json::from_value(valid).unwrap()
}
/// Mouse and keyboard activation preserve the selected slice and original media type.
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn download_control_preserves_original_bytes_and_slice_bounds() {
    let corpus = download_corpus();
    let pack = maestro_theme::load_brand_pack(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/brand/brand.json"
    ))
    .unwrap();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    let i18n = crate::I18n::new(Rc::new(Language)).unwrap();
    assert_eq!(corpus.download.len(), 5);
    for (index, case) in corpus.download.into_iter().enumerate() {
        let input = &case.input;
        let selected = match input.kind.as_str() {
            "bytes" => &input.bytes[..],
            "slice" => &input.bytes[input.start.unwrap()..input.end.unwrap()],
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
        assert_eq!(
            host.0.borrow().as_slice(),
            &[(input.filename.clone(), case.output.mime, case.output.bytes)]
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
    operands: (&Host, &DownloadInput, &[u8]),
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
                &input.filename,
                (input.mime_type.as_str(), selected),
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
