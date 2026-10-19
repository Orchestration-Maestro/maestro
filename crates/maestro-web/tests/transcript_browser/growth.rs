//! Native growth and reader-follow transitions through the containing transcript.
use super::aggregate::{Language, Renderer, draw, draw_at_height, transcript_root};
use super::{check, js};
use crate::reading_slots_support::Slots;
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{Agent, AgentMessage, NoCustomMessages, ToolResultMessage, UserBlock};
use maestro_web::{AgentInterface, I18n};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::*;

/// Browser callback delivery, without an elapsed-time guess.
async fn frame() {
    let promise = js(
        &JsValue::NULL,
        "return new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))",
    );
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
        .await
        .unwrap();
}
/// Set a real offset and dispatch the same native scroll notification as the reader.
fn scroll(root: &JsValue, expression: &str) {
    js(
        root,
        &format!(
            "const v=value.parentElement;v.scrollTop={expression};v.dispatchEvent(new Event('scroll'))"
        ),
    );
}
/// Read the native offset and total height, not a requested target.
fn metrics(root: &JsValue) -> (f64, f64) {
    (
        js(root, "return value.parentElement.scrollTop")
            .as_f64()
            .unwrap(),
        js(root, "return value.parentElement.scrollHeight")
            .as_f64()
            .unwrap(),
    )
}
/// Check final native offsets after each independent growth source.
fn grew(root: &JsValue, before: (f64, f64), following: bool) {
    let after = metrics(root);
    assert!(
        after.1 > before.1,
        "content must actually grow: {before:?} -> {after:?}"
    );
    if following {
        check(
            root,
            "const v=value.parentElement;return Math.abs(v.scrollHeight-v.clientHeight-v.scrollTop)<1 && v.scrollTop>0",
        );
        assert!(after.0 > before.0);
    } else {
        assert_eq!(
            after.0.to_bits(),
            before.0.to_bits(),
            "reader-away offset must survive growth"
        );
    }
}
/// Owned history with offscreen image content and a real builtin command-result renderer.
struct History {
    agent: Rc<Agent<BrowserMessages<NoCustomMessages>>>,
    user: Arc<RwLock<maestro_app::UserMessage>>,
    result: Arc<RwLock<ToolResultMessage>>,
}
impl History {
    /// Put growth above the viewport so browser anchoring cannot disguise a follow jump.
    fn new() -> Self {
        let agent = Rc::new(Agent::<BrowserMessages<NoCustomMessages>>::default());
        let user = Arc::new(RwLock::new(maestro_app::UserMessage {
            content: maestro_app::UserContent::Text("![growth](data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7)".into()), timestamp: 0.,
        }));
        let assistant = maestro_app::faux_assistant_message(
            vec![maestro_app::AssistantContent::ToolCall(
                maestro_app::ToolCall {
                    id: "growth-call".into(),
                    name: "bash".into(),
                    arguments: serde_json::json!({"command":"controlled output"})
                        .as_object()
                        .unwrap()
                        .clone(),
                    thought_signature: None,
                },
            )],
            maestro_app::FauxAssistantMessageOptions::default(),
        );
        let result = Arc::new(RwLock::new(ToolResultMessage {
            tool_call_id: "growth-call".into(),
            tool_name: "bash".into(),
            content: vec![text("initial tool result")],
            details: None,
            is_error: false,
            timestamp: 0.,
        }));
        let mut messages = vec![
            AgentMessage::User(user.clone()),
            AgentMessage::Assistant(Arc::new(RwLock::new(assistant))),
            AgentMessage::ToolResult(result.clone()),
        ];
        messages.extend((0..30).map(|n| {
            AgentMessage::User(Arc::new(RwLock::new(maestro_app::UserMessage {
                content: maestro_app::UserContent::Text(format!("reader neighbor {n}")),
                timestamp: 0.,
            })))
        }));
        agent.state().write().unwrap().set_messages(messages);
        Self {
            agent,
            user,
            result,
        }
    }
    /// Reuse the same message handles while admitting a public aggregate refresh.
    fn refresh(&self, interface: &mut AgentInterface) {
        interface.set_session(Some(self.agent.clone())).unwrap();
    }
    /// Append real markdown paragraphs without replacing the neighboring image.
    fn grow_content(&self) {
        let mut user = self.user.write().unwrap();
        let maestro_app::UserContent::Text(content) = &mut user.content else {
            unreachable!()
        };
        content.push_str(&"\n\nnew native paragraph".repeat(8));
    }
}
/// A real command-output block, consumed through the stable assistant's shared result index.
fn text(value: &str) -> UserBlock {
    UserBlock::Text(maestro_app::TextContent {
        text: value.into(),
        text_signature: None,
    })
}

/// Content, tool and image growth follow only after a real near-bottom return; editor shrink preserves policy.
/// # Panics
/// Panics if final offsets ignore reader movement, growth or editor relayout.
#[wasm_bindgen]
pub async fn transcript_growth_follows_only_when_enabled() {
    for following in [false, true] {
        let scene = Slots::new();
        let i18n = I18n::new(Rc::new(Language)).unwrap();
        maestro_web::register_message_renderer(
            "user",
            Rc::new(Renderer(Rc::new(RefCell::new(vec![])))),
        );
        let history = History::new();
        let mut interface = AgentInterface::new(scene.host.clone());
        history.refresh(&mut interface);
        draw(&scene, &mut interface, &i18n);
        let root = transcript_root();
        let decoded = js(&root, "return value.querySelector('img').decode()");
        wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(decoded))
            .await
            .unwrap();
        frame().await;
        draw(&scene, &mut interface, &i18n);
        scroll(&root, "v.scrollHeight");
        draw(&scene, &mut interface, &i18n);
        scroll(&root, "v.scrollTop-200");
        draw(&scene, &mut interface, &i18n);
        if following {
            scroll(&root, "v.scrollHeight-v.clientHeight-5");
            draw(&scene, &mut interface, &i18n);
        }
        frame().await;
        draw(&scene, &mut interface, &i18n);
        let before = metrics(&root);
        history.grow_content();
        history.refresh(&mut interface);
        draw(&scene, &mut interface, &i18n);
        grew(&root, before, following);
        frame().await;
        draw(&scene, &mut interface, &i18n);
        let before = metrics(&root);
        history.result.write().unwrap().content = vec![text(&"tool growth\n".repeat(12))];
        history.refresh(&mut interface);
        draw(&scene, &mut interface, &i18n);
        check(&root, "return value.innerText.includes('tool growth')");
        grew(&root, before, following);
        frame().await;
        draw(&scene, &mut interface, &i18n);
        let before = metrics(&root);
        let resized = js(
            &root,
            "const image=value.querySelector('img');return new Promise(resolve=>{const observer=new ResizeObserver(()=>{observer.disconnect();resolve()});observer.observe(image);image.style.height='200px'})",
        );
        wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(resized))
            .await
            .unwrap();
        draw(&scene, &mut interface, &i18n);
        grew(&root, before, following);
        frame().await;
        draw(&scene, &mut interface, &i18n);
        editor_shrink(&scene, &mut interface, &i18n, &history, following);
    }
}
/// The separately owned editor changes available height, not transcript follow preference.
fn editor_shrink(
    scene: &Slots,
    interface: &mut AgentInterface,
    i18n: &I18n,
    history: &History,
    following: bool,
) {
    let root = transcript_root();
    let before = metrics(&root).0;
    let height = js(&root, "return value.parentElement.clientHeight")
        .as_f64()
        .unwrap();
    draw_at_height(scene, interface, i18n, 140.).unwrap();
    check(
        &root,
        &format!("return value.parentElement.clientHeight<{height}"),
    );
    scroll(&root, "v.scrollTop");
    draw_at_height(scene, interface, i18n, 140.).unwrap();
    if following {
        check(
            &root,
            "const v=value.parentElement;return Math.abs(v.scrollHeight-v.clientHeight-v.scrollTop)<1",
        );
    } else {
        assert_eq!(metrics(&root).0.to_bits(), before.to_bits());
    }
    let before = metrics(&root);
    history.grow_content();
    history.refresh(interface);
    draw_at_height(scene, interface, i18n, 140.).unwrap();
    grew(&root, before, following);
}
