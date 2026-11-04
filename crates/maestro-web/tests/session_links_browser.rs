//! Real-document clipboard effects and native message-slot composition.
#![cfg(test)]
#![cfg(target_arch = "wasm32")]
use crate::browser_fixture_support::{Patch, dom_exception, error, get, global, object};
use crate::reading_slots_support;
use crate::support;
use crate::support::pointer::pointer;
use maestro_web::browser::attachment_content::{AttachmentHost, BrowserAttachmentHost};
use reading_slots_support::{js, snapshot, unchanged};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, closure::Closure, prelude::*};

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
    let fixture = Fixture::default();
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
    let fixture = Fixture::default();
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
    let fixture = Fixture::default();
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

/// One controlled clipboard transaction at a time below the native adapter's wrappers.
#[derive(Clone, Default)]
pub struct Fixture {
    /// The running transaction, if any.
    active: Rc<RefCell<Option<Transaction>>>,
}

/// Effects one clipboard transaction observed.
#[derive(Default)]
struct Effects {
    /// Effect names in order.
    events: Vec<&'static str>,
    /// Temporary text areas the page created.
    nodes: Vec<web_sys::HtmlTextAreaElement>,
    /// Timers the page scheduled.
    timers: u32,
    /// Settlement of a pending API write.
    resolve: Option<js_sys::Function>,
}

/// A running transaction: its mode, its effects and the replaced page operations.
struct Transaction {
    /// The refused or altered step, or an ordinary mode.
    mode: String,
    /// Effects observed so far.
    effects: Rc<RefCell<Effects>>,
    /// Replaced page operations, restored when the transaction ends.
    patches: Vec<Patch>,
    /// The controlled API write, alive while installed.
    _write: Closure<dyn Fn(JsValue) -> Result<JsValue, JsValue>>,
}

/// Modes whose clipboard API offers a write.
const API_MODES: [&str; 4] = ["api", "call", "reject", "pending"];

impl Fixture {
    /// Install controlled effects below the native adapter's wrappers.
    pub fn begin(&self, mode: &str, text: &str) {
        assert!(
            self.active.borrow().is_none(),
            "unfinished clipboard transaction"
        );
        let effects: Rc<RefCell<Effects>> = Rc::default();
        let write = api_write(mode, text, &effects);
        let clipboard = object(&[("writeText", write.as_ref())]);
        let (document, window) = (global("document"), global("window"));
        let counted = Rc::clone(&effects);
        let mut patches = vec![
            Patch::method(&window, "setTimeout", move |original, this, args| {
                counted.borrow_mut().timers += 1;
                original.apply(&this, &args)
            }),
            api(&get(&window, "navigator"), mode, clipboard.into()),
            text_areas(&document, mode, &effects),
            appends(&get(&document, "body"), mode, &effects),
        ];
        if mode == "body" {
            patches.push(Patch::getter(&document, "body", |_| Ok(JsValue::NULL)));
        }
        patches.push(copy_command(&document, mode, text, &effects));
        *self.active.borrow_mut() = Some(Transaction {
            mode: mode.to_owned(),
            effects,
            patches,
            _write: write,
        });
    }
    /// Check synchronous native admission before polling the future.
    pub fn immediate(&self, mode: &str) {
        let active = self.active.borrow();
        let effects = active.as_ref().unwrap().effects.borrow();
        let api = API_MODES.contains(&mode);
        assert!(
            !api || effects.events.first() == Some(&"api"),
            "API deferred behind local dispatch"
        );
        assert!(
            api || ["create", "getter"].contains(&mode) || effects.nodes.len() == 1,
            "synchronous fallback deferred"
        );
    }
    /// Assert resource cleanup and restore every patched native operation.
    pub fn finish(&self, success: bool) {
        let transaction = self.active.borrow_mut().take().unwrap();
        drop(transaction.patches);
        let (mode, effects) = (transaction.mode.as_str(), transaction.effects.borrow());
        assert!(
            effects.nodes.iter().all(|node| !node.is_connected()),
            "temporary selection leaked"
        );
        assert!(
            !["api", "pending"].contains(&mode) || effects.nodes.is_empty(),
            "fallback after API success"
        );
        let expected = !["false", "create", "body", "append", "exec"].contains(&mode);
        assert_eq!(success, expected, "wrong native success result");
        assert_eq!(effects.timers, 0, "feedback admitted without live control");
        let expected_events: &[&str] = match mode {
            "api" | "pending" => &["api"],
            "create" => &[],
            "body" | "append" => &["create"],
            "call" | "reject" => &["api", "create", "append", "exec"],
            _ => &["create", "append", "exec"],
        };
        assert_eq!(effects.events, expected_events, "wrong effect sequence");
        let order = effects.events.join(" -> ");
        web_sys::console::log_1(&format!("PASS clipboard {mode}: {order}").into());
    }
    /// Fulfill the retained native clipboard request after owner release.
    pub fn resolve_pending(&self) {
        let active = self.active.borrow();
        let effects = active.as_ref().unwrap().effects.borrow();
        effects
            .resolve
            .as_ref()
            .unwrap()
            .call0(&JsValue::UNDEFINED)
            .unwrap();
    }
}

/// The controlled API write: it records the call and settles as `mode` selects.
fn api_write(
    mode: &str,
    text: &str,
    effects: &Rc<RefCell<Effects>>,
) -> Closure<dyn Fn(JsValue) -> Result<JsValue, JsValue>> {
    let (mode, text, effects) = (mode.to_owned(), text.to_owned(), Rc::clone(effects));
    Closure::new(move |value: JsValue| {
        effects.borrow_mut().events.push("api");
        if value.as_string().as_deref() != Some(text.as_str()) {
            return Err(error("wrong API text"));
        }
        Ok(match mode.as_str() {
            "call" => return Err(dom_exception("controlled call", "Error")),
            "reject" => js_sys::Promise::reject(&dom_exception("controlled rejection", "Error")),
            "pending" => js_sys::Promise::new(&mut |resolve, _| {
                effects.borrow_mut().resolve = Some(resolve);
            }),
            _ => js_sys::Promise::resolve(&JsValue::UNDEFINED),
        }
        .into())
    })
}

/// The page clipboard as `mode` presents it.
fn api(navigator: &JsValue, mode: &str, clipboard: JsValue) -> Patch {
    let mode = mode.to_owned();
    Patch::getter(navigator, "clipboard", move |_| match mode.as_str() {
        "getter" => Err(dom_exception("controlled getter", "Error")),
        "method" => Ok(js_sys::Object::new().into()),
        api if API_MODES.contains(&api) => Ok(clipboard.clone()),
        _ => Ok(JsValue::UNDEFINED),
    })
}

/// Record created text areas, refusing creation in `create` mode.
fn text_areas(document: &JsValue, mode: &str, effects: &Rc<RefCell<Effects>>) -> Patch {
    let (refuse, effects) = (mode == "create", Rc::clone(effects));
    Patch::method(document, "createElement", move |original, this, args| {
        let text_area = args.get(0) == "textarea";
        if text_area && refuse {
            return Err(dom_exception("controlled create", "Error"));
        }
        let node = original.apply(&this, &args)?;
        if text_area {
            let mut effects = effects.borrow_mut();
            effects.nodes.push(node.clone().unchecked_into());
            effects.events.push("create");
        }
        Ok(node)
    })
}

/// Record text-area appends to the body, refusing them in `append` mode.
fn appends(body: &JsValue, mode: &str, effects: &Rc<RefCell<Effects>>) -> Patch {
    let (refuse, effects) = (mode == "append", Rc::clone(effects));
    Patch::method(body, "appendChild", move |original, this, args| {
        if get(&args.get(0), "localName") == "textarea" {
            if refuse {
                return Err(dom_exception("controlled append", "Error"));
            }
            effects.borrow_mut().events.push("append");
        }
        original.apply(&this, &args)
    })
}

/// The copy command, requiring the selected temporary text area's exact operands.
fn copy_command(
    document: &JsValue,
    mode: &str,
    text: &str,
    effects: &Rc<RefCell<Effects>>,
) -> Patch {
    let (mode, text, effects) = (mode.to_owned(), text.to_owned(), Rc::clone(effects));
    let document_node: web_sys::Document = document.clone().unchecked_into();
    let command =
        Closure::<dyn Fn(String) -> Result<bool, JsValue>>::new(move |command: String| {
            let node = effects.borrow().nodes.last().cloned().unwrap();
            let focused = document_node
                .active_element()
                .is_some_and(|active| active.is_same_node(Some(&node)));
            let length = u32::try_from(text.encode_utf16().count()).unwrap();
            let style = node.style();
            let selected = command == "copy"
                && focused
                && node.value() == text
                && node.selection_start().unwrap() == Some(0)
                && node.selection_end().unwrap() == Some(length)
                && style.get_property_value("position").unwrap() == "fixed"
                && style.get_property_value("opacity").unwrap() == "0";
            if !selected {
                return Err(error("native selection operands"));
            }
            effects.borrow_mut().events.push("exec");
            match mode.as_str() {
                "exec" => return Err(dom_exception("controlled exec", "Error")),
                "removed" => node.remove(),
                _ => {}
            }
            Ok(mode != "false")
        });
    Patch::value(document, "execCommand", command.as_ref()).keeping(command)
}
