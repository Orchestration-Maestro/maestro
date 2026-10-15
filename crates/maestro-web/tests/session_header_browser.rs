//! Browser-executed assertions through the public native header boundary.
#![cfg(all(test, target_arch = "wasm32"))]
use base64::Engine as _;
use maestro_theme::{BrandMode, ThemeInfo, ThemeOperations};
use maestro_web::browser::attachment_content::BrowserAttachmentHost;
use maestro_web::export_html::template::{
    HeaderState, HeaderView, SessionDocument, load_document, render_header,
};
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, prelude::*};

/// Supply only the checked-in brand pack, with no browser filesystem access.
struct Pack;
impl ThemeOperations for Pack {
    fn read_to_string(&self, _: &str) -> std::io::Result<String> {
        Ok(include_str!("../../../assets/brand/brand.json").into())
    }
    fn environment(&self, _: &str) -> Option<String> {
        unreachable!()
    }
    fn exists(&self, _: &str) -> bool {
        unreachable!()
    }
    fn read_dir(&self, _: &str) -> std::io::Result<Vec<String>> {
        unreachable!()
    }
    fn sort_by_name(&self, _: &mut [ThemeInfo]) -> std::io::Result<()> {
        unreachable!()
    }
}
/// Real native owner and canvas caller retained across repaints.
struct Header {
    state: HeaderState,
    pack: maestro_theme::BrandPack,
    document: SessionDocument,
    context: egui::Context,
}
impl Header {
    /// Bind to the browser runner's supplied canvas.
    fn new() -> Self {
        std::panic::set_hook(Box::new(|info| {
            web_sys::console::error_1(&info.to_string().into())
        }));
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas: web_sys::HtmlCanvasElement = document
            .get_element_by_id("canvas")
            .unwrap()
            .dyn_into()
            .unwrap();
        Self {
            state: HeaderState::new(Rc::new(BrowserAttachmentHost::new(canvas).unwrap())),
            pack: maestro_theme::load_brand_pack("brand.json", &Pack).unwrap(),
            document: load_document(
                &base64::engine::general_purpose::STANDARD
                    .encode(r#"{"entries":[],"header":{"id":"roomy"}}"#),
                "",
                None,
            )
            .unwrap(),
            context: egui::Context::default(),
        }
    }
    /// Draw with no caller spacing, returning the actual following widget top.
    fn draw(&mut self, size: egui::Vec2) -> f32 {
        let presentation = self.pack.presentation(BrandMode::Dark).unwrap();
        let mut following = 0.;
        let mut output = self.context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                ..Default::default()
            },
            |ui| {
                ui.set_max_size(size);
                ui.spacing_mut().item_spacing.y = 0.;
                render_header(
                    ui,
                    &self.document,
                    &mut self.state,
                    HeaderView {
                        presentation: &presentation,
                    },
                )
                .unwrap();
                following = ui.allocate_space(egui::vec2(10., 10.)).1.top();
            },
        );
        output.textures_delta.clear();
        following
    }
}
/// Locate the live painted panel, not a reconstructed DOM sample.
fn panel() -> web_sys::Element {
    web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .query_selector(".maestro-session-header")
        .unwrap()
        .unwrap()
}
/// A roomy native header allocates its transparent tail before the next widget.
#[wasm_bindgen]
pub fn header_trailing_spacing_precedes_following_widget() {
    let mut header = Header::new();
    let following = header.draw(egui::vec2(800., 600.));
    let bottom = panel().get_bounding_client_rect().bottom();
    let presentation = header.pack.presentation(BrandMode::Dark).unwrap();
    assert!(
        (f64::from(following) - bottom - presentation.spacing["lg"]).abs() < 0.1,
        "following widget at {following}, panel bottom {bottom}; trailing spacing must be allocated exactly once"
    );
}
/// Fractional native offsets on both axes survive the repaint's measurement.
#[wasm_bindgen]
pub fn header_fractional_scroll_survives_repaint() {
    let mut header = Header::new();
    let size = egui::vec2(300., 40.);
    header.document = load_document(
        &base64::engine::general_purpose::STANDARD.encode(format!(
            r#"{{"entries":[],"header":{{"id":"{}"}}}}"#,
            "wide".repeat(200)
        )),
        "",
        None,
    )
    .unwrap();
    header.draw(size);
    let wrapper = panel().parent_element().unwrap().parent_element().unwrap();
    let read = |key: &str| {
        js_sys::Reflect::get(&wrapper, &key.into())
            .unwrap()
            .as_f64()
            .unwrap()
    };
    js_sys::Reflect::set(&wrapper, &"scrollLeft".into(), &17.6.into()).unwrap();
    js_sys::Reflect::set(&wrapper, &"scrollTop".into(), &25.6.into()).unwrap();
    let before = (read("scrollLeft"), read("scrollTop"));
    assert!(
        before.0.fract() != 0. && before.1.fract() != 0.,
        "browser must supply fractional offsets: {before:?}"
    );
    header.context.request_repaint();
    header.draw(size);
    let after = (read("scrollLeft"), read("scrollTop"));
    assert_eq!(
        after, before,
        "repaint must preserve both fractional native offsets"
    );
}
/// The public loader's absent header reaches the literal native fallback.
#[wasm_bindgen]
pub fn header_absent_from_loaded_document_shows_unknown() {
    let mut header = Header::new();
    header.document = load_document(
        &base64::engine::general_purpose::STANDARD.encode(r#"{"entries":[]}"#),
        "",
        None,
    )
    .unwrap();
    header.draw(egui::vec2(800., 600.));
    assert_eq!(
        panel()
            .query_selector("h1")
            .unwrap()
            .unwrap()
            .text_content()
            .unwrap(),
        "Session: unknown"
    );
}
