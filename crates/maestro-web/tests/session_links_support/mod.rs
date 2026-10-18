//! Source-derived inputs driven through canvas activation.
#![cfg(test)]
use crate::{brand::Pack, effects_support::Host};
use maestro_theme::BrandMode;
use maestro_web::export_html::template::{CopyLinkState, CopyLinkView, render_copy_link_button};
use std::rc::Rc;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub urls: Vec<Case>,
    pub control: Control,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub name: String,
    pub input: Input,
    pub expected: Expected,
}
#[derive(Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub href: String,
    pub base: Option<String>,
    pub leaf: String,
    pub entry: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    pub outcome: String,
    pub url: Option<String>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub identities: Vec<Identity>,
    pub geometry: Geometry,
    pub title: String,
    pub paths: Vec<String>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub input: String,
    pub expected: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Geometry {
    pub width: f32,
    pub height: f32,
    pub inset: f32,
    pub padding: f32,
    pub glyph_width: f32,
    pub glyph_height: f32,
    pub transition_seconds: f32,
}
/// Read the committed control cases.
/// # Panics
/// Panics if the fixture does not match its schema.
#[must_use]
pub fn fixture() -> Fixture {
    serde_json::from_str(include_str!("../fixtures/session_links.json")).unwrap()
}
pub struct Canvas {
    pub host: Rc<Host>,
    pub state: CopyLinkState,
    pub context: egui::Context,
    pub input: Input,
    pub rect: egui::Rect,
    pub clip: egui::Rect,
    pub hovered: bool,
    pub enabled: bool,
    pub time: f64,
    pub paint: crate::canvas_paint::Paint,
    pub pixels: Vec<egui::Color32>,
    pub covered: bool,
    pub layout: egui::Layout,
}
impl Canvas {
    /// Construct an idle controlled canvas.
    #[must_use]
    pub fn new() -> Self {
        let host = Rc::new(Host::default());
        Self {
            state: CopyLinkState::new(host.clone()),
            host,
            context: egui::Context::default(),
            input: fixture().urls.remove(0).input,
            rect: egui::Rect::from_min_size(egui::pos2(40., 40.), egui::vec2(28., 28.)),
            clip: egui::Rect::EVERYTHING,
            hovered: true,
            enabled: true,
            time: 0.,
            paint: crate::canvas_paint::Paint::default(),
            pixels: vec![],
            covered: false,
            layout: egui::Layout::top_down(egui::Align::Min),
        }
    }
    pub fn frame(
        &mut self,
        events: Vec<egui::Event>,
    ) -> (
        Result<egui::Response, maestro_web::browser::attachment_content::AttachmentUiError>,
        egui::FullOutput,
    ) {
        self.pack_frame(events, "brand.json", BrandMode::Dark)
    }
    /// Draw one frame using the selected pack.
    /// # Panics
    /// Panics if the controlled brand pack or draw result is absent.
    pub fn pack_frame(
        &mut self,
        events: Vec<egui::Event>,
        name: &str,
        mode: BrandMode,
    ) -> (
        Result<egui::Response, maestro_web::browser::attachment_content::AttachmentUiError>,
        egui::FullOutput,
    ) {
        let pack = maestro_theme::load_brand_pack(name, &Pack).unwrap();
        let presentation = pack.presentation(mode).unwrap();
        let mut result = None;
        let context = self.context.clone();
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |ui| {
                result = Some(self.draw(ui, &presentation));
            },
        );
        self.pixels = output
            .textures_delta
            .set
            .values()
            .flat_map(|deltas| deltas.iter())
            .flat_map(|delta| match &delta.image {
                egui::ImageData::Color(image) => image.pixels.clone(),
            })
            .collect();
        self.paint.update(&output);
        output.textures_delta.clear();
        (result.unwrap(), output)
    }
    /// Draw with an independently configured parent, observing that it remains unchanged.
    fn draw(
        &mut self,
        ui: &mut egui::Ui,
        presentation: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<egui::Response, maestro_web::browser::attachment_content::AttachmentUiError> {
        let mut parent = ui.new_child(
            egui::UiBuilder::new()
                .layout(self.layout)
                .max_rect(ui.max_rect()),
        );
        parent.set_clip_rect(self.clip);
        if !self.enabled {
            parent.disable();
        }
        if self.covered {
            cover(&parent, self.rect);
        }
        let style = parent.style().clone();
        let position = parent.next_widget_position();
        let result = render_copy_link_button(
            &mut parent,
            &self.input.entry,
            &mut self.state,
            CopyLinkView {
                presentation,
                current_leaf_id: &self.input.leaf,
                location_href: &self.input.href,
                share_base_url: self.input.base.as_deref(),
                rect: self.rect,
                message_hovered: self.hovered,
            },
        );
        assert_eq!(parent.layout(), &self.layout);
        assert_eq!(parent.style(), &style);
        assert_eq!(parent.next_widget_position(), position);
        result
    }
    /// Deliver a complete primary gesture.
    /// # Errors
    /// Returns an error from the reached control draw.
    pub fn activate(
        &mut self,
    ) -> Result<egui::Response, maestro_web::browser::attachment_content::AttachmentUiError> {
        self.frame(vec![]).0?;
        self.frame(pointer(self.rect.center(), true)).0?;
        self.frame(pointer(self.rect.center(), false)).0
    }
}
#[must_use]
pub fn pointer(pos: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}
/// Assert URL preparation through control activation.
/// # Panics
/// Panics if a selected fixture or its output differs.
pub fn check_urls(names: &[&str]) {
    let cases: Vec<_> = fixture()
        .urls
        .into_iter()
        .filter(|case| names.contains(&case.name.as_str()))
        .collect();
    assert_eq!(cases.len(), names.len());
    for case in cases {
        let mut canvas = Canvas::new();
        canvas.input = case.input;
        canvas.frame(vec![]).0.unwrap();
        assert!(canvas.host.copies.borrow().is_empty());
        let result = canvas.activate();
        if case.expected.outcome == "invalid-location" {
            assert!(matches!(
                result,
                Err(maestro_web::browser::attachment_content::AttachmentUiError::Presentation(_))
            ));
            assert!(case.expected.url.is_none());
            assert!(canvas.host.copies.borrow().is_empty());
            assert_eq!(canvas.host.admissions.get(), 0);
        } else {
            assert_eq!(case.expected.outcome, "success");
            result.unwrap();
            let copies = canvas.host.copies.borrow();
            assert_eq!(
                copies.as_slice(),
                &[case.expected.url.unwrap()],
                "{}",
                case.name
            );
            if let Ok(url) = url::Url::parse(&copies[0]) {
                let pairs: Vec<_> = url.query_pairs().collect();
                assert_eq!(
                    pairs.iter().find(|(k, _)| k == "leafId").unwrap().1,
                    canvas.input.leaf
                );
                assert_eq!(
                    pairs.iter().find(|(k, _)| k == "targetId").unwrap().1,
                    canvas.input.entry
                );
            }
        }
    }
}
/// Finish one particular pending operation, retaining pending siblings.
pub fn settle(canvas: &Canvas, index: usize, success: bool) {
    canvas.host.results.borrow()[index].set(Some(success));
    canvas.host.run_work();
}
/// Read the painted check indication from this retained control.
#[must_use]
pub fn check(output: &egui::FullOutput) -> bool {
    output
        .shapes
        .iter()
        .any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text()=="✓"))
}
/// Read the actual painted catalog glyph bounds.
#[must_use]
pub fn link(output: &egui::FullOutput) -> Option<egui::Rect> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Mesh(mesh) => Some(mesh.calc_bounds()),
        _ => None,
    })
}

/// Place a real input-owning foreground neighbor.
fn cover(ui: &egui::Ui, rect: egui::Rect) {
    egui::Area::new(egui::Id::new("cover"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ui.ctx(), |ui| {
            ui.allocate_response(rect.size(), egui::Sense::click());
        });
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}
