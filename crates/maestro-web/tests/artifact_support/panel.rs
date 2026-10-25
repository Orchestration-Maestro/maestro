//! Strict panel recordings and actual application composition.
use super::{
    Frame,
    presentation::{Presentation, ready},
};
use maestro_app::browser::tools::artifacts::{Artifact, LocalArtifactsPanel};
use maestro_web::{ArtifactsPanel, tools::artifacts::ArtifactUiError};
use serde::Deserialize;
use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, RwLock},
};
/// Complete committed shell recording.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Render-only cases.
    pub render: Vec<Case<RenderOutput>>,
    /// Selection and notification operations.
    pub operations: Vec<Case>,
    /// Reconnection cases.
    pub reconnect: Vec<Case>,
    /// Literal scroll cases.
    pub literal_scroll: Vec<ScrollCase>,
    /// Visible shadow cases.
    pub shell_shadow: Vec<ShadowCase>,
}
/// Complete source shell operands.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// Ordered map keys paired with mutable record filenames.
    pub entries: Vec<(String, String)>,
    /// Selected literal filename.
    pub active: Option<String>,
    #[serde(default)]
    /// Collapsed property.
    pub collapsed: bool,
    #[serde(default)]
    /// Full-screen property.
    pub overlay: bool,
    #[serde(default)]
    /// Whether a close notification is installed.
    pub close: bool,
    /// Operation applied before observation.
    pub operation: Option<String>,
    /// Caller language.
    pub language: String,
}
/// Ordered literal tab and its active treatment.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tab {
    /// Literal filename.
    pub filename: String,
    /// Exact rendered span.
    pub text: String,
    /// Selected literal filename.
    pub active: bool,
}
/// Painted shell observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RenderOutput {
    /// Whether the shell is hidden.
    pub hidden: bool,
    /// Ordered tab observations.
    pub tabs: Vec<Tab>,
    /// Localized close title.
    pub header_text: Option<Vec<String>>,
    /// Whether the installed selected header is evaluated.
    pub header_rendered: bool,
    /// Selected literal filename.
    pub active: Option<String>,
    #[serde(rename = "inline_border")]
    /// Whether the left border is included.
    pub inline_border: Option<bool>,
}
/// Operation and reconnection observations, with group-only results kept optional.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Output {
    /// Selected literal filename.
    pub active: Option<String>,
    /// Visible native body identities by filename.
    pub visible: Vec<String>,
    /// Host-open notification count.
    pub opened: usize,
    /// Close notification count.
    pub closed: usize,
    /// Retained root identity observation.
    pub same_roots: Option<bool>,
}
/// Complete source shell case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case<O = Output> {
    /// Complete recorded input.
    pub input: Input,
    /// Recorded output.
    pub output: O,
}
/// Literal scrolling operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScrollInput {
    /// Literal filename.
    pub filename: String,
}
/// Scroll alignment and animation policy.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScrollOptions {
    /// Scroll animation policy.
    pub behavior: String,
    /// Horizontal placement policy.
    pub inline: String,
}
/// Ordered literal scroll target.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scroll {
    /// Literal filename.
    pub filename: String,
    /// Scroll policy.
    pub options: ScrollOptions,
}
/// Selection and actual scroll outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScrollOutput {
    /// Selected literal filename.
    pub active: String,
    /// Visible native body identities by filename.
    pub visible: Vec<String>,
    /// Ordered literal targets.
    pub scroll: Vec<Scroll>,
    /// Recorded failure, when present.
    pub error: Option<String>,
}
/// Literal source-selector correction and successful neighbors.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScrollCase {
    /// Complete recorded input.
    pub input: ScrollInput,
    /// Recorded output.
    pub output: ScrollOutput,
}
/// Source color-mode input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShadowInput {
    /// Source dark-mode setting.
    pub dark: bool,
}
/// Visible shadow geometry and opacity.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shadow {
    /// Horizontal and vertical shadow offsets.
    pub offset: [f32; 2],
    /// Visible penumbra width.
    pub blur: f32,
    /// Caster expansion, including negative shrink.
    pub spread: f32,
    /// Shadow opacity.
    pub opacity: f32,
}
/// All visible source shadow layers.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shadows {
    /// Visible shadow layers.
    pub layers: Vec<Shadow>,
}
/// Source shadow observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShadowCase {
    /// Complete recorded input.
    pub input: ShadowInput,
    /// Recorded output.
    pub output: Shadows,
}
/// Parse only the consumed recording fields.
/// # Panics
/// Panics if the controlled fixture or reached operation fails.
#[must_use]
pub fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../fixtures/artifact_panel.json")).unwrap()
}
/// Retained actual tool, panel and controlled native host.
pub struct Harness {
    /// Controlled native publication boundary.
    pub host: Rc<super::super::session_header_support::Host>,
    /// Actual retained panel.
    pub panel: Rc<ArtifactsPanel>,
    /// Test-only application presentation.
    pub presentation: Rc<Presentation>,
    /// Actual local application command owner.
    pub caller: LocalArtifactsPanel,
    /// Owned drawing operands.
    pub frame: Frame,
    /// Close notification count.
    pub closed: Rc<Cell<usize>>,
}
impl Harness {
    /// Connect first while empty so fixture selection remains unrepaired.
    /// # Panics
    /// Panics if the controlled fixture or reached operation fails.
    #[must_use]
    pub fn new(input: &Input) -> Self {
        let host = Rc::new(super::super::session_header_support::Host::default());
        let panel = Rc::new(ArtifactsPanel::new(host.clone()));
        let mut presentation = Presentation::new(host.clone());
        presentation.panel = Some(panel.clone());
        let presentation = Rc::new(presentation);
        let caller = LocalArtifactsPanel::new_local(presentation.clone());
        panel.connected_callback(&caller).unwrap();
        let now = chrono::DateTime::from_timestamp_millis(0).unwrap();
        for (key, name) in &input.entries {
            caller.artifacts().write().unwrap().insert(
                key.clone(),
                Arc::new(RwLock::new(Artifact {
                    filename: name.clone(),
                    content: "YQ==".into(),
                    created_at: now,
                    updated_at: now,
                })),
            );
            if input.operation.as_deref() != Some("missing-view") {
                panel.get_or_create_artifact_element(key, "YQ==").unwrap();
            }
        }
        if let Some(name) = &input.active {
            caller.show_artifact(name);
        }
        panel.set_collapsed(input.collapsed);
        panel.set_overlay(input.overlay);
        let frame = Frame::default();
        frame
            .i18n
            .get_translations()
            .borrow_mut()
            .get_mut("en")
            .unwrap()
            .borrow_mut()
            .insert(
                "Close artifacts".into(),
                maestro_web::TranslationValue::Text(
                    if input.language == "de" {
                        "Artefakte schließen"
                    } else {
                        "Close artifacts"
                    }
                    .into(),
                ),
            );
        let closed = Rc::new(Cell::new(0));
        if input.close {
            let closed = closed.clone();
            panel.set_on_close(Some(Rc::new(move || closed.set(closed.get() + 1))));
        }
        Self {
            host,
            panel,
            presentation,
            caller,
            frame,
            closed,
        }
    }
    /// Draw only this retained composition.
    /// # Errors
    /// Returns the reached panel failure.
    pub fn draw(&self) -> Result<egui::FullOutput, ArtifactUiError> {
        self.frame.draw_panel(&self.panel, &self.caller, vec![])
    }
    /// Dispatch real pointer press and release at the supplied control.
    /// # Panics
    /// Panics if the controlled fixture or reached operation fails.
    pub fn click(&self, pos: egui::Pos2) {
        self.frame
            .draw_panel(
                &self.panel,
                &self.caller,
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
            )
            .unwrap();
    }
    /// Execute through the actual local application tool.
    /// # Panics
    /// Panics if the controlled fixture or reached operation fails.
    pub fn command(&self, command: &str, filename: &str, content: Option<&str>) {
        let mut args = serde_json::json!({"command":command,"filename":filename});
        if let Some(content) = content {
            args["content"] = content.into();
        }
        ready((self.caller.tool().execute)(
            "call".into(),
            args,
            None,
            None,
        ))
        .unwrap();
    }
    /// Find published roots whose independent policy currently shows them.
    #[must_use]
    pub fn visible(&self) -> Vec<egui::Id> {
        self.host
            .retained
            .borrow()
            .iter()
            .filter_map(|(id, (visible, _))| visible.then_some(*id))
            .collect()
    }
}
/// Actual text spans, preserving order and repeated occurrences.
#[must_use]
pub fn text(output: &egui::FullOutput) -> Vec<&egui::epaint::TextShape> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text),
            _ => None,
        })
        .collect()
}
/// Exact actual tab rectangle inferred from its public text span.
/// # Panics
/// Panics if the controlled fixture or reached operation fails.
#[must_use]
pub fn tab(output: &egui::FullOutput, name: &str) -> egui::Rect {
    let text = text(output)
        .into_iter()
        .find(|text| text.galley.job.text == name)
        .unwrap();
    text.galley.rect.translate(text.pos.to_vec2())
}

/// Actual icon geometry identifies close (zero) and download (one) controls.
/// # Panics
/// Panics if the requested icon is absent from the observed panel.
#[must_use]
pub fn control(output: &egui::FullOutput, index: usize) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh) => Some(mesh.calc_bounds().center()),
            _ => None,
        })
        .nth(1 - index)
        .unwrap()
}
