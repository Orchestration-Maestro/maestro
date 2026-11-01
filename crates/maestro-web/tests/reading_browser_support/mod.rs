//! Shared actual-document host and drawing harness for native reading views.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::support;
use maestro_web::{
    ArtifactElement, I18n,
    browser::attachment_content::*,
    tools::artifacts::{artifact_element::ArtifactView, create_artifact_element},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, prelude::*};
/// Evaluate a document observation.
/// # Panics
/// Panics when the evaluation throws.
#[must_use]
pub fn js(code: &str) -> JsValue {
    js_sys::eval(code).unwrap()
}
/// Evaluate a document statement for its effect.
/// # Panics
/// Panics when the evaluation throws.
pub fn run(code: &str) {
    let _ = js(code);
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
    pub downloads: RefCell<Vec<maestro_web::SharedAttachment>>,
}
impl Host {
    /// Bind the actual canvas document and retain per-test state.
    /// # Panics
    /// Panics when the document has no canvas or the native host cannot bind it.
    #[must_use]
    pub fn new() -> Self {
        std::panic::set_hook(Box::new(|info| {
            web_sys::console::error_1(&info.to_string().into());
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
impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}
impl AttachmentHost for Host {
    fn enter_placement_scope(
        &self,
        scope: egui::Id,
    ) -> maestro_web::browser::attachment_content::PlacementScope {
        self.native.enter_placement_scope(scope)
    }
    fn hide_placement_scope(&self, scope: egui::Id) {
        self.native.hide_placement_scope(scope);
    }

    fn copy_to_clipboard(
        &self,
        text: &str,
        mode: maestro_web::browser::attachment_content::ClipboardMode,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool>>> {
        self.native.copy_to_clipboard(text, mode)
    }

    fn set_retained_visibility(
        &self,
        id: egui::Id,
        visible: bool,
        modal: Option<egui::Id>,
    ) -> Result<(), AttachmentHostError> {
        self.native.set_retained_visibility(id, visible, modal)
    }
    fn present(
        &self,
        s: AttachmentSurface<'_>,
        p: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<(), AttachmentHostError> {
        if let Some(message) = self.failure.borrow_mut().take() {
            return Err(AttachmentHostError::Presentation(message));
        }
        self.native.present(s, p)
    }
    fn reading_layout(&self, id: egui::Id) -> Result<ReadingLayout, AttachmentHostError> {
        self.native.reading_layout(id)
    }
    fn remove(&self, id: egui::Id) {
        self.native.remove(id);
    }
    fn set_overlay(&self, id: egui::Id, visible: bool) -> Result<(), AttachmentHostError> {
        self.native.set_overlay(id, visible)
    }
    fn hover_capable(&self) -> Result<bool, AttachmentHostError> {
        self.native.hover_capable()
    }
    fn timer(
        &self,
        ms: i32,
        repeat: bool,
        callback: Rc<dyn Fn()>,
    ) -> Result<maestro_web::browser::timer::Timer, AttachmentHostError> {
        self.native.timer(ms, repeat, callback)
    }
    fn spawn_local(&self, f: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>) {
        self.native.spawn_local(f);
    }
    fn download(&self, a: maestro_web::SharedAttachment) -> Result<(), AttachmentHostError> {
        self.downloads.borrow_mut().push(a.clone());
        if self.fail_download.get() {
            return Err(AttachmentHostError::Host("save failure".into()));
        }
        self.native.download(a)
    }
    fn download_bytes(&self, b: &[u8], n: &str, m: &str) -> Result<(), AttachmentHostError> {
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
    /// Physical density for projection.
    pub density: f32,
}
impl View {
    /// Bind an installed artifact to the actual canvas document.
    /// # Panics
    /// Panics when the canvas, artifact factory, brand pack or dictionary cannot be set up.
    #[must_use]
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
            pack: support::brand::pack("brand.json").unwrap(),
            mode: maestro_theme::BrandMode::Dark,
            size: egui::vec2(400., 300.),
            density: 1.,
            i18n: I18n::new(Rc::new(support::Host::default())).unwrap(),
        }
    }
    /// Drive one public body or header input frame.
    /// # Panics
    /// Panics when the presentation or the drawn view fails.
    pub fn draw(&mut self, header: bool, events: Vec<egui::Event>) -> egui::FullOutput {
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
            if header {
                ui.horizontal(|ui| self.element.get_header_buttons(ui, v))
                    .inner
                    .unwrap();
            } else {
                self.element.render(ui, v).unwrap();
            }
        });
        output.textures_delta.clear();
        output
    }
}
pub use crate::support::pointer::click;

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
