//! Real native transaction harness using only public reading admission.
#![cfg(test)]
#![cfg(target_arch = "wasm32")]
use maestro_theme::BrandMode;
use maestro_web::browser::attachment_content::*;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::Element;

/// Invoke native browser observations and fault hooks below the adapter.
/// # Panics
/// Panics when the browser observation throws.
pub(crate) fn js(argument: &JsValue, body: &str) -> JsValue {
    js_sys::Function::new_with_args("value", body)
        .call1(&JsValue::NULL, argument)
        .unwrap()
}
/// A complete public host registration, without production inspection hooks.
pub struct Slots {
    pub host: Rc<BrowserAttachmentHost>,
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
    /// Create an actual-document registration on the supplied runner canvas.
    /// # Panics
    /// Panics when the canvas, browser host or embedded pack is unavailable.
    #[must_use]
    pub fn new() -> Self {
        std::panic::set_hook(Box::new(|info| {
            web_sys::console::error_1(&info.to_string().into());
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
            host: Rc::new(BrowserAttachmentHost::new(canvas).unwrap()),
            content,
            id: egui::Id::new("native-slots"),
            root,
            context: egui::Context::default(),
            origin: egui::pos2(12., 18.),
            clip: egui::Rect::EVERYTHING,
            holes: vec![],
            density: 1.,
            mode: BrandMode::Dark,
            pack: maestro_theme::load_brand_pack("brand.json", &crate::support::brand::Pack)
                .unwrap(),
        }
    }
    /// Build the fixed native reading witness and retain only its node mirror.
    /// # Panics
    /// Panics when required native nodes cannot be created or found.
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
                    commit: None,
                    owner: None,
                    element: root.clone(),
                    slots: slots.clone(),
                });
                Ok(ReadingRoot {
                    commit: None,
                    owner: None,
                    element: root,
                    slots,
                })
            }),
        })
    }
    /// Return the admitted native root.
    /// # Panics
    /// Panics before the factory has mounted.
    #[must_use]
    pub fn root(&self) -> Element {
        self.root.borrow().as_ref().unwrap().element.clone()
    }
    /// Return the admitted native scrolling viewport.
    /// # Panics
    /// Panics when the root has no viewport.
    #[must_use]
    pub fn wrapper(&self) -> Element {
        self.root().parent_element().unwrap()
    }
    /// Return the native clipping frame.
    /// # Panics
    /// Panics when the root or containing viewport is detached.
    #[must_use]
    pub fn frame(&self) -> Element {
        self.wrapper().parent_element().unwrap()
    }
    /// Submit one explicit reading transaction phase.
    /// # Errors
    /// Returns native presentation failures.
    /// # Panics
    /// Panics when the embedded pack cannot provide its selected mode.
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
    /// Read the current native slot layout.
    /// # Errors
    /// Returns missing-registration or native readback failures.
    pub fn layout(&self) -> Result<ReadingLayout, AttachmentUiError> {
        self.host.reading_layout(self.id)
    }
    /// Measure and publish one bounded native viewport.
    /// # Panics
    /// Panics when native measurement or publication fails.
    pub(crate) fn publish(
        &self,
        size: egui::Vec2,
        sizes: &[(egui::Id, egui::Vec2)],
    ) -> ReadingLayout {
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
    /// Supply three independent canvas-control dimensions.
    /// # Panics
    /// Panics if a three-element ordinal cannot fit its slot identity.
    #[must_use]
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
#[must_use]
pub fn snapshot(root: &Element) -> JsValue {
    js(
        root,
        include_str!("../session_header_browser_support/snapshot.js"),
    )
}
/// Compare native state against the retained snapshot, without serializing nodes.
/// # Panics
/// Panics when any retained native state differs from the snapshot.
pub fn unchanged(saved: &JsValue) {
    assert!(
        js(
            saved,
            include_str!("../session_header_browser_support/unchanged.js")
        )
        .as_bool()
        .unwrap()
    );
}

impl Default for Slots {
    fn default() -> Self {
        Self::new()
    }
}
