//! Real browser qualification of the generic file view.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::support;
use maestro_web::{
    ArtifactElement, I18n,
    browser::attachment_content::BrowserAttachmentHost,
    tools::artifacts::{artifact_element::ArtifactView, create_artifact_element},
};
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, prelude::*};
/// Actual native view and caller retained across redraws.
struct View {
    /// Retained production element.
    element: Box<dyn ArtifactElement>,
    /// Embedded caller brand.
    pack: maestro_theme::BrandPack,
    /// Caller dictionary.
    i18n: I18n,
    /// Owned frame and focus state.
    context: egui::Context,
    /// Actual widget-event labels from the most recent frame.
    labels: Vec<String>,
}
impl View {
    /// Bind the supplied canvas, not a synthesized native sample.
    fn new() -> Self {
        std::panic::set_hook(Box::new(|info| {
            web_sys::console::error_1(&info.to_string().into());
        }));
        let doc = web_sys::window().unwrap().document().unwrap();
        let canvas = doc.get_element_by_id("canvas").unwrap().dyn_into().unwrap();
        let host = Rc::new(BrowserAttachmentHost::new(canvas).unwrap());
        Self {
            element: create_artifact_element("a.md.zip", "AAEC//79", host).unwrap(),
            pack: support::brand::pack("brand.json").unwrap(),
            i18n: I18n::new(Rc::new(support::Host::default())).unwrap(),
            context: egui::Context::default(),
            labels: Vec::new(),
        }
    }
    /// Draw the public body and header against the actual browser host.
    fn draw(&mut self, events: Vec<egui::Event>) {
        let presentation = self
            .pack
            .presentation(maestro_theme::BrandMode::Dark)
            .unwrap();
        let mut output = self.context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400., 300.),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                let view = ArtifactView {
                    highlighter: &crate::highlighter_support::NoHighlight,
                    i18n: &self.i18n,
                    presentation: &presentation,
                };
                self.element.get_header_buttons(ui, view).unwrap();
                self.element.render(ui, view).unwrap();
            },
        );
        output.textures_delta.clear();
        self.labels = output
            .platform_output
            .events
            .iter()
            .filter_map(|event| event.widget_info().label.clone())
            .collect();
    }
    /// Activate the actual canvas-owned header control.
    fn click(&mut self) {
        let pos = egui::pos2(18., 16.);
        self.draw(vec![
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
        ]);
    }
}
/// Execute native DOM observations without adding production feature bindings.
fn js(code: &str) -> JsValue {
    js_sys::eval(code).unwrap()
}
/// Literal hostile filenames and localized guidance remain native selectable text.
#[wasm_bindgen]
pub async fn generic_guidance_is_literal_selectable_and_localized() {
    let mut view = View::new();
    view.element
        .set_filename(" <script>雪 & \"quotes\"</script>.zip".into());
    view.draw(vec![]);
    assert_eq!(
        js("document.querySelector('.maestro-generic-artifact > div > div').textContent")
            .as_string()
            .unwrap(),
        view.element.filename()
    );
    assert!(
        js("document.querySelector('.maestro-generic-artifact script') === null")
            .as_bool()
            .unwrap()
    );
    assert_eq!(
        js("document.querySelector('.maestro-generic-artifact p').textContent")
            .as_string()
            .unwrap(),
        "Preview not available for this file type. Click the download button above to view it on your computer."
    );
    assert!(js("(()=>{const r=document.createRange();r.selectNodeContents(document.querySelector('.maestro-generic-artifact p'));getSelection().removeAllRanges();getSelection().addRange(r);return getSelection().toString().includes('Preview not available');})()").as_bool().unwrap());
    assert!(js("(()=>{const r=document.querySelector('.maestro-generic-artifact').getBoundingClientRect();const s=document.querySelector('.maestro-generic-artifact svg').getBoundingClientRect();return r.width>0 && r.height>0 && s.width===64 && s.height===64;})()").as_bool().unwrap());
    support::reading_paint::observe("neighbors-generic").await;
    let translations = view.i18n.get_translations();
    for (key, value) in [
        ("Download", "Télécharger"),
        (
            "Preview not available for this file type.",
            "Aperçu indisponible.",
        ),
        (
            "Click the download button above to view it on your computer.",
            "Téléchargez le fichier.",
        ),
    ] {
        translations.borrow()["en"].borrow_mut().insert(
            key.into(),
            maestro_web::TranslationValue::Text(value.into()),
        );
    }
    view.i18n.set_translations(translations);
    view.draw(vec![egui::Event::Key {
        key: egui::Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(view.labels.iter().any(|label| label == "Télécharger"));
    assert_eq!(
        js("document.querySelector('.maestro-generic-artifact p').textContent")
            .as_string()
            .unwrap(),
        "Aperçu indisponible. Téléchargez le fichier."
    );
}
/// Root identity and unrelated native selection/scroll survive field edits.
#[wasm_bindgen]
pub fn generic_reading_root_survives_field_edits() {
    let mut view = View::new();
    view.element.set_content("Zg==".into());
    view.draw(vec![]);
    js(
        "globalThis.savedRoot=document.querySelector('.maestro-generic-artifact');globalThis.wrapper=savedRoot.parentElement;globalThis.first=savedRoot.querySelector('p span');globalThis.second=savedRoot.querySelector('p span:last-child');const r=document.createRange();r.selectNodeContents(first);getSelection().removeAllRanges();getSelection().addRange(r);savedRoot.style.minHeight='900px';wrapper.scrollTop=23.5;globalThis.savedScroll=wrapper.scrollTop;",
    );
    assert!(js("savedScroll>0").as_bool().unwrap());
    view.element.set_content("Zm8=".into());
    view.draw(vec![]);
    assert!(js("savedRoot===document.querySelector('.maestro-generic-artifact') && getSelection().toString()===first.textContent && wrapper.scrollTop===savedScroll").as_bool().unwrap());
    view.element.set_filename("owned <name>.zip".into());
    view.draw(vec![]);
    assert_eq!(
        js("savedRoot.querySelector('div > div').textContent")
            .as_string()
            .unwrap(),
        "owned <name>.zip"
    );
    assert!(
        js("getSelection().toString()===first.textContent && wrapper.scrollTop===savedScroll")
            .as_bool()
            .unwrap()
    );
    js(
        "const q=document.createRange();q.selectNodeContents(second);getSelection().removeAllRanges();getSelection().addRange(q);",
    );
    let translations = view.i18n.get_translations();
    translations.borrow()["en"].borrow_mut().insert(
        "Preview not available for this file type.".into(),
        maestro_web::TranslationValue::Text("Aperçu indisponible.".into()),
    );
    view.i18n.set_translations(translations);
    view.draw(vec![]);
    assert!(js("savedRoot===document.querySelector('.maestro-generic-artifact') && first.textContent==='Aperçu indisponible.' && getSelection().toString()===second.textContent && wrapper.scrollTop===savedScroll").as_bool().unwrap());
    view.click();
    drop(view);
    assert!(js("!savedRoot.isConnected").as_bool().unwrap());
}
/// The browser runner captures the reached download event and original file bytes.
#[wasm_bindgen]
pub fn generic_download_saves_original_blob_bytes() {
    let mut view = View::new();
    view.draw(vec![]);
    view.click();
    assert!(
        js("document.querySelector('a[download]')===null")
            .as_bool()
            .unwrap()
    );
}
