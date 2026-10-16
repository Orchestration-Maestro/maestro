//! Shared actual retained-panel browser harness.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{presentation, support};
use maestro_app::browser::tools::artifacts::LocalArtifactsPanel;
use maestro_theme::BrandMode;
use maestro_web::{
    ArtifactsPanel, I18n, browser::attachment_content::BrowserAttachmentHost,
    tools::artifacts::artifact_element::ArtifactView,
};
use std::rc::Rc;
use wasm_bindgen::JsCast as _;
/// Actual application caller, retained panel and document-scoped native host.
pub struct Panel {
    /// Existing document-scoped native registry.
    pub host: Rc<BrowserAttachmentHost>,
    /// Retained production panel.
    pub panel: Rc<ArtifactsPanel>,
    /// Single application selection owner.
    pub caller: LocalArtifactsPanel,
    /// Owned toolkit frame state.
    pub context: egui::Context,
    /// Caller-supplied brand pack.
    pub pack: maestro_theme::BrandPack,
    /// Live caller localization.
    pub i18n: I18n,
}
impl Panel {
    /// Bind the runner's existing canvas and the existing local presentation harness.
    pub fn new() -> Self {
        std::panic::set_hook(Box::new(|info| {
            web_sys::console::error_1(&info.to_string().into())
        }));
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document
            .get_element_by_id("canvas")
            .unwrap()
            .dyn_into()
            .unwrap();
        let host = Rc::new(BrowserAttachmentHost::new(canvas).unwrap());
        let panel = Rc::new(ArtifactsPanel::new(host.clone()));
        let mut presentation = presentation::Presentation::new(host.clone());
        presentation.panel = Some(panel.clone());
        let caller = LocalArtifactsPanel::new_local(Rc::new(presentation));
        panel.connected_callback(&caller).unwrap();
        Self {
            host,
            panel,
            caller,
            context: egui::Context::default(),
            pack: maestro_theme::load_brand_pack("brand.json", &support::brand::Pack).unwrap(),
            i18n: I18n::new(Rc::new(support::Host::default())).unwrap(),
        }
    }
    /// Create through the actual tool, not a replacement artifact map.
    pub fn create(&self, name: &str, content: &str) {
        presentation::ready((self.caller.tool().execute)(
            "call".into(),
            serde_json::json!({"command":"create","filename":name,"content":content}),
            None,
            None,
        ))
        .unwrap();
        self.draw(vec![]);
    }
    /// Draw the real public panel with the current application selection.
    pub fn draw(&self, events: Vec<egui::Event>) -> egui::FullOutput {
        let presentation = self.pack.presentation(BrandMode::Dark).unwrap();
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
                self.panel
                    .render(
                        ui,
                        &self.caller,
                        ArtifactView {
                            i18n: &self.i18n,
                            presentation: &presentation,
                        },
                    )
                    .unwrap();
            },
        );
        output.textures_delta.clear();
        output
    }
}
