//! Real transcript composition with retained native roots and counted recipe drawing.
use super::{check, js};
use crate::reading_slots_support::Slots;
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{Agent, AgentMessage, NoCustomMessages};
use maestro_web::components::markdown_block::MarkdownBlock;
use maestro_web::tools::types::{ToolContent, ToolContentOutput, ToolRenderView, ToolUiError};
use maestro_web::{AgentInterface, I18n, I18nDiagnostic, I18nError, LanguageHost};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::*;

/// Localization without browser persistence.
pub(crate) struct Language;
impl LanguageHost for Language {
    fn stored_language(&self) -> Result<Option<String>, I18nError> {
        Ok(Some("en".into()))
    }
    fn browser_language(&self) -> Result<Option<String>, I18nError> {
        Ok(None)
    }
    fn legacy_language(&self) -> Result<Option<String>, I18nError> {
        Ok(None)
    }
    fn store_language(&self, _: &str) -> Result<(), I18nError> {
        Ok(())
    }
    fn reload(&self) -> Result<(), I18nError> {
        Ok(())
    }
    fn unknown_translation(&self, _: &I18nDiagnostic) {}
}
/// A real native recipe whose canvas invocation is observable independently of native text.
struct Recipe {
    text: String,
    calls: Rc<RefCell<Vec<String>>>,
}
/// Retain the child through clean canvas frames and offscreen reconciliation.
#[derive(Default)]
struct Row {
    block: Option<MarkdownBlock>,
}
impl ToolContent for Recipe {
    fn render(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        state: &maestro_app::ToolRendererState,
    ) -> ToolContentOutput {
        self.calls.borrow_mut().push(self.text.clone());
        let slot = state.state::<Row>();
        let mut row = slot.borrow_mut();
        let block = row
            .block
            .get_or_insert_with(|| MarkdownBlock::new(Rc::clone(view.host)));
        block.content.clone_from(&self.text);
        let response = block
            .render(
                ui,
                maestro_web::components::code_block::CodeBlockView {
                    i18n: view.i18n,
                    presentation: view.presentation,
                    highlighter: view.highlighter,
                    modal: None,
                },
            )
            .map_err(ToolUiError::Reading);
        ToolContentOutput {
            response,
            overlays: vec![],
        }
    }
}
/// Adapt ordinary user messages without inventing a second agent execution family.
pub(crate) struct Renderer(pub(crate) Rc<RefCell<Vec<String>>>);
impl maestro_web::MessageRenderer<BrowserMessages<NoCustomMessages>> for Renderer {
    fn render(
        &self,
        message: &AgentMessage<BrowserMessages<NoCustomMessages>>,
    ) -> Result<Box<dyn ToolContent>, ToolUiError> {
        let AgentMessage::User(message) = message else {
            unreachable!()
        };
        let text = match &message.read().unwrap().content {
            maestro_app::UserContent::Text(text) => text.clone(),
            maestro_app::UserContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|block| {
                    if let maestro_app::UserBlock::Text(text) = block {
                        Some(text.text.as_str())
                    } else {
                        None
                    }
                })
                .collect(),
        };
        Ok(Box::new(Recipe {
            text,
            calls: Rc::clone(&self.0),
        }))
    }
}
/// Frame the public aggregate using the production registry and browser Agent.
pub(crate) fn draw(scene: &Slots, interface: &mut AgentInterface, i18n: &I18n) {
    try_draw(scene, interface, i18n).unwrap();
}
/// Return a reached render failure without hiding its following retry frame.
pub(crate) fn try_draw(
    scene: &Slots,
    interface: &mut AgentInterface,
    i18n: &I18n,
) -> Result<(), ToolUiError> {
    draw_at_height(scene, interface, i18n, 180.)
}
/// Relayout the containing view when the separately owned editor consumes vertical space.
pub(super) fn draw_at_height(
    scene: &Slots,
    interface: &mut AgentInterface,
    i18n: &I18n,
    height: f32,
) -> Result<(), ToolUiError> {
    draw_in_rect(
        scene,
        interface,
        i18n,
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., height)),
    )
}
/// Supply fractional canvas placement independently of the browser canvas's two CSS axes.
pub(crate) fn draw_in_rect(
    scene: &Slots,
    interface: &mut AgentInterface,
    i18n: &I18n,
    rect: egui::Rect,
) -> Result<(), ToolUiError> {
    draw_frame(scene, interface, i18n, rect, vec![]).map(|mut output| {
        output.textures_delta.clear();
    })
}
/// Return the painted frame after delivering actual canvas pointer events.
pub(super) fn draw_frame(
    scene: &Slots,
    interface: &mut AgentInterface,
    i18n: &I18n,
    rect: egui::Rect,
    events: Vec<egui::Event>,
) -> Result<egui::FullOutput, ToolUiError> {
    let mut response = Ok(());
    let host: Rc<dyn maestro_web::browser::attachment_content::AttachmentHost> = scene.host.clone();
    let p = scene.pack.presentation(scene.mode).unwrap();
    let mut output = scene.context.run_ui(
        egui::RawInput {
            screen_rect: Some(rect),
            events,
            ..Default::default()
        },
        |ui| {
            let output = interface.render(
                ui,
                ToolRenderView::new(
                    &host,
                    i18n,
                    &p,
                    &crate::highlighter_support::NoHighlight,
                    None,
                ),
            );
            assert!(output.overlays.is_empty());
            response = output.response.map(|_| ());
        },
    );
    match response {
        Ok(()) => Ok(output),
        Err(error) => {
            output.textures_delta.clear();
            Err(error)
        }
    }
}
/// Every text root stays searchable while actual canvas invocation follows the native scroll.
/// # Panics
/// Panics if native text is detached or clean canvas drawing visits offscreen rows.
#[wasm_bindgen]
pub fn transcript_keeps_all_native_text_searchable_while_canvas_rows_are_culled() {
    let scene = Slots::new();
    let i18n = I18n::new(Rc::new(Language)).unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    maestro_web::register_message_renderer("user", Rc::new(Renderer(Rc::clone(&calls))));
    let session = Rc::new(Agent::<BrowserMessages<NoCustomMessages>>::default());
    session.state().write().unwrap().set_messages(
        (0..40)
            .map(|n| {
                AgentMessage::User(Arc::new(RwLock::new(maestro_app::UserMessage {
                    content: maestro_app::UserContent::Text(if n == 1 || n == 38 {
                        "duplicate row".into()
                    } else {
                        format!("native transcript row {n}")
                    }),
                    timestamp: 0.,
                })))
            })
            .collect(),
    );
    let mut interface = AgentInterface::new(scene.host.clone());
    interface.set_auto_scroll(false);
    interface.set_session(Some(session)).unwrap();
    draw(&scene, &mut interface, &i18n);
    let root = transcript_root();
    check(
        &root,
        "return value.innerText.includes('native transcript row 0') && value.innerText.includes('native transcript row 39') && value.innerText.match(/duplicate row/g).length===2",
    );
    assert_eq!(
        calls.borrow().len(),
        40,
        "one draw per row during reconciliation"
    );
    calls.borrow_mut().clear();
    draw(&scene, &mut interface, &i18n);
    let top = calls.take();
    assert!(top.contains(&"native transcript row 0".into()));
    assert!(!top.contains(&"native transcript row 39".into()));
    assert!(top.len() < 40);
    check(
        &root,
        "return window.find('native transcript row 39',false,false,true,false,false,false)",
    );
    js(
        &root,
        "value.parentElement.scrollTop=value.parentElement.scrollHeight",
    );
    draw(&scene, &mut interface, &i18n);
    let bottom = calls.take();
    assert!(bottom.contains(&"native transcript row 39".into()));
    assert!(!bottom.contains(&"native transcript row 0".into()));
    check(
        &root,
        "return value.innerText.match(/duplicate row/g).length===2 && value.firstElementChild.isConnected && value.lastElementChild.isConnected",
    );
}

/// Qualify planned placement with variable heights and one native sizing publication.
/// # Panics
/// Panics if row work rescans, actions render twice or native sizes publish per child.
#[wasm_bindgen]
pub fn transcript_registry_planned_rows_publish_sizes_once_with_linear_work() {
    for count in [40, 80] {
        let scene = Slots::new();
        let i18n = I18n::new(Rc::new(Language)).unwrap();
        let calls = Rc::new(RefCell::new(Vec::new()));
        maestro_web::register_message_renderer("user", Rc::new(Renderer(calls.clone())));
        let session = Rc::new(Agent::<BrowserMessages<NoCustomMessages>>::default());
        session.state().write().unwrap().set_messages(
            (0..count)
                .map(|n| {
                    AgentMessage::User(Arc::new(RwLock::new(maestro_app::UserMessage {
                        content: maestro_app::UserContent::Text(format!(
                            "variable row {n}{}",
                            "\n\nparagraph".repeat(n % 3)
                        )),
                        timestamp: 0.,
                    })))
                })
                .collect(),
        );
        let mut interface = AgentInterface::new(scene.host.clone());
        interface.set_auto_scroll(false);
        interface.set_session(Some(session)).unwrap();
        begin_slot_writes();
        draw(&scene, &mut interface, &i18n);
        let writes = end_slot_writes();
        assert_eq!(writes, count);
        assert_eq!(
            calls.borrow().len(),
            count,
            "each recipe is invoked only once"
        );
        let root = transcript_root();
        check(
            &root,
            "const slots=[...value.querySelectorAll('.maestro-transcript-slot')];return new Set(slots.map(n=>n.getBoundingClientRect().height)).size>1 && slots.every(slot=>Math.abs(slot.firstElementChild.getBoundingClientRect().top-slot.getBoundingClientRect().top)<0.1)",
        );
        calls.borrow_mut().clear();
        begin_slot_writes();
        draw(&scene, &mut interface, &i18n);
        assert!(calls.borrow().len() < count);
        assert_eq!(end_slot_writes(), 0);
    }
}

/// An offscreen image invalidates only its own native slot before later rows are revealed.
/// # Panics
/// Panics if growth is missed, unchanged slots republish sizes or a later visible row is stale.
#[wasm_bindgen]
pub async fn transcript_registry_resize_reflows_only_the_changed_offscreen_slot() {
    let scene = Slots::new();
    let i18n = I18n::new(Rc::new(Language)).unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    maestro_web::register_message_renderer("user", Rc::new(Renderer(calls.clone())));
    let session = image_session();
    let mut interface = AgentInterface::new(scene.host.clone());
    interface.set_auto_scroll(false);
    interface.set_session(Some(session)).unwrap();
    draw(&scene, &mut interface, &i18n);
    let root = transcript_root();
    let settled = js(
        &root,
        "return value.querySelector('img').decode().then(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))))",
    );
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(settled))
        .await
        .unwrap();
    draw(&scene, &mut interface, &i18n);
    calls.borrow_mut().clear();
    let old_top = last_row_top(&root);
    let resized = js(
        &root,
        "const image=value.querySelector('img');return new Promise(resolve=>{const observed=new ResizeObserver(()=>{observed.disconnect();resolve()});observed.observe(image);image.style.height='200px'})",
    );
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(resized))
        .await
        .unwrap();
    begin_slot_writes();
    draw(&scene, &mut interface, &i18n);
    assert_eq!(end_slot_writes(), 1);
    assert_eq!(
        calls
            .borrow()
            .iter()
            .filter(|text| text.starts_with("![offscreen growth]"))
            .count(),
        1
    );
    assert!(calls.borrow().len() < 40);
    let new_top = last_row_top(&root);
    assert!(new_top > old_top + 100.);
    calls.borrow_mut().clear();
    js(
        &root,
        "value.parentElement.scrollTop=value.parentElement.scrollHeight",
    );
    begin_slot_writes();
    draw(&scene, &mut interface, &i18n);
    assert_eq!(end_slot_writes(), 0);
    assert!(calls.borrow().contains(&"image neighbor 39".into()));
    assert!(
        !calls
            .borrow()
            .iter()
            .any(|text| text.starts_with("![offscreen growth]"))
    );
}

/// Count only persistent sizing of native transcript slots, not child reading publication.
pub(super) fn begin_slot_writes() {
    js(
        &JsValue::NULL,
        "globalThis.slotWrites=0;globalThis.slotMounts=0;globalThis.slotSetter=Element.prototype.setAttribute;Element.prototype.setAttribute=function(k,v){if(k==='class' && v==='maestro-transcript-slot')slotMounts++;if(k==='style' && this.classList.contains('maestro-transcript-slot'))slotWrites++;return slotSetter.call(this,k,v)}",
    );
}
/// Restore the native method even when the caller needs further frames.
pub(super) fn end_slot_writes() -> usize {
    js(
        &JsValue::NULL,
        "Element.prototype.setAttribute=slotSetter;delete globalThis.slotSetter;return slotWrites.toString()",
    )
    .as_string()
    .unwrap().parse().unwrap()
}

/// Build real Agent history with one offscreen image between ordinary native-text neighbors.
fn image_session() -> Rc<Agent<BrowserMessages<NoCustomMessages>>> {
    let session = Rc::new(Agent::<BrowserMessages<NoCustomMessages>>::default());
    session.state().write().unwrap().set_messages((0..40).map(|n| AgentMessage::User(Arc::new(RwLock::new(maestro_app::UserMessage {
        content:maestro_app::UserContent::Text(if n==20 { "![offscreen growth](data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7)".into() } else { format!("image neighbor {n}") }),timestamp:0.,
    })))).collect());
    session
}

/// Observe the later native row directly, without deriving it from declared slot sizes.
fn last_row_top(root: &JsValue) -> f64 {
    js(
        root,
        "return value.querySelectorAll('.maestro-transcript-slot')[39].getBoundingClientRect().top",
    )
    .as_f64()
    .unwrap()
}

/// Locate the public containing native root in the actual runner document.
pub(crate) fn transcript_root() -> JsValue {
    js(
        &JsValue::NULL,
        "return document.querySelector('.maestro-transcript')",
    )
}
