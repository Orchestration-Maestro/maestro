//! Source-derived inputs driven through canvas activation.
#![cfg(test)]
use crate::{brand::pack, effects_support::Host};
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
    pub viewport: egui::Vec2,
    pub hovered: bool,
    pub enabled: bool,
    pub time: f64,
    pub paint: crate::canvas_paint::Paint,
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
            viewport: egui::vec2(800., 600.),
            hovered: true,
            enabled: true,
            time: 0.,
            paint: crate::canvas_paint::Paint::default(),
            covered: false,
            layout: egui::Layout::top_down(egui::Align::Min),
        }
    }
    pub fn frame(
        &mut self,
        events: Vec<egui::Event>,
    ) -> (Result<egui::Response, std::io::Error>, egui::FullOutput) {
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
    ) -> (Result<egui::Response, std::io::Error>, egui::FullOutput) {
        let pack = pack(name).unwrap();
        let presentation = pack.presentation(mode).unwrap();
        self.styled_frame(events, &presentation)
    }
    /// Draw supplied selected values through the same caller and paint recorder.
    /// # Panics
    /// Panics if the controlled frame does not return a draw result.
    pub fn styled_frame(
        &mut self,
        events: Vec<egui::Event>,
        presentation: &maestro_theme::BrandPresentation<'_>,
    ) -> (Result<egui::Response, std::io::Error>, egui::FullOutput) {
        let mut result = None;
        let context = self.context.clone();
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.viewport)),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |ui| {
                result = Some(self.draw(ui, presentation));
            },
        );
        self.paint.update(&output);
        output.textures_delta.clear();
        (result.unwrap(), output)
    }
    /// Draw with an independently configured parent, observing that it remains unchanged.
    fn draw(
        &mut self,
        ui: &mut egui::Ui,
        presentation: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<egui::Response, std::io::Error> {
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
    pub fn activate(&mut self) -> Result<egui::Response, std::io::Error> {
        self.frame(vec![]).0?;
        self.frame(pointer(self.rect.center(), true)).0?;
        self.frame(pointer(self.rect.center(), false)).0
    }
}
pub use crate::support::pointer::pointer;
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
            assert!(result.is_err());
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
/// Match the painted success glyph against both independent native producers.
#[must_use]
pub fn check(canvas: &Canvas, output: &egui::FullOutput) -> bool {
    glyph_state(canvas, output) == Some(true)
}
/// Classify visible native coverage; background-only paint has no glyph state.
fn glyph_state(canvas: &Canvas, output: &egui::FullOutput) -> Option<bool> {
    let region = canvas.rect.shrink(3.);
    let actual = canvas.paint.region(&canvas.context, &output.shapes, region);
    let background = canvas.paint.pixel(
        &canvas.context,
        &output.shapes,
        canvas.rect.center_bottom() - egui::vec2(0., 3.),
    );
    let signal: Vec<f32> = actual
        .iter()
        .map(|pixel| {
            (0..3)
                .map(|i| (f32::from(pixel[i]) - background[i]).powi(2))
                .sum::<f32>()
                .sqrt()
        })
        .collect();
    let error = |copied| {
        let (paint, shapes) = native_glyph(canvas, copied, egui::Color32::WHITE);
        let mask = paint.region(&canvas.context, &shapes, region);
        let weights: Vec<_> = mask
            .iter()
            .map(|pixel| f32::from(pixel[3]) / 255.)
            .collect();
        let scale = signal.iter().zip(&weights).map(|(a, b)| a * b).sum::<f32>()
            / weights.iter().map(|v| v * v).sum::<f32>();
        signal
            .iter()
            .zip(weights)
            .map(|(a, b)| (a - scale * b).powi(2))
            .sum::<f32>()
    };
    signal
        .iter()
        .any(|v| *v > 1.)
        .then(|| error(true) < error(false))
}
/// Prepare the original independent native glyph without composing a control.
#[must_use]
pub fn native_glyph(
    canvas: &Canvas,
    copied: bool,
    color: egui::Color32,
) -> (crate::canvas_paint::Paint, Vec<egui::epaint::ClippedShape>) {
    if copied {
        native_text(canvas, "✓", egui::FontId::proportional(16.), color)
    } else {
        let (paint, mut shapes) = crate::canvas_paint::icon(
            &canvas.context,
            egui::Rect::from_center_size(canvas.rect.center(), egui::vec2(14., 14.)),
            "Link",
            color,
        );
        shapes[0].clip_rect = canvas.clip;
        (paint, shapes)
    }
}
impl Canvas {
    /// Draw an ordinary frame and inspect its painted success glyph.
    pub fn check_frame(&mut self) -> bool {
        let output = self.frame(vec![]).1;
        check(self, &output)
    }
}
/// Return composed bounds only when the native Link glyph contributes painted pixels.
#[must_use]
pub fn link(canvas: &Canvas, output: &egui::FullOutput) -> Option<egui::Rect> {
    if glyph_state(canvas, output) != Some(false) {
        return None;
    }
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Mesh(mesh) => Some(mesh.calc_bounds()),
        _ => None,
    })
}
impl Canvas {
    /// Draw an ordinary frame and inspect its visible native Link glyph.
    pub fn link_frame(&mut self) -> Option<egui::Rect> {
        let output = self.frame(vec![]).1;
        link(self, &output)
    }
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

/// Compare a composed interior to the independent native glyph and selected roles.
/// # Panics
/// Panics if native glyph pixels or their selected colors differ.
pub fn assert_glyph(
    canvas: &Canvas,
    output: &egui::FullOutput,
    copied: bool,
    colors: [egui::Color32; 2],
    opacity: f32,
) {
    let (native, shapes) = native_glyph(canvas, copied, egui::Color32::WHITE);
    let region = canvas.rect.shrink(7.);
    let expected = native.region(&canvas.context, &shapes, region);
    let actual = canvas.paint.region(&canvas.context, &output.shapes, region);
    assert_eq!(expected.len(), actual.len());
    let background = colors[0].to_array();
    let foreground = colors[1].to_array();
    assert!(expected.iter().any(|p| p[3] > 0));
    for ((source, actual), point) in expected
        .iter()
        .zip(actual)
        .zip(crate::canvas_paint::points(&canvas.context, region))
    {
        if !canvas.clip.contains(point) {
            assert_eq!(actual, [0; 4]);
            continue;
        }
        let coverage = f32::from(source[3]) / 255.;
        for i in 0..3 {
            let expected = (f32::from(foreground[i]) * coverage
                + f32::from(background[i]) * (1. - coverage))
                * opacity;
            assert!(
                (f32::from(actual[i]) - expected).abs() <= 2.1,
                "native glyph channel differs"
            );
        }
    }
}

/// Prepare text through the original native layout and tessellation route.
#[must_use]
pub fn native_text(
    canvas: &Canvas,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
) -> (crate::canvas_paint::Paint, Vec<egui::epaint::ClippedShape>) {
    let galley = canvas
        .context
        .fonts_mut(|fonts| fonts.layout_no_wrap(text.into(), font, color));
    let pos = canvas.rect.center() - galley.size() / 2.;
    let shape = egui::epaint::TextShape::new(pos, galley, color);
    (
        crate::canvas_paint::Paint::native_text(&canvas.context),
        vec![egui::epaint::ClippedShape {
            clip_rect: canvas.clip,
            shape: egui::Shape::Text(shape),
        }],
    )
}
