//! Shared actual-document host and drawing harness for native reading views.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::support;
use maestro_web::{
    ArtifactElement, AttachmentOverlay, I18n,
    browser::attachment_content::*,
    tools::artifacts::{artifact_element::ArtifactView, create_artifact_element},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, prelude::*};
/// Evaluate a document observation.
pub fn js(code: &str) -> JsValue {
    js_sys::eval(code).unwrap()
}
/// Real host with a one-shot error at the public presentation boundary.
pub struct Host {
    /// Real native platform owner.
    pub native: BrowserAttachmentHost,
    /// One-shot failure at the same presentation boundary as predecessor tests.
    pub failure: RefCell<Option<String>>,
    /// Controlled save failure.
    pub fail_download: Cell<bool>,
    /// Original record handles reaching save.
    pub downloads: RefCell<Vec<Rc<RefCell<maestro_web::Attachment>>>>,
}
impl Host {
    /// Bind the actual canvas document and retain per-test state.
    pub fn new() -> Self {
        std::panic::set_hook(Box::new(|info| {
            web_sys::console::error_1(&info.to_string().into())
        }));
        let canvas = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id("canvas")
            .unwrap()
            .dyn_into()
            .unwrap();
        Self {
            native: BrowserAttachmentHost::new(canvas).unwrap(),
            failure: RefCell::default(),
            fail_download: Cell::new(false),
            downloads: RefCell::default(),
        }
    }
}
impl AttachmentHost for Host {
    fn copy_to_clipboard(
        &self,
        text: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool>>> {
        self.native.copy_to_clipboard(text)
    }
    fn capture_focus(&self) -> Result<Option<Rc<FocusTarget>>, AttachmentUiError> {
        self.native.capture_focus()
    }
    fn restore_focus(
        &self,
        target: Rc<FocusTarget>,
    ) -> Result<maestro_web::browser::timer::Timer, AttachmentUiError> {
        self.native.restore_focus(target)
    }
    fn dialog_bounds(
        &self,
        id: egui::Id,
        dimensions: [&str; 2],
    ) -> Result<egui::Rect, AttachmentUiError> {
        self.native.dialog_bounds(id, dimensions)
    }
    fn native_key(&self, event: &web_sys::KeyboardEvent, context: &egui::Context) {
        self.native.native_key(event, context);
    }

    fn set_retained_visibility(
        &self,
        id: egui::Id,
        visible: bool,
        modal: Option<egui::Id>,
    ) -> Result<(), AttachmentUiError> {
        self.native.set_retained_visibility(id, visible, modal)
    }
    fn present(
        &self,
        s: AttachmentSurface<'_>,
        p: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<(), AttachmentUiError> {
        if let Some(message) = self.failure.borrow_mut().take() {
            return Err(AttachmentUiError::Presentation(message));
        }
        self.native.present(s, p)
    }
    fn reading_layout(&self, id: egui::Id) -> Result<ReadingLayout, AttachmentUiError> {
        self.native.reading_layout(id)
    }
    fn remove(&self, id: egui::Id) {
        self.native.remove(id);
    }
    fn set_overlay(&self, id: egui::Id, visible: bool) -> Result<(), AttachmentUiError> {
        self.native.set_overlay(id, visible)
    }
    fn hover_capable(&self) -> Result<bool, AttachmentUiError> {
        self.native.hover_capable()
    }
    fn take_escape(&self) -> bool {
        self.native.take_escape()
    }
    fn timer(
        &self,
        ms: i32,
        repeat: bool,
        callback: Rc<dyn Fn()>,
    ) -> Result<maestro_web::browser::timer::Timer, AttachmentUiError> {
        self.native.timer(ms, repeat, callback)
    }
    fn spawn_local(&self, f: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>) {
        self.native.spawn_local(f);
    }
    fn download(&self, a: Rc<RefCell<maestro_web::Attachment>>) -> Result<(), AttachmentUiError> {
        self.downloads.borrow_mut().push(a.clone());
        if self.fail_download.get() {
            return Err(AttachmentUiError::Host("save failure".into()));
        }
        self.native.download(a)
    }
    fn download_bytes(&self, b: &[u8], n: &str, m: &str) -> Result<(), AttachmentUiError> {
        self.native.download_bytes(b, n, m)
    }
}
/// Caller-owned native view retained across explicit input frames.
pub struct View {
    /// Native grammar owner loaded once per caller.
    pub highlighter: maestro_theme::SyntectHighlighter,
    /// Real artifact view.
    pub element: Box<dyn ArtifactElement>,
    /// Real platform adapter with controlled failures.
    pub host: Rc<Host>,
    /// Explicit input-frame owner.
    pub context: egui::Context,
    /// Supplied caller pack.
    pub pack: maestro_theme::BrandPack,
    /// Supplied caller mode.
    pub mode: maestro_theme::BrandMode,
    /// Live caller dictionary.
    pub i18n: I18n,
    /// Body viewport in canvas points.
    pub size: egui::Vec2,
    /// Overlay screen geometry independent of the artifact body.
    pub overlay_size: egui::Vec2,
    /// Physical density for projection.
    pub density: f32,
}
impl View {
    /// Bind an installed artifact to the actual canvas document.
    pub fn new(filename: &str, content: &str) -> Self {
        let host = Rc::new(Host::new());
        Self {
            highlighter: match maestro_theme::SyntectHighlighter::new() {
                Ok(engine) => engine,
                Err(_) => unreachable!("invalid embedded grammar table"),
            },
            element: create_artifact_element(filename, content, host.clone()).unwrap(),
            host,
            context: egui::Context::default(),
            pack: maestro_theme::load_brand_pack("brand.json", &support::brand::Pack).unwrap(),
            mode: maestro_theme::BrandMode::Dark,
            size: egui::vec2(400., 300.),
            density: 1.,
            overlay_size: egui::vec2(800., 600.),
            i18n: I18n::new(Rc::new(support::Host::default())).unwrap(),
        }
    }
    /// Drive one public body or header input frame.
    pub fn draw(&mut self, header: bool, events: Vec<egui::Event>) -> egui::FullOutput {
        self.frame(events, |ui, element, v| {
            if header {
                ui.horizontal(|ui| element.get_header_buttons(ui, v))
                    .inner
                    .unwrap();
            } else {
                element.render(ui, v).unwrap();
            }
        })
    }
    /// Drive one input frame in which the caller places the artifact inside the root.
    /// # Panics
    /// Panics when the pack has no presentation for the selected mode.
    pub fn frame(
        &mut self,
        events: Vec<egui::Event>,
        mut place: impl FnMut(&mut egui::Ui, &mut dyn ArtifactElement, ArtifactView<'_>),
    ) -> egui::FullOutput {
        let p = self.pack.presentation(self.mode).unwrap();
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            events,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(self.density);
        let mut output = self.context.run_ui(input, |ui| {
            let v = ArtifactView {
                highlighter: &self.highlighter,
                i18n: &self.i18n,
                presentation: &p,
            };
            place(ui, self.element.as_mut(), v);
        });
        output.textures_delta.clear();
        output
    }
    /// Draw the public overlay and retain its actual control positions.
    pub fn overlay(&self, overlay: &mut AttachmentOverlay, events: Vec<egui::Event>) {
        self.try_overlay(overlay, events).unwrap();
    }
    /// Preserve a reached failure without skipping end-of-frame cleanup.
    pub fn try_overlay(
        &self,
        overlay: &mut AttachmentOverlay,
        events: Vec<egui::Event>,
    ) -> Result<bool, AttachmentUiError> {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                self.overlay_size,
            )),
            events,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(self.density);
        self.context.begin_pass(input);
        let result = overlay.render(
            &self.context,
            &self.i18n,
            &self.pack.presentation(self.mode).unwrap(),
        );
        let mut out = self.context.end_pass();
        out.textures_delta.clear();
        // Save actual control positions from the painter, never native text echoes.
        let labels: Vec<_> = out
            .shapes
            .iter()
            .filter_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    Some((
                        t.galley.job.text.clone(),
                        t.pos.x,
                        t.pos.y,
                        t.galley.size().x,
                        t.galley.size().y,
                    ))
                } else {
                    None
                }
            })
            .collect();
        js_sys::Reflect::set(
            &js_sys::global(),
            &"labels".into(),
            &js_sys::JSON::parse(&serde_json::to_string(&labels).unwrap()).unwrap(),
        )
        .unwrap();
        result
    }
    /// Click the canvas-owned mode at its painted label.
    pub fn toggle(&self, overlay: &mut AttachmentOverlay, label: &str) {
        let position = js(&format!(
            "(()=>{{const l=labels.find(l=>l[0]==={});return [l[1]+l[3]/2,l[2]+l[4]/2]}})()",
            serde_json::to_string(label).unwrap()
        ));
        let array = js_sys::Array::from(&position);
        self.overlay(
            overlay,
            click(egui::pos2(
                num_traits::cast(array.get(0).as_f64().unwrap()).unwrap(),
                num_traits::cast(array.get(1).as_f64().unwrap()).unwrap(),
            )),
        );
    }
}
/// A complete pointer activation.
pub fn click(pos: egui::Pos2) -> Vec<egui::Event> {
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
    ]
}

/// Require one actual document observation.
/// # Panics
/// Panics when the observation is not true.
pub fn check(code: &str) {
    assert!(js(code).as_bool().unwrap(), "{code}");
}
/// Quote literal text at the document boundary.
/// # Panics
/// Panics if string serialization fails.
#[must_use]
pub fn quoted(text: &str) -> String {
    serde_json::to_string(text).unwrap()
}
/// Await an already-created native promise.
/// # Panics
/// Panics if the native promise rejects.
pub async fn event(promise: JsValue) {
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
        .await
        .unwrap();
}
