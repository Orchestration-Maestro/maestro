//! Drawing and strict source-derived artifact input records.
#![cfg(test)]
use maestro_web::{ArtifactElement, tools::artifacts::artifact_element::ArtifactView};
use serde::Deserialize;
/// Complete source-run fixture, consumed by disjoint behavior tests.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Application classifier cases.
    pub classifier: Vec<NameCase>,
    /// Download media types.
    /// Expected Blob MIME type.
    pub mime: Vec<NameCase>,
    /// Original payload decoding.
    pub decode: Vec<DecodeCase>,
    /// Byte control cases.
    pub download: Vec<DownloadCase>,
}
/// Literal filename input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameInput {
    /// Literal authored filename.
    pub filename: String,
}
/// Filename-selected output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameCase {
    /// Recorded operation operands.
    pub input: NameInput,
    /// Recorded observable result.
    pub output: String,
}
/// Original content input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecodeInput {
    /// Original encoded content.
    pub content: String,
}
/// Success bytes or invalid-data observation.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Decoded {
    /// Successful original bytes.
    Bytes {
        /// Complete decoded payload.
        bytes: Vec<u8>,
    },
    /// Source invalid-data category.
    Error {
        /// Recorded source failure category.
        error: String,
    },
}
/// Payload decoding case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecodeCase {
    /// Recorded operation operands.
    pub input: DecodeInput,
    /// Recorded observable result.
    pub output: Decoded,
}
/// Selected byte input with optional slice boundaries.
#[derive(Deserialize)]
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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownloadOutput {
    /// Original byte values.
    pub bytes: Vec<u8>,
    /// Expected Blob MIME type.
    pub mime: String,
}
/// Byte-control case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownloadCase {
    /// Recorded operation operands.
    pub input: DownloadInput,
    /// Recorded observable result.
    pub output: DownloadOutput,
}
/// Parse the committed recording with strict fields.
/// # Panics
/// Panics if the committed recording is invalid.
#[must_use]
pub fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../fixtures/generic_artifacts.json")).unwrap()
}
/// Owned native drawing operands.
pub struct Frame {
    /// Retained drawing context.
    pub context: egui::Context,
    /// Logical drawing viewport.
    pub size: egui::Vec2,
    /// Supplied brand pack.
    pub pack: maestro_theme::BrandPack,
    /// Selected presentation mode.
    pub mode: maestro_theme::BrandMode,
    /// Supplied language operations.
    pub i18n: maestro_web::I18n,
    /// Viewport dimensions supplied to the next frame.
    pub size: egui::Vec2,
}
impl Default for Frame {
    fn default() -> Self {
        Self {
            context: egui::Context::default(),
            size: egui::vec2(400., 300.),
            pack: super::session_header_support::pack(),
            mode: maestro_theme::BrandMode::Dark,
            i18n: maestro_web::I18n::new(std::rc::Rc::new(super::support::Host::default()))
                .unwrap(),
        }
    }
}
impl Frame {
    /// Draw a real element, optionally routing pointer activation to its header.
    /// # Errors
    /// Returns the reached view failure.
    /// # Panics
    /// Panics if the controlled pack is invalid.
    pub fn draw(
        &self,
        view: &mut dyn ArtifactElement,
        header: bool,
        events: Vec<egui::Event>,
    ) -> Result<egui::FullOutput, maestro_web::browser::attachment_content::AttachmentUiError> {
        let presentation = self.pack.presentation(self.mode).unwrap();
        let mut result = Ok(());
        let mut output = self.context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                events,
                ..Default::default()
            },
            |ui| {
                let operands = ArtifactView {
                    i18n: &self.i18n,
                    presentation: &presentation,
                };
                result = if header {
                    view.get_header_buttons(ui, operands)
                } else {
                    view.render(ui, operands)
                };
            },
        );
        output.textures_delta.clear();
        result.map(|()| output)
    }
    /// Activate the actual header using pointer press and release.
    /// # Errors
    /// Returns decoding, drawing or platform failures.
    pub fn click(
        &self,
        view: &mut dyn ArtifactElement,
    ) -> Result<(), maestro_web::browser::attachment_content::AttachmentUiError> {
        self.draw(view, true, vec![])?;
        let pos = egui::pos2(18., 16.);
        self.draw(
            view,
            true,
            vec![
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
            ],
        )?;
        Ok(())
    }
}

pub mod presentation;

impl Frame {
    /// Draw a panel with the actual application caller and supplied pointer events.
    /// # Errors
    /// Propagates the reached panel failure.
    /// # Panics
    /// Panics for an invalid controlled pack.
    pub fn draw_panel(
        &self,
        panel: &maestro_web::ArtifactsPanel,
        caller: &maestro_app::browser::tools::artifacts::LocalArtifactsPanel,
        events: Vec<egui::Event>,
    ) -> Result<egui::FullOutput, maestro_web::browser::attachment_content::AttachmentUiError> {
        let presentation = self.pack.presentation(self.mode).unwrap();
        let mut result = Ok(());
        let mut output = self.context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                events,
                ..Default::default()
            },
            |ui| {
                result = panel.render(
                    ui,
                    caller,
                    ArtifactView {
                        i18n: &self.i18n,
                        presentation: &presentation,
                    },
                );
            },
        );
        output.textures_delta.clear();
        result.map(|()| output)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod panel;
