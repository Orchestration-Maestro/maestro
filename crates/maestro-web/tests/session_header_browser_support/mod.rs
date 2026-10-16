//! Real native transaction harness using only public reading admission.
#![cfg(all(test, target_arch = "wasm32"))]
use maestro_theme::BrandMode;
use maestro_web::browser::attachment_content::*;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::Element;

/// Invoke native browser observations and fault hooks below the adapter.
pub fn js(argument: &JsValue, body: &str) -> JsValue {
    js_sys::Function::new_with_args("value", body)
        .call1(&JsValue::NULL, argument)
        .unwrap()
}
/// A complete public host registration, without production inspection hooks.
pub struct Slots {
    pub host: BrowserAttachmentHost,
    pub content: Rc<ReadingContent>,
    pub id: egui::Id,
    pub root: Rc<RefCell<Option<ReadingRoot>>>,
    pub context: egui::Context,
    pub pack: maestro_theme::BrandPack,
    pub origin: egui::Pos2,
    pub clip: egui::Rect,
    pub holes: Vec<egui::Rect>,
    pub density: f32,
    pub mode: BrandMode,
}
impl Slots {
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
        let root = Rc::new(RefCell::new(None));
        let content = Self::factory(Rc::clone(&root));
        Self {
            host: BrowserAttachmentHost::new(canvas).unwrap(),
            content,
            id: egui::Id::new("native-slots"),
            root,
            context: egui::Context::default(),
            origin: egui::pos2(12., 18.),
            clip: egui::Rect::EVERYTHING,
            holes: vec![],
            density: 1.,
            mode: BrandMode::Dark,
            pack: maestro_theme::load_brand_pack("brand.json", &super::Pack).unwrap(),
        }
    }
    pub fn factory(target: Rc<RefCell<Option<ReadingRoot>>>) -> Rc<ReadingContent> {
        Rc::new(ReadingContent {
            mount: Rc::new(move |document| {
                let root = document.create_element("div").unwrap();
                // Test markup is fixed; production factories only mount literal text.
                root.set_inner_html("<style>.test-slot{flex-shrink:0;display:block}</style><h1 style='white-space:nowrap'>selectable long wide native text for scrolling both axes and retaining selection endpoints</h1><div style='display:flex;flex-wrap:wrap;gap:4px'><span class='test-slot'></span><span class='test-slot' style=''></span><span class='test-slot' style='opacity:1'></span></div><p style='height:200px'>following native text</p>");
                let slots = (0..3)
                    .map(|i| {
                        let element: Element = js(
                            &root,
                            &format!("return value.querySelectorAll('.test-slot')[{i}]"),
                        )
                        .dyn_into()
                        .unwrap();
                        (egui::Id::new(i), element)
                    })
                    .collect::<Vec<_>>();
                *target.borrow_mut() = Some(ReadingRoot {
                    owner: None,
                    element: root.clone(),
                    slots: slots.clone(),
                });
                Ok(ReadingRoot {
                    owner: None,
                    element: root,
                    slots,
                })
            }),
        })
    }
    pub fn root(&self) -> Element {
        self.root.borrow().as_ref().unwrap().element.clone()
    }
    pub fn wrapper(&self) -> Element {
        self.root().parent_element().unwrap()
    }
    pub fn frame(&self) -> Element {
        self.wrapper().parent_element().unwrap()
    }
    pub fn place(
        &self,
        phase: ReadingPhase,
        size: egui::Vec2,
        sizes: &[(egui::Id, egui::Vec2)],
    ) -> Result<(), AttachmentUiError> {
        self.host.present(
            AttachmentSurface {
                id: self.id,
                rect: egui::Rect::from_min_size(self.origin, size),
                clip_rect: self.clip,
                pixels_per_point: self.density,
                opacity: 1.,
                occluders: self.holes.clone(),
                content: AttachmentContent::Reading(ReadingPlacement {
                    content: Rc::clone(&self.content),
                    phase,
                    sizes,
                    modal: None,
                    context: &self.context,
                }),
            },
            &self.pack.presentation(self.mode).unwrap(),
        )
    }
    pub fn layout(&self) -> Result<ReadingLayout, AttachmentUiError> {
        self.host.reading_layout(self.id)
    }
    pub fn publish(&self, size: egui::Vec2, sizes: &[(egui::Id, egui::Vec2)]) -> ReadingLayout {
        self.place(ReadingPhase::Measure, size, sizes).unwrap();
        let layout = self.layout().unwrap();
        self.place(
            ReadingPhase::Publish,
            egui::vec2(size.x, layout.height.min(size.y)),
            sizes,
        )
        .unwrap();
        layout
    }
    pub fn sizes(values: [(f32, f32); 3]) -> Vec<(egui::Id, egui::Vec2)> {
        values
            .into_iter()
            .enumerate()
            .map(|(i, (w, h))| (egui::Id::new(i32::try_from(i).unwrap()), egui::vec2(w, h)))
            .collect()
    }
}
impl Drop for Slots {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
/// Capture actual node identities, optional attributes and fractional offsets.
pub fn snapshot(root: &Element) -> JsValue {
    js(root, include_str!("snapshot.js"))
}
/// Compare native state against the retained snapshot, without serializing nodes.
pub fn unchanged(saved: &JsValue) {
    assert!(js(saved, include_str!("unchanged.js")).as_bool().unwrap());
}

mod disclosures;
