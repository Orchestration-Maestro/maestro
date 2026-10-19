//! Stable list policy and actual native usage activation through unequal siblings.
use crate::reading_slots_support::{Slots, js};
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{AgentMessage, NoCustomMessages};
use maestro_web::browser::attachment_content::AttachmentHost;
use maestro_web::tools::types::{ToolRenderView, ToolRendererBinding, ToolResultMessageView};
use maestro_web::{MessageList, MessageListProps};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::*;

/// Calls observed after actual assistant/tool composition.
type Calls = Rc<RefCell<Vec<(String, Option<f64>, bool)>>>;
/// Record the actual tool operands rather than duplicating assistant visibility selection.
fn forwarded() -> Calls {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let captured = calls.clone();
    maestro_web::register_tool_renderer(
        "stable-policy",
        ToolRendererBinding::from_raw(Rc::new(crate::renderer_recipes::Renderer(Rc::new(
            move |args: Option<&maestro_app::JsonObject>,
                  result: Option<ToolResultMessageView<'_>>,
                  streaming| {
                captured.borrow_mut().push((
                    args.unwrap()["marker"].as_str().unwrap().to_owned(),
                    result.map(|value| value.timestamp()),
                    streaming,
                ));
                Ok(crate::renderer_recipes::content("stable tool", true))
            },
        )))),
    );
    calls
}
/// Produce independent native cost actions without borrowing either sibling's owner.
fn action(counter: Rc<Cell<usize>>) -> Rc<dyn Fn()> {
    Rc::new(move || counter.set(counter.get() + 1))
}
/// Ordinary browser message history.
type Messages = [AgentMessage<BrowserMessages<NoCustomMessages>>];
/// Draw both policies in one browser frame with their own callback owners.
fn draw_siblings(
    scene: &Slots,
    lists: &mut [MessageList; 2],
    (messages, pending, tools): (&Messages, &HashSet<String>, &[String]),
    (i18n, policies, hits): (&maestro_web::I18n, [bool; 2], &[Rc<Cell<usize>>; 2]),
) {
    let host: Rc<dyn AttachmentHost> = scene.host.clone();
    let p = scene.pack.presentation(scene.mode).unwrap();
    scene
        .context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                ..Default::default()
            },
            |ui| {
                for (index, list) in lists.iter_mut().enumerate() {
                    list.render(
                        ui,
                        MessageListProps {
                            messages,
                            tools,
                            pending_tool_calls: Some(pending),
                            is_streaming: policies[index],
                            on_cost_click: Some(action(hits[index].clone())),
                        },
                        ToolRenderView::new(
                            &host,
                            i18n,
                            &p,
                            &crate::highlighter_support::NoHighlight,
                            None,
                        ),
                    )
                    .response
                    .unwrap();
                }
            },
        )
        .textures_delta
        .clear();
}
/// Exercise pending visibility, completed pairing and usage actions without inherited sibling policy.
/// # Panics
/// Panics if a stable assistant receives streaming policy or another list's operands/actions.
#[wasm_bindgen]
pub fn stable_rows_forward_tool_visibility_and_cost_actions() {
    let scene = Slots::new();
    let i18n =
        maestro_web::I18n::new(Rc::new(crate::transcript_browser::aggregate::Language)).unwrap();
    let host: Rc<dyn AttachmentHost> = scene.host.clone();
    let calls = forwarded();
    let messages: Vec<AgentMessage<BrowserMessages<NoCustomMessages>>> = vec![
        AgentMessage::Assistant(Arc::new(RwLock::new(serde_json::from_value(serde_json::json!({
            "role":"assistant","content":[
                {"type":"toolCall","id":"waiting","name":"stable-policy","arguments":{"marker":"waiting"}},
                {"type":"toolCall","id":"finished","name":"stable-policy","arguments":{"marker":"finished"}}],
            "api":"openai-completions","provider":"openai","model":"controlled","stopReason":"stop","timestamp":0,
            "usage":{"input":3,"output":2,"cacheRead":0,"cacheWrite":0,"totalTokens":5,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}
        })).unwrap()))),
        AgentMessage::ToolResult(Arc::new(RwLock::new(maestro_app::ToolResultMessage {
            tool_call_id:"finished".into(), tool_name:"stable-policy".into(), timestamp:42.,
            content:vec![], details:None, is_error:false,
        }))),
    ];
    let pending = HashSet::from(["waiting".into()]);
    let tools = ["stable-policy".into()];
    let mut lists = [
        MessageList::new(host.clone()),
        MessageList::new(host.clone()),
    ];
    let hits = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
    for policies in [[true, false], [false, true]] {
        calls.borrow_mut().clear();
        draw_siblings(
            &scene,
            &mut lists,
            (&messages, &pending, &tools),
            (&i18n, policies, &hits),
        );
        let expected = if policies[0] {
            vec![
                ("finished".into(), Some(42.), false),
                ("waiting".into(), None, true),
                ("finished".into(), Some(42.), false),
            ]
        } else {
            vec![
                ("waiting".into(), None, true),
                ("finished".into(), Some(42.), false),
                ("finished".into(), Some(42.), false),
            ]
        };
        assert_eq!(*calls.borrow(), expected);
        assert_eq!(js(&JsValue::NULL,"const usage=document.querySelectorAll('.maestro-message-usage');usage[0].click();usage[1].click();return usage.length").as_f64(),Some(2.));
    }
    assert_eq!([hits[0].get(), hits[1].get()], [2, 2]);
}
