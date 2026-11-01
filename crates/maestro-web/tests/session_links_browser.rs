//! Real-document clipboard effects and native message-slot composition.
#![cfg(test)]
#![cfg(target_arch = "wasm32")]
use crate::reading_slots_support;
use crate::support;
use crate::support::pointer::pointer;
use maestro_web::browser::attachment_content::{AttachmentHost, BrowserAttachmentHost};
use reading_slots_support::{js, snapshot, unchanged};
use wasm_bindgen::{JsCast as _, prelude::*};

/// Construct the default adapter on the runner's actual canvas document.
fn host() -> BrowserAttachmentHost {
    let canvas = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .get_element_by_id("canvas")
        .unwrap()
        .dyn_into()
        .unwrap();
    BrowserAttachmentHost::new(canvas).unwrap()
}
/// Native API admission precedes dispatch; rejected and absent APIs select fallback.
#[wasm_bindgen]
pub async fn browser_clipboard_prefers_api_and_falls_back() {
    let fixture = Fixture::load().await;
    let host = host();
    for mode in [
        "api", "missing", "method", "getter", "call", "reject", "false",
    ] {
        fixture.begin(mode, "literal 雪😀\n<entry>&");
        let work = host.copy_to_clipboard(
            "literal 雪😀\n<entry>&",
            maestro_web::browser::attachment_content::ClipboardMode::Fallback,
        );
        fixture.immediate(mode);
        fixture.finish(work.await);
    }
}
/// Real temporary nodes are disconnected on every reached failure and success.
#[wasm_bindgen]
pub async fn browser_clipboard_cleans_every_temporary_textarea() {
    let fixture = Fixture::load().await;
    let host = host();
    for mode in [
        "create", "body", "append", "exec", "missing", "false", "removed",
    ] {
        fixture.begin(mode, "original 雪😀\ntext");
        let work = host.copy_to_clipboard(
            "original 雪😀\ntext",
            maestro_web::browser::attachment_content::ClipboardMode::Fallback,
        );
        fixture.immediate(mode);
        fixture.finish(work.await);
    }
}

/// Keep the control outside the native root's callback owner, with one shared host.
struct Composition {
    /// Canvas control lifetime ends before native registration release.
    state: maestro_web::export_html::template::CopyLinkState,
    /// Existing native registration and measured placement owner.
    slots: reading_slots_support::Slots,
    /// Literal slot position from native measurement.
    rect: egui::Rect,
}
impl Composition {
    /// Measure a literal reading message with one identified control hole.
    fn new() -> Self {
        use maestro_web::browser::attachment_content::{ReadingContent, ReadingRoot};

        let mut slots = reading_slots_support::Slots::new();
        let target = slots.root.clone();
        slots.content = std::rc::Rc::new(ReadingContent {
            mount: std::rc::Rc::new(move |document| {
                let root = document.create_element("div").unwrap();
                root.set_attribute("class", "link-message").unwrap();
                root.set_attribute("style", "position:relative;padding:6px;height:200px")
                    .unwrap();
                root.set_inner_html("<style>.link-message .test-slot{display:block;position:absolute;right:8px;top:8px}</style><p>selectable native message 雪 text</p><span class='test-slot'></span>");
                let slot = root.query_selector(".test-slot").unwrap().unwrap();
                reading_slots_support::js(
                    &root,
                    "value.addEventListener('mouseenter', event=>{if(!event.isTrusted)throw new Error('untrusted hover');event.currentTarget.setAttribute('data-hovered','true')});value.addEventListener('mouseleave',event=>event.currentTarget.setAttribute('data-hovered','false'));return true",
                );
                let controls = vec![(egui::Id::new("link-slot"), slot)];
                *target.borrow_mut() = Some(ReadingRoot {
                    frame_style: "",
                    decoration: None,
                    element: root.clone(),
                    slots: controls.clone(),
                    commit: None,
                    owner: None,
                });
                Ok(ReadingRoot {
                    frame_style: "",
                    decoration: None,
                    element: root,
                    slots: controls,
                    commit: None,
                    owner: None,
                })
            }),
        });
        let sizes = [(egui::Id::new("link-slot"), egui::vec2(28., 28.))];
        let layout = slots.publish(egui::vec2(300., 80.), &sizes);
        let rect = layout.controls[0].1.translate(slots.origin.to_vec2());
        slots.holes = vec![rect];
        slots.publish(egui::vec2(300., 80.), &sizes);
        Self {
            state: maestro_web::export_html::template::CopyLinkState::new(slots.host.clone()),
            slots,
            rect,
        }
    }
    /// Paint into the same measured hole, leaving native text to its owner.
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::Response {
        let p = self.slots.pack.presentation(self.slots.mode).unwrap();
        let hovered = reading_slots_support::js(
            &self.slots.root(),
            "return value.getAttribute('data-hovered') === 'true'",
        )
        .as_bool()
        .unwrap();
        let mut response = None;
        let mut output = self.slots.context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                ui.set_clip_rect(self.slots.clip);
                response = Some(
                    maestro_web::export_html::template::render_copy_link_button(
                        ui,
                        "native 雪",
                        &mut self.state,
                        maestro_web::export_html::template::CopyLinkView {
                            presentation: &p,
                            current_leaf_id: "branch",
                            location_href: "https://example.test/view",
                            share_base_url: None,
                            rect: self.rect,
                            message_hovered: hovered,
                        },
                    )
                    .unwrap(),
                );
            },
        );
        output.textures_delta.clear();
        let response = response.unwrap();
        if response.gained_focus() {
            reading_slots_support::js(
                &self.slots.root(),
                "value.querySelector('.test-slot').scrollIntoView({block:'nearest',inline:'nearest'}); return true",
            );
        }
        response
    }
}
/// The real containing owner retains selection, native placement and weak completion.
/// # Panics
/// Panics when the actual native observations differ from the required behavior.
#[wasm_bindgen]
pub async fn copy_link_native_slot_composition_preserves_reading_selection() {
    let fixture = Fixture::load().await;
    let mut view = Composition::new();
    let root = view.slots.root();
    assert_eq!(view.rect.size(), egui::vec2(28., 28.));
    assert!(js(&root,"const a=value.getBoundingClientRect(), b=value.querySelector('.test-slot').getBoundingClientRect(); return Math.abs(b.top-a.top-8)<0.1 && Math.abs(a.right-b.right-8)<0.1").as_bool().unwrap());
    support::reading_paint::observe("link-hover").await;
    assert!(
        js(
            &root,
            "return value.getAttribute('data-hovered') === 'true'"
        )
        .as_bool()
        .unwrap()
    );
    js(
        &root,
        "const node=value.querySelector('p').firstChild, range=document.createRange(); range.setStart(node,2);range.setEnd(node,10);window.getSelection().removeAllRanges();window.getSelection().addRange(range);value.parentElement.scrollTop=12.5;return true",
    );
    let saved = snapshot(&root);
    let sizes = [(egui::Id::new("link-slot"), egui::vec2(28., 28.))];
    view.slots.publish(egui::vec2(300., 80.), &sizes);
    view.frame(vec![]);
    unchanged(&saved);
    assert!(
        !js(
            &view.slots.frame(),
            "return getComputedStyle(value).clipPath === 'none'"
        )
        .as_bool()
        .unwrap()
    );
    let full = view.rect;
    view.slots.clip = egui::Rect::from_min_max(full.center(), full.max);
    view.frame(pointer(full.min, true));
    view.frame(pointer(full.center(), false));
    fixture.begin(
        "pending",
        "https://example.test/view?leafId=branch&targetId=native+%E9%9B%AA",
    );
    view.slots.clip = egui::Rect::EVERYTHING;
    focus_and_activate(&mut view, &root);
    fixture.immediate("pending");
    let host = std::rc::Rc::downgrade(&view.slots.host);
    drop(view);
    fixture.resolve_pending();
    // A native frame follows settlement of the ready local microtask queue.
    let settled = js(&JsValue::NULL, "return new Promise(requestAnimationFrame)")
        .dyn_into::<js_sys::Promise>()
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(settled).await.unwrap();
    fixture.finish(true);
    assert!(host.upgrade().is_none());
    assert!(!root.is_connected());
}

/// Reveal the actual native slot on toolkit focus, then activate with the keyboard.
fn focus_and_activate(view: &mut Composition, root: &web_sys::Element) {
    js(
        root,
        "window.linkReveal=0;const slot=value.querySelector('.test-slot'); const original=slot.scrollIntoView;slot.scrollIntoView=function(...args){window.linkReveal++;return original.apply(this,args)};return true",
    );
    let response = view.frame(vec![]);
    response.surrender_focus();
    view.frame(vec![]);
    let focused = view.frame(vec![egui::Event::Key {
        key: egui::Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(focused.has_focus());
    assert_eq!(js(root, "return window.linkReveal").as_f64(), Some(1.));
    view.frame(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
}

/// Native namespaces retained for one browser test transaction.
#[derive(Clone)]
pub struct Fixture {
    /// Original packaged native namespace.
    module0: crate::browser_fixture_support::FixtureModule,
}
impl Fixture {
    /// Load each namespace before installing controlled host effects.
    pub async fn load() -> Self {
        Self {
            module0: crate::browser_fixture_support::FixtureModule::load(&wasm_bindgen::link_to!(
                module = "/tests/session_links_support/clipboard.js"
            ))
            .await
            .unwrap(),
        }
    }
    /// Install controlled effects below the native adapter's wrappers.
    pub fn begin(&self, mode: &str, text: &str) {
        self.module0
            .call("begin", &js_sys::Array::of2(&mode.into(), &text.into()))
            .unwrap();
    }
    /// Check synchronous native admission before polling the future.
    pub fn immediate(&self, mode: &str) {
        self.module0
            .call("immediate", &js_sys::Array::of1(&mode.into()))
            .unwrap();
    }
    /// Assert resource cleanup and restore every patched native operation.
    pub fn finish(&self, success: bool) {
        self.module0
            .call("finish", &js_sys::Array::of1(&success.into()))
            .unwrap();
    }
    /// Fulfill the retained native clipboard request after owner release.
    pub fn resolve_pending(&self) {
        self.module0
            .call("resolvePending", &js_sys::Array::new())
            .unwrap();
    }
}
