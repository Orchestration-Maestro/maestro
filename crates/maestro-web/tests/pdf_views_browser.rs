//! Real browser PDF pages, native errors and surface transitions.
#![cfg(all(test, target_arch = "wasm32"))]
pub mod support;
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
/// Evaluate document observations without shipping test bindings.
fn js(code: &str) -> JsValue {
    js_sys::eval(code).unwrap()
}
/// Original complete fixture bytes.
fn bytes(multi: bool) -> &'static [u8] {
    if multi {
        include_bytes!("../../maestro-app/tests/fixtures/pdf/synthetic-multipage.pdf")
    } else {
        include_bytes!("../../maestro-app/tests/fixtures/pdf/synthetic-tall.pdf")
    }
}
/// Original encoded bytes for either public consumer.
fn encoded(multi: bool) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes(multi))
}
/// Real host with a one-shot error at the public presentation boundary.
struct Host {
    /// Real native platform owner.
    native: BrowserAttachmentHost,
    /// One-shot failure at the same presentation boundary as predecessor tests.
    failure: RefCell<Option<String>>,
    /// Controlled save failure.
    fail_download: Cell<bool>,
    /// Original record handles reaching save.
    downloads: RefCell<Vec<Rc<RefCell<maestro_web::Attachment>>>>,
}
impl Host {
    /// Bind the actual canvas document and retain per-test state.
    fn new() -> Self {
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
struct View {
    /// Real artifact view.
    element: Box<dyn ArtifactElement>,
    /// Real platform adapter with controlled failures.
    host: Rc<Host>,
    /// Explicit input-frame owner.
    context: egui::Context,
    /// Supplied caller pack.
    pack: maestro_theme::BrandPack,
    /// Supplied caller mode.
    mode: maestro_theme::BrandMode,
    /// Live caller dictionary.
    i18n: I18n,
    /// Body viewport in canvas points.
    size: egui::Vec2,
    /// Physical density for projection.
    density: f32,
}
impl View {
    /// Bind the actual canvas document and retain per-test state.
    fn new() -> Self {
        let host = Rc::new(Host::new());
        Self {
            element: create_artifact_element("current.pdf", &encoded(true), host.clone()).unwrap(),
            host,
            context: egui::Context::default(),
            pack: maestro_theme::load_brand_pack("brand.json", &support::brand::Pack).unwrap(),
            mode: maestro_theme::BrandMode::Dark,
            size: egui::vec2(400., 300.),
            density: 1.,
            i18n: I18n::new(Rc::new(support::Host::default())).unwrap(),
        }
    }
    /// Drive one public body or header input frame.
    fn draw(&mut self, header: bool, events: Vec<egui::Event>) {
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
                i18n: &self.i18n,
                presentation: &p,
            };
            if header {
                self.element.get_header_buttons(ui, v).unwrap();
            } else {
                self.element.render(ui, v).unwrap();
            }
        });
        output.textures_delta.clear();
    }
    /// Draw the public overlay and retain its actual control positions.
    fn overlay(&self, overlay: &mut AttachmentOverlay, events: Vec<egui::Event>) {
        self.try_overlay(overlay, events).unwrap();
    }
    /// Preserve a reached failure without skipping end-of-frame cleanup.
    fn try_overlay(
        &self,
        overlay: &mut AttachmentOverlay,
        events: Vec<egui::Event>,
    ) -> Result<bool, AttachmentUiError> {
        self.context.begin_pass(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800., 600.),
            )),
            events,
            ..Default::default()
        });
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
    fn toggle(&self, overlay: &mut AttachmentOverlay, label: &str) {
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
fn click(pos: egui::Pos2) -> Vec<egui::Event> {
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
/// Original aliased PDF attachment.
fn record() -> Rc<RefCell<maestro_web::Attachment>> {
    Rc::new(RefCell::new(maestro_web::Attachment {
        id: "same".into(),
        r#type: maestro_app::browser::utils::attachment_utils::AttachmentType::Document,
        file_name: "original.pdf".into(),
        mime_type: "application/pdf".into(),
        size: bytes(true).len(),
        content: encoded(true),
        extracted_text: Some("literal text".into()),
        preview: None,
    }))
}
/// Assert a native document observation.
fn check(code: &str) {
    assert!(
        js(code).as_bool().unwrap(),
        "native document assertion failed: {code}"
    );
}

/// All native pages retain original order, raster dimensions and bounded geometry.
#[wasm_bindgen]
pub fn pdf_native_pages_keep_order_geometry_and_separators() {
    let mut view = View::new();
    for count in 1..=3 {
        view.draw(false, vec![]);
        assert_eq!(
            js("document.querySelectorAll('.maestro-pdf img').length")
                .as_f64()
                .unwrap(),
            f64::from(count)
        );
    }
    check(
        "(()=>{const r=document.querySelector('.maestro-pdf');const p=[...r.querySelectorAll('img')];return r.querySelectorAll('hr').length===2&&p.every(p=>p.width>0&&p.getAttribute('width')==='300'&&p.getAttribute('height')==='150')&&p[0].src!==p[1].src&&p[0].src===p[2].src&&getComputedStyle(r).padding==='16px'&&r.lastElementChild.lastElementChild.tagName==='IMG'&&getComputedStyle(p[0]).backgroundColor==='rgb(255, 255, 255)'&&getComputedStyle(p[0]).boxShadow!=='none'&&getComputedStyle(p[0]).borderRadius!=='0px'&&r.parentElement.scrollHeight>r.parentElement.clientHeight})()",
    );
    js(
        "globalThis.pages=[...document.querySelectorAll('.maestro-pdf img')].map(p=>p.src);globalThis.root=document.querySelector('.maestro-pdf');",
    );
    view.draw(false, vec![]);
    check(
        "root===document.querySelector('.maestro-pdf')&&pages.every((p,i)=>p===root.querySelectorAll('img')[i].src)",
    );
    js(
        "globalThis.savedWrapper=root.parentElement;savedWrapper.scrollTop=41.5;globalThis.savedScroll=savedWrapper.scrollTop;document.querySelector('#canvas').style.width='600px';document.querySelector('#canvas').style.height='300px';",
    );
    view.density = 1.25;
    view.draw(false, vec![]);
    check(
        "root===document.querySelector('.maestro-pdf')&&savedWrapper.scrollTop===savedScroll&&Math.abs(savedWrapper.getBoundingClientRect().width-375)<1&&Math.abs(savedWrapper.getBoundingClientRect().height-187.5)<1",
    );
    js(
        "document.querySelector('#canvas').style.width='800px';document.querySelector('#canvas').style.height='600px';",
    );
    view.density = 1.;
    for width in [2., 0.] {
        view.size = egui::vec2(width, 300.);
        view.draw(false, vec![]);
        check(
            "document.querySelector('.maestro-pdf').parentElement.getBoundingClientRect().width<=2",
        );
    }
    view.size = egui::vec2(400., 300.);
    let generic = create_artifact_element("a.zip", "", view.host.clone()).unwrap();
    drop(std::mem::replace(&mut view.element, generic));
    let mut overlay = AttachmentOverlay::open(record(), None, view.host.clone());
    for _ in 0..3 {
        view.overlay(&mut overlay, vec![]);
    }
    check(
        "(()=>{const r=document.querySelector('.maestro-pdf');return getComputedStyle(r).padding==='0px'&&r.querySelectorAll('img').length===3&&[...r.querySelectorAll('img')].every((p,i)=>p.src===pages[i])&&r.getBoundingClientRect().width<=1000})()",
    );
}

/// Artifact separators keep the source's collapsed page margins.
#[wasm_bindgen]
pub fn pdf_artifact_native_page_gap_follows_source_spacing() {
    let mut view = View::new();
    for _ in 0..3 {
        view.draw(false, vec![]);
    }
    check(
        "(()=>{const p=[...document.querySelectorAll('.maestro-pdf img')];return p.length===3&&p.slice(1).every((p,i)=>Math.abs(p.getBoundingClientRect().top-document.querySelectorAll('.maestro-pdf img')[i].getBoundingClientRect().bottom-33)<0.5)})()",
    );
}

/// Overlay separators keep the source's collapsed page margins.
#[wasm_bindgen]
pub fn pdf_overlay_native_page_gap_follows_source_spacing() {
    let view = View::new();
    let mut overlay = AttachmentOverlay::open(record(), None, view.host.clone());
    for _ in 0..4 {
        view.overlay(&mut overlay, vec![]);
    }
    check(
        "(()=>{const p=[...document.querySelectorAll('.maestro-pdf img')];return p.length===3&&p.slice(1).every((p,i)=>Math.abs(p.getBoundingClientRect().top-document.querySelectorAll('.maestro-pdf img')[i].getBoundingClientRect().bottom-33)<0.5)})()",
    );
}

/// The native overlay viewport owns panel chrome independently of the artifact.
#[wasm_bindgen]
pub fn pdf_overlay_native_viewport_keeps_panel_and_border() {
    let mut view = View::new();
    for pack in ["brand.json", "signal.json"] {
        view.pack = maestro_theme::load_brand_pack(pack, &support::brand::Pack).unwrap();
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            view.mode = mode;
            let mut overlay = AttachmentOverlay::open(record(), None, view.host.clone());
            for _ in 0..2 {
                view.overlay(&mut overlay, vec![]);
            }
            check(
                "(()=>{const r=document.querySelector('.maestro-pdf:not(.maestro-pdf-artifact)'),s=getComputedStyle(r),p=document.createElement('div');r.append(p);p.style.background='var(--maestro-color-panel)';p.style.border='1px solid var(--maestro-color-border)';const c=getComputedStyle(p);const ok=s.backgroundColor===c.backgroundColor&&s.borderTopColor===c.borderTopColor&&Math.abs(parseFloat(s.borderTopWidth)-1)<0.5&&s.borderTopStyle==='solid'&&s.boxShadow!=='none';p.remove();return ok})()",
            );
            drop(overlay);
            view.draw(false, vec![]);
            check(
                "(()=>{const r=document.querySelector('.maestro-pdf-artifact'),s=getComputedStyle(r),p=document.createElement('div');r.append(p);p.style.background='var(--maestro-color-background)';const ok=s.backgroundColor===getComputedStyle(p).backgroundColor&&s.borderTopWidth==='0px'&&s.boxShadow==='none';p.remove();return ok})()",
            );
        }
    }
}

/// Equal-content edits preserve completed pages and the reader's position.
#[wasm_bindgen]
pub fn pdf_same_content_rewrite_retains_pages_and_scroll() {
    let mut view = View::new();
    view.draw(false, vec![]);
    js("globalThis.firstPage=document.querySelector('.maestro-pdf img')");
    view.element.set_content(encoded(true));
    for _ in 0..2 {
        view.draw(false, vec![]);
    }
    check("firstPage===document.querySelector('.maestro-pdf img')");
    js(
        "globalThis.retainedRoot=document.querySelector('.maestro-pdf');globalThis.retainedPages=[...retainedRoot.querySelectorAll('img')];globalThis.retainedWrapper=retainedRoot.parentElement;retainedWrapper.scrollTop=41.5;globalThis.retainedScroll=retainedWrapper.scrollTop;",
    );
    check("retainedPages.length===3&&retainedScroll>0");
    view.element.set_content(encoded(true));
    view.draw(false, vec![]);
    check(
        "retainedRoot===document.querySelector('.maestro-pdf')&&retainedRoot.querySelectorAll('img').length===3&&retainedPages.every((p,i)=>p===retainedRoot.querySelectorAll('img')[i])&&retainedWrapper.scrollTop===retainedScroll",
    );
}

/// Caller brand properties govern chrome and localized selectable error text.
#[wasm_bindgen]
pub fn pdf_native_modes_and_error_text_follow_brand_and_localization() {
    let mut view = View::new();
    for pack in ["brand.json", "signal.json"] {
        view.pack = maestro_theme::load_brand_pack(pack, &support::brand::Pack).unwrap();
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            view.mode = mode;
            view.element.set_content(encoded(true));
            view.draw(false, vec![]);
            check(
                "(()=>{const p=document.querySelector('.maestro-pdf img'),r=p.parentElement;return getComputedStyle(p).borderTopColor===getComputedStyle(r).getPropertyValue('--maestro-color-border').trim().replace(/^#(.{2})(.{2})(.{2})$/,(_,a,b,c)=>`rgb(${parseInt(a,16)}, ${parseInt(b,16)}, ${parseInt(c,16)})`)&&getComputedStyle(p).backgroundColor==='rgb(255, 255, 255)'})()",
            );
            assert_artifact_error_chrome(&mut view);
        }
    }
    let translations = view.i18n.get_translations();
    for (key, value) in [
        ("Error loading PDF", "Erreur PDF"),
        ("Error loading file", "Erreur fichier"),
        ("Download", "Télécharger"),
    ] {
        translations.borrow()["en"].borrow_mut().insert(
            key.into(),
            maestro_web::TranslationValue::Text(value.into()),
        );
    }
    view.i18n.set_translations(translations);
    view.draw(false, vec![]);
    assert_eq!(
        js("document.querySelector('.maestro-file-error h3').textContent")
            .as_string()
            .unwrap(),
        "Erreur PDF"
    );
    let generic = create_artifact_element("a.zip", "", view.host.clone()).unwrap();
    drop(std::mem::replace(&mut view.element, generic));
    *view.host.failure.borrow_mut() = Some("<script>literal 雪 & quotes</script>".repeat(80));
    let mut overlay = AttachmentOverlay::open(record(), None, view.host.clone());
    view.overlay(&mut overlay, vec![]);
    view.overlay(&mut overlay, vec![]);
    assert_eq!(
        js("document.querySelector('.maestro-file-error h3').textContent")
            .as_string()
            .unwrap(),
        "Erreur fichier"
    );
    assert_eq!(
        js("document.querySelector('.maestro-file-error p').textContent")
            .as_string()
            .unwrap(),
        "<script>literal 雪 & quotes</script>".repeat(80)
    );
    check(
        "(()=>{const r=document.querySelector('.maestro-file-error'),p=r.querySelector('div'),a=r.getBoundingClientRect(),b=p.getBoundingClientRect();return !r.querySelector('script')&&b.width<=672&&Math.abs(a.left+a.width/2-b.left-b.width/2)<1&&getComputedStyle(r).padding==='0px'&&getComputedStyle(p.querySelector('p')).whiteSpace==='pre-wrap'})()",
    );
}

/// Edits cancel partial documents; the real header saves only current bytes.
#[wasm_bindgen]
pub fn pdf_native_replacement_and_download_use_current_document() {
    let mut view = View::new();
    view.draw(false, vec![]);
    js("globalThis.oldPage=document.querySelector('.maestro-pdf img')");
    view.element.set_content(encoded(false));
    view.draw(false, vec![]);
    check(
        "!oldPage.isConnected&&document.querySelectorAll('.maestro-pdf img').length===1&&document.querySelector('.maestro-pdf img').getAttribute('height')==='300'",
    );
    js(
        "globalThis.currentPage=document.querySelector('.maestro-pdf img');globalThis.currentRoot=currentPage.closest('.maestro-pdf');globalThis.wrapper=currentRoot.parentElement;wrapper.scrollTop=10;",
    );
    for _ in 0..4 {
        view.draw(false, vec![]);
    }
    check(
        "currentPage===document.querySelector('.maestro-pdf img')&&currentRoot===document.querySelector('.maestro-pdf')&&document.querySelectorAll('.maestro-pdf img').length===1",
    );
    view.element.set_content(String::new());
    view.draw(false, vec![]);
    check("currentPage===document.querySelector('.maestro-pdf img')");
    view.element.set_content("invalid!".into());
    view.draw(false, vec![]);
    check(
        "!currentRoot.isConnected&&document.querySelector('.maestro-file-error h3').textContent==='Error loading PDF'",
    );
    view.element.set_content(String::new());
    view.draw(false, vec![]);
    check(
        "!document.querySelector('.maestro-file-error')&&document.querySelectorAll('.maestro-pdf img').length===0",
    );
    assert_complete_pdf_encodings(&mut view);
    view.element.set_content(encoded(false));
    view.draw(false, vec![]);
    view.draw(true, vec![]);
    view.draw(true, click(egui::pos2(18., 16.)));
    check("document.querySelector('a[download]')===null");
    js("globalThis.lastRoot=document.querySelector('.maestro-pdf')");
    drop(view);
    check("!lastRoot.isConnected");
}

/// Existing overlay failure triggers and details survive the native-text move.
#[wasm_bindgen]
pub fn overlay_native_errors_preserve_existing_behaviour() {
    let view = View::new();
    let record = record();
    *view.host.failure.borrow_mut() = Some("preview failure".into());
    let mut overlay = AttachmentOverlay::open(record.clone(), None, view.host.clone());
    view.overlay(&mut overlay, vec![]);
    view.overlay(&mut overlay, vec![]);
    assert_eq!(
        js("document.querySelector('.maestro-file-error h3').textContent")
            .as_string()
            .unwrap(),
        "Error loading file"
    );
    assert_eq!(
        js("document.querySelector('.maestro-file-error p').textContent")
            .as_string()
            .unwrap(),
        "preview failure"
    );
    js(
        "globalThis.errorRoot=document.querySelector('.maestro-file-error');const range=document.createRange();range.selectNodeContents(errorRoot.querySelector('p'));getSelection().removeAllRanges();getSelection().addRange(range)",
    );
    for mime in [
        "image/png",
        "text/plain",
        "presentationml",
        "wordprocessingml",
        "ms-excel",
    ] {
        record.borrow_mut().mime_type = mime.into();
        record.borrow_mut().content = "record edits must not retry loading".into();
        view.overlay(&mut overlay, vec![]);
        check(
            "errorRoot===document.querySelector('.maestro-file-error')&&getSelection().toString()==='preview failure'&&!document.querySelector('.maestro-pdf')&&errorRoot.querySelector('div').getBoundingClientRect().width<=672",
        );
    }
    record.borrow_mut().mime_type = "application/pdf".into();
    record.borrow_mut().content = encoded(true);
    view.overlay(&mut overlay, vec![]);
    assert_overlay_text_failure(&view, &mut overlay, &record);
    drop(overlay);
    assert_presentation_failure(&view, &record);
    assert_empty_error_and_save_failure(&view, &record);
}

/// Non-reading transitions release old roots, including same-tag previews.
#[wasm_bindgen]
pub fn reading_content_switches_release_old_native_roots() {
    let view = View::new();
    let record = record();
    // Warm up the overlay controls without consuming a PDF frame.
    record.borrow_mut().mime_type = "text/plain".into();
    let mut overlay = AttachmentOverlay::open(record.clone(), None, view.host.clone());
    view.overlay(&mut overlay, vec![]);
    view.overlay(&mut overlay, vec![]);
    record.borrow_mut().mime_type = "application/pdf".into();
    view.overlay(&mut overlay, vec![]);
    check("document.querySelectorAll('.maestro-pdf img').length===1");
    js(
        "globalThis.oldRoot=document.querySelector('.maestro-pdf');globalThis.firstPage=oldRoot.querySelector('img').src;",
    );
    view.toggle(&mut overlay, "Text");
    check(
        "!oldRoot.isConnected&&[...document.querySelectorAll('pre')].some(p=>p.textContent==='literal text')",
    );
    view.toggle(&mut overlay, "PDF");
    check(
        "document.querySelectorAll('.maestro-pdf img').length===1&&document.querySelector('.maestro-pdf img').src===firstPage",
    );
    for mime in ["text/plain", "image/png", "wordprocessingml"] {
        js("globalThis.oldRoot=document.querySelector('.maestro-pdf')");
        record.borrow_mut().mime_type = mime.into();
        record.borrow_mut().r#type = if mime == "image/png" {
            maestro_app::browser::utils::attachment_utils::AttachmentType::Image
        } else {
            maestro_app::browser::utils::attachment_utils::AttachmentType::Document
        };
        view.overlay(&mut overlay, vec![]);
        check("!oldRoot.isConnected&&!document.querySelector('.maestro-pdf')");
        record.borrow_mut().mime_type = "application/pdf".into();
        record.borrow_mut().r#type =
            maestro_app::browser::utils::attachment_utils::AttachmentType::Document;
        view.overlay(&mut overlay, vec![]);
        check(
            "document.querySelectorAll('.maestro-pdf img').length===1&&document.querySelector('.maestro-pdf').isConnected",
        );
    }
    *view.host.failure.borrow_mut() = Some("failure".into());
    view.overlay(&mut overlay, vec![]);
    view.overlay(&mut overlay, vec![]);
    check(
        "document.querySelector('.maestro-file-error h3').textContent==='Error loading file'&&document.querySelector('.maestro-file-error p').textContent==='failure'",
    );
    js("globalThis.oldError=document.querySelector('.maestro-file-error')");
    view.toggle(&mut overlay, "Text");
    check("!oldError.isConnected&&!document.querySelector('.maestro-file-error')");
    view.toggle(&mut overlay, "PDF");
    check("document.querySelectorAll('.maestro-pdf img').length===1");
    js("globalThis.lastRoot=document.querySelector('.maestro-pdf')");
    drop(overlay);
    check(
        "!lastRoot.isConnected&&!document.querySelector('.maestro-pdf')&&!document.querySelector('.maestro-file-error')",
    );
}

/// Check literal diagnostics, caller styling and selection in the current mode.
fn assert_artifact_error_chrome(view: &mut View) {
    view.element.set_content("Zg==".into());
    let diagnostic =
        maestro_app::browser::utils::attachment_utils::pdf::PdfDocument::new(vec![102])
            .err()
            .unwrap()
            .to_string();
    view.draw(false, vec![]);
    assert_eq!(
        js("document.querySelector('.maestro-file-error h3').textContent")
            .as_string()
            .unwrap(),
        "Error loading PDF"
    );
    assert_eq!(
        js("document.querySelector('.maestro-file-error p').textContent")
            .as_string()
            .unwrap(),
        diagnostic
    );
    check(
        "(()=>{const r=document.querySelector('.maestro-file-error'),p=r.querySelector('div'),s=getComputedStyle(r),c=getComputedStyle(p);return c.maxWidth==='672px'&&getComputedStyle(r).padding==='16px'&&c.backgroundColor!==s.color&&c.borderTopColor===s.color&&c.overflowWrap==='anywhere'&&s.fontFamily.replaceAll(String.fromCharCode(34),'')===s.getPropertyValue('--maestro-type-body-font').trim().replaceAll(String.fromCharCode(34),'')})()",
    );
    js(
        "globalThis.errorRoot=document.querySelector('.maestro-file-error');const range=document.createRange();range.selectNodeContents(errorRoot.querySelector('p'));getSelection().removeAllRanges();getSelection().addRange(range);globalThis.selected=getSelection().toString();",
    );
    view.draw(false, vec![]);
    check(
        "errorRoot===document.querySelector('.maestro-file-error')&&getSelection().toString()===selected&&selected.length>0",
    );
}

/// Complete raw, prefixed and folded original documents through the real body.
fn assert_complete_pdf_encodings(view: &mut View) {
    for row in [
        encoded(true),
        format!("data:application/pdf;base64,{}", encoded(true)),
        format!(
            "data:application/pdf;base64,{}",
            encoded(true)
                .as_bytes()
                .chunks(72)
                .map(|p| std::str::from_utf8(p).unwrap())
                .collect::<Vec<_>>()
                .join("\n")
        ),
    ] {
        view.element.set_content(row);
        for _ in 0..3 {
            view.draw(false, vec![]);
        }
        check(
            "document.querySelectorAll('.maestro-pdf img').length===3&&document.querySelectorAll('.maestro-pdf hr').length===2",
        );
    }
}

/// Text failures stay dominant, including when native error publication fails.
fn assert_overlay_text_failure(
    view: &View,
    overlay: &mut AttachmentOverlay,
    record: &Rc<RefCell<maestro_web::Attachment>>,
) {
    view.toggle(overlay, "Text");
    check(
        "!document.querySelector('.maestro-file-error')&&[...document.querySelectorAll('pre')].some(p=>p.textContent==='literal text')",
    );
    record.borrow_mut().extracted_text = Some("retained alias".into());
    view.overlay(overlay, vec![]);
    check("[...document.querySelectorAll('pre')].some(p=>p.textContent==='retained alias')");
    *view.host.failure.borrow_mut() = Some("text failure".into());
    view.overlay(overlay, vec![]);
    view.overlay(overlay, vec![]);
    assert_eq!(
        js("document.querySelector('.maestro-file-error p').textContent")
            .as_string()
            .unwrap(),
        "text failure"
    );
    record.borrow_mut().mime_type = "image/png".into();
    view.overlay(overlay, vec![]);
    assert_eq!(
        js("document.querySelector('.maestro-file-error p').textContent")
            .as_string()
            .unwrap(),
        "text failure"
    );
    record.borrow_mut().mime_type = "application/pdf".into();
    view.overlay(overlay, vec![]);
    *view.host.failure.borrow_mut() = Some("publication rejected".into());
    assert!(matches!(
        view.try_overlay(overlay, vec![]),
        Err(AttachmentUiError::Presentation(_))
    ));
    view.overlay(overlay, vec![]);
    assert_eq!(
        js("document.querySelector('.maestro-file-error p').textContent")
            .as_string()
            .unwrap(),
        "text failure"
    );
    view.toggle(overlay, "PDF");
    check(
        "!document.querySelector('.maestro-file-error')&&document.querySelectorAll('.maestro-pdf img').length===1",
    );
}

/// Presentation text retains its established one-shot failure detail.
fn assert_presentation_failure(view: &View, record: &Rc<RefCell<maestro_web::Attachment>>) {
    record.borrow_mut().mime_type = "presentationml".into();
    *view.host.failure.borrow_mut() = Some("presentation failure".into());
    let mut overlay = AttachmentOverlay::open(record.clone(), None, view.host.clone());
    view.overlay(&mut overlay, vec![]);
    view.overlay(&mut overlay, vec![]);
    assert_eq!(
        js("document.querySelector('.maestro-file-error p').textContent")
            .as_string()
            .unwrap(),
        "presentation failure"
    );
    drop(overlay);
}

/// Suppressed empty errors keep the body, save policy and close control live.
fn assert_empty_error_and_save_failure(view: &View, record: &Rc<RefCell<maestro_web::Attachment>>) {
    record.borrow_mut().mime_type = "application/pdf".into();
    *view.host.failure.borrow_mut() = Some(String::new());
    let mut overlay = AttachmentOverlay::open(record.clone(), None, view.host.clone());
    view.overlay(&mut overlay, vec![]);
    view.overlay(&mut overlay, vec![]);
    check(
        "!document.querySelector('.maestro-file-error')&&document.querySelectorAll('.maestro-pdf img').length>0",
    );
    view.toggle(&mut overlay, "Text");
    view.host.fail_download.set(true);
    let result = view.try_overlay(&mut overlay, click(egui::pos2(728., 24.)));
    assert!(matches!(result, Err(AttachmentUiError::Host(_))));
    assert!(Rc::ptr_eq(
        view.host.downloads.borrow().last().unwrap(),
        &record
    ));
    view.host.fail_download.set(false);
    view.overlay(&mut overlay, vec![]);
    check(
        "!document.querySelector('.maestro-file-error')&&[...document.querySelectorAll('pre')].some(p=>p.textContent==='retained alias')",
    );
    view.overlay(
        &mut overlay,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    check(
        "!document.querySelector('.maestro-file-error')&&!document.querySelector('.maestro-pdf')",
    );
}
