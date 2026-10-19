//! Actual native-frame witnesses for streaming publication and cancellation.
use super::{edit, message};
use crate::reading_slots_support::{Slots, js};
use maestro_web::browser::attachment_content::AttachmentHost;
use maestro_web::tools::types::ToolRenderView;
use maestro_web::{
    I18n, I18nDiagnostic, I18nError, LanguageHost, StreamingMessageContainer, StreamingMessageProps,
};
use std::rc::Rc;
use wasm_bindgen::{JsValue, prelude::*};

/// English localization without browser persistence.
struct Language;
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
/// A real browser host and drawing context, without mounting the slot fixture.
struct Scene {
    /// Existing actual-document host harness.
    slots: Slots,
    /// Localization of builtin children.
    i18n: I18n,
}
impl Scene {
    /// Construct the same production registry used by standalone reading widgets.
    fn new() -> Self {
        Self {
            slots: Slots::new(),
            i18n: I18n::new(Rc::new(Language)).unwrap(),
        }
    }
    /// Bind an independent streaming subtree to the actual native frame adapter.
    fn container(&self) -> StreamingMessageContainer {
        StreamingMessageContainer::new(self.slots.host.clone())
    }
    /// Publish the current assistant into actual native DOM and read visible text.
    fn draw(&self, container: &mut StreamingMessageContainer) -> String {
        self.draw_props(container, StreamingMessageProps::default())
    }
    /// Draw one caller-selected policy without changing its operands.
    fn draw_props(
        &self,
        container: &mut StreamingMessageContainer,
        props: StreamingMessageProps<'_>,
    ) -> String {
        let action = props.on_cost_click;
        let host: Rc<dyn AttachmentHost> = self.slots.host.clone();
        let p = self.slots.pack.presentation(self.slots.mode).unwrap();
        let mut output = self.slots.context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                ..Default::default()
            },
            |ui| {
                let result = container.render(
                    ui,
                    StreamingMessageProps {
                        tools: props.tools,
                        is_streaming: props.is_streaming,
                        pending_tool_calls: props.pending_tool_calls,
                        tool_results_by_id: props.tool_results_by_id,
                        on_cost_click: action.clone(),
                    },
                    ToolRenderView {
                        host: &host,
                        i18n: &self.i18n,
                        presentation: &p,
                        highlighter: &crate::highlighter_support::NoHighlight,
                        modal: None,
                    },
                );
                assert!(result.overlays.is_empty());
                result.response.unwrap();
            },
        );
        output.textures_delta.clear();
        js(&JsValue::NULL, "return document.body.innerText")
            .as_string()
            .unwrap()
    }
}
/// Wait for actual browser delivery, not a timer or a fixed delay.
async fn frame() {
    let promise = js(
        &JsValue::NULL,
        "return new Promise(resolve=>requestAnimationFrame(()=>resolve()))",
    );
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
        .await
        .unwrap();
}
/// Two source updates become one owned deep snapshot on the actual next frame.
/// # Panics
/// Panics if deferred text/arguments publish too early or keep aliasing the source.
#[wasm_bindgen]
pub async fn streaming_updates_coalesce_nested_edits_at_the_next_frame() {
    let scene = Scene::new();
    let mut container = scene.container();
    container
        .set_message(Some(message("obsolete")), false)
        .unwrap();
    let latest = message("latest");
    container.set_message(Some(latest.clone()), false).unwrap();
    assert!(!scene.draw(&mut container).contains("latest"));
    edit(&latest, "before-frame");
    frame().await;
    edit(&latest, "after-frame");
    let text = scene.draw(&mut container);
    assert!(text.contains("before-frame"));
    assert!(text.contains("\"value\": \"before-frame\""));
    assert!(!text.contains("after-frame") && !text.contains("obsolete"));
}
/// Retired callbacks cannot overwrite immediate state or affect another instance.
/// # Panics
/// Panics if a cancelled publication is delivered or another frame is cancelled.
#[wasm_bindgen]
pub async fn streaming_clear_cancels_only_its_own_pending_update() {
    let scene = Scene::new();
    let mut first = scene.container();
    let mut second = scene.container();
    first.set_message(Some(message("retired")), false).unwrap();
    second
        .set_message(Some(message("independent")), false)
        .unwrap();
    first.set_message(None, true).unwrap();
    assert!(scene.draw(&mut first).is_empty());
    frame().await;
    assert!(scene.draw(&mut first).is_empty());
    assert!(scene.draw(&mut second).contains("independent"));
    drop(second);
    first
        .set_message(Some(message("retired-again")), false)
        .unwrap();
    first.set_message(Some(message("immediate")), true).unwrap();
    assert!(scene.draw(&mut first).contains("immediate"));
    frame().await;
    let text = scene.draw(&mut first);
    assert!(text.contains("immediate") && !text.contains("retired"));
}
/// Clear cannot starve a subsequent response's first native frame.
/// # Panics
/// Panics if either independent response waits for a second update to appear.
#[wasm_bindgen]
pub async fn streaming_clear_keeps_the_next_turn_first_frame() {
    let scene = Scene::new();
    let mut instances = [scene.container(), scene.container()];
    for (index, container) in instances.iter_mut().enumerate() {
        if index == 1 {
            container
                .set_message(Some(message("retired")), false)
                .unwrap();
        }
        container.set_message(None, true).unwrap();
        container
            .set_message(Some(message("first-frame")), false)
            .unwrap();
    }
    frame().await;
    for container in &mut instances {
        assert!(scene.draw(container).contains("first-frame"));
        container
            .set_message(Some(message("second-update")), false)
            .unwrap();
    }
    frame().await;
    for container in &mut instances {
        assert!(scene.draw(container).contains("second-update"));
    }
}
/// Immediate state remains a live shared handle through an unrelated clear.
/// # Panics
/// Panics if rendering copies an immediate source or cross-contaminates instances.
#[wasm_bindgen]
pub async fn streaming_immediate_updates_and_instances_stay_independent() {
    let scene = Scene::new();
    let mut first = scene.container();
    let mut second = scene.container();
    let source = message("immediate");
    first.set_message(Some(source.clone()), true).unwrap();
    assert!(scene.draw(&mut first).contains("immediate"));
    edit(&source, "retained-handle");
    second
        .set_message(Some(message("unrelated")), false)
        .unwrap();
    second.set_message(None, true).unwrap();
    frame().await;
    let text = scene.draw(&mut first);
    assert!(text.contains("retained-handle") && !text.contains("unrelated"));
    drop(first);
    assert!(scene.draw(&mut second).is_empty());
}

/// User, attachment-user, tool-result and custom roles remain outside the streaming subtree.
fn suppressed() -> Vec<
    maestro_app::AgentMessage<
        maestro_app::browser::components::messages::CustomAgentMessages<
            maestro_app::NoCustomMessages,
        >,
    >,
> {
    use maestro_app::browser::components::messages::{ArtifactAction, CustomAgentMessages};
    use maestro_app::{AgentMessage, UserContent, UserMessage};
    use std::sync::{Arc, RwLock};
    vec![
        AgentMessage::User(Arc::new(RwLock::new(UserMessage {
            content: UserContent::Text("suppressed-user".into()),
            timestamp: 0.,
        }))),
        AgentMessage::Custom(Arc::new(RwLock::new(
            CustomAgentMessages::UserWithAttachments(maestro_web::UserMessageWithAttachments {
                content: UserContent::Text("suppressed-attachment-user".into()),
                timestamp: 0.,
                attachments: None,
            }),
        ))),
        AgentMessage::ToolResult(Arc::new(RwLock::new(crate::renderer_recipes::message(
            None,
        )))),
        AgentMessage::Custom(Arc::new(RwLock::new(CustomAgentMessages::Artifact(
            maestro_web::ArtifactMessage {
                action: ArtifactAction::Create,
                filename: "suppressed-artifact".into(),
                content: None,
                title: None,
                timestamp: "0".into(),
            },
        )))),
    ]
}
/// Every role/streaming branch keeps the source's cursor and assistant policy.
/// # Panics
/// Panics if a suppressed role mounts, a cursor survives idle, or forwarded operands differ.
#[wasm_bindgen]
pub fn streaming_render_branches_keep_pending_and_assistant_states() {
    use std::{
        cell::Cell,
        collections::{HashMap, HashSet},
    };
    let scene = Scene::new();
    let mut container = scene.container();
    let calls = forwarded_calls();
    let hits = Rc::new(Cell::new(0));
    let clicked = hits.clone();
    let action: Rc<dyn Fn()> = Rc::new(move || clicked.set(clicked.get() + 1));
    let tools = ["controlled".into()];
    let pending = HashSet::from(["call".into()]);
    let mut result = crate::renderer_recipes::message(None);
    result.timestamp = 42.;
    let results = HashMap::from([("call".into(), result)]);
    for streaming in [false, true] {
        role_branches(
            &scene,
            &mut container,
            StreamingMessageProps {
                tools: &tools,
                is_streaming: streaming,
                tool_results_by_id: Some(&results),
                on_cost_click: Some(action.clone()),
                ..Default::default()
            },
        );
    }
    assert_eq!(hits.get(), 1);
    container
        .set_message(Some(message("assistant-branch")), true)
        .unwrap();
    scene.draw_props(
        &mut container,
        StreamingMessageProps {
            tools: &tools,
            pending_tool_calls: Some(&pending),
            ..Default::default()
        },
    );
    scene.draw_props(
        &mut container,
        StreamingMessageProps {
            tools: &tools,
            ..Default::default()
        },
    );
    assert_eq!(
        *calls.borrow(),
        [
            (false, Some(42.)),
            (true, Some(42.)),
            (true, None),
            (false, None)
        ]
    );
}

/// Same-kind updates retain the assistant's disclosure; clear retires its native subtree.
/// # Panics
/// Panics if expansion resets during updates or survives the next response.
#[wasm_bindgen]
pub async fn streaming_updates_retain_children_until_clear() {
    let scene = Scene::new();
    let mut container = scene.container();
    let source = message("with-thinking");
    let maestro_app::AgentMessage::Assistant(value) = &source else {
        unreachable!()
    };
    value.write().unwrap().content.insert(
        0,
        serde_json::from_value(
            serde_json::json!({"type":"thinking","thinking":"**owned-thought**"}),
        )
        .unwrap(),
    );
    container.set_message(Some(source.clone()), true).unwrap();
    assert!(!scene.draw(&mut container).contains("owned-thought"));
    js(
        &JsValue::NULL,
        "globalThis.thinkingRoot=document.querySelector('.maestro-thinking-header');thinkingRoot.click()",
    );
    assert!(scene.draw(&mut container).contains("owned-thought"));
    {
        let mut source = value.write().unwrap();
        let maestro_app::AssistantContent::Thinking(thinking) = &mut source.content[0] else {
            unreachable!()
        };
        thinking.thinking = "**updated-owned-thought**".into();
    }
    container.set_message(Some(source.clone()), false).unwrap();
    frame().await;
    assert!(scene.draw(&mut container).contains("updated-owned-thought"));
    assert_eq!(js(&JsValue::NULL, "return thinkingRoot.isConnected && thinkingRoot===document.querySelector('.maestro-thinking-header')").as_bool(), Some(true));
    container.set_message(None, true).unwrap();
    assert_eq!(
        js(&JsValue::NULL, "return thinkingRoot.isConnected").as_bool(),
        Some(false)
    );
    assert!(scene.draw(&mut container).is_empty());
    container.set_message(Some(source), false).unwrap();
    frame().await;
    assert!(!scene.draw(&mut container).contains("owned-thought"));
    assert_eq!(
        js(
            &JsValue::NULL,
            "return thinkingRoot!==document.querySelector('.maestro-thinking-header')"
        )
        .as_bool(),
        Some(true)
    );
}

/// Observe operands after actual assistant/tool composition.
type ForwardedCalls = Rc<std::cell::RefCell<Vec<(bool, Option<f64>)>>>;
/// Register the tool callback that records forwarded operands.
fn forwarded_calls() -> ForwardedCalls {
    use maestro_web::tools::types::{ToolRendererBinding, ToolResultMessageView};
    use std::cell::RefCell;
    let calls = Rc::new(RefCell::new(Vec::new()));
    let captured = calls.clone();
    maestro_web::register_tool_renderer(
        "controlled",
        ToolRendererBinding::from_raw(Rc::new(crate::renderer_recipes::Renderer(Rc::new(
            move |args: Option<&maestro_app::JsonObject>,
                  result: Option<ToolResultMessageView<'_>>,
                  streaming| {
                assert_eq!(args.unwrap()["nested"]["value"], "assistant-branch");
                captured
                    .borrow_mut()
                    .push((streaming, result.map(|result| result.timestamp())));
                Ok(crate::renderer_recipes::content("forwarded tool", true))
            },
        )))),
    );
    calls
}
/// Exercise all six roles under one streaming policy.
fn role_branches(
    scene: &Scene,
    container: &mut StreamingMessageContainer,
    props: StreamingMessageProps<'_>,
) {
    let streaming = props.is_streaming;
    container.set_message(None, true).unwrap();
    assert!(
        scene
            .draw_props(
                container,
                StreamingMessageProps {
                    is_streaming: streaming,
                    ..Default::default()
                }
            )
            .is_empty()
    );
    cursor_present(streaming);
    container
        .set_message(Some(message("assistant-branch")), true)
        .unwrap();
    assert!(
        scene
            .draw_props(container, props)
            .contains("assistant-branch")
    );
    cursor_present(streaming);
    assert_eq!(
        js(
            &JsValue::NULL,
            "return !!document.querySelector('.maestro-message-usage')"
        )
        .as_bool(),
        Some(!streaming)
    );
    if !streaming {
        js(
            &JsValue::NULL,
            "document.querySelector('.maestro-message-usage').click()",
        );
    }
    for role in suppressed() {
        container.set_message(Some(role), true).unwrap();
        assert!(
            scene
                .draw_props(
                    container,
                    StreamingMessageProps {
                        is_streaming: streaming,
                        ..Default::default()
                    }
                )
                .is_empty()
        );
        assert_eq!(js(&JsValue::NULL, "return document.querySelectorAll('.maestro-transcript-cursor,.maestro-reading-frame').length").as_f64(), Some(0.));
    }
}
/// Observe the native cursor, not a declared rendering branch.
fn cursor_present(expected: bool) {
    assert_eq!(
        js(
            &JsValue::NULL,
            "return !!document.querySelector('.maestro-transcript-cursor')"
        )
        .as_bool(),
        Some(expected)
    );
}
