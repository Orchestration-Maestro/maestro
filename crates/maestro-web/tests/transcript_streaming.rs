//! Deferred transcript publication through owned frame registrations.
#![cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
use crate::{attachment_support::Frame, effects_support};
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{AgentMessage, AssistantContent, NoCustomMessages};
#[cfg(not(target_arch = "wasm32"))]
use maestro_web::browser::attachment_content::AttachmentHost;
#[cfg(not(target_arch = "wasm32"))]
use maestro_web::tools::types::ToolRenderView;
#[cfg(not(target_arch = "wasm32"))]
use maestro_web::{StreamingMessageContainer, StreamingMessageProps};
#[cfg(not(target_arch = "wasm32"))]
use std::rc::Rc;
use std::sync::{Arc, RwLock};

/// A live assistant source with independently mutable nested arguments.
fn message(text: &str) -> AgentMessage<BrowserMessages<NoCustomMessages>> {
    AgentMessage::Assistant(Arc::new(RwLock::new(serde_json::from_value(serde_json::json!({
        "role":"assistant", "content":[{"type":"text","text":format!("```javascript\n{text}\n```")},
        {"type":"toolCall","id":"call","name":"controlled","arguments":{"nested":{"value":text}}}],
        "api":"openai-completions","provider":"openai","model":"controlled","stopReason":"stop","timestamp":0,
        "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
        "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}
    })).unwrap())))
}
/// Edit the same shared source without replacing its outer identity.
fn edit(message: &AgentMessage<BrowserMessages<NoCustomMessages>>, text: &str) {
    let AgentMessage::Assistant(value) = message else {
        unreachable!()
    };
    let mut value = value.write().unwrap();
    let AssistantContent::Text(content) = &mut value.content[0] else {
        unreachable!()
    };
    content.text = format!("```javascript\n{text}\n```");
    let AssistantContent::ToolCall(call) = &mut value.content[1] else {
        unreachable!()
    };
    call.arguments["nested"]["value"] = text.into();
}
/// Observe actual assistant Markdown rendering instead of private state.
#[cfg(not(target_arch = "wasm32"))]
fn rendered(
    container: &mut StreamingMessageContainer,
    host: &Rc<dyn AttachmentHost>,
) -> Vec<String> {
    let highlighter = crate::recording_highlighter::Highlighter::default();
    let mut frame = Frame::default();
    frame.run(vec![], |ctx, i18n, presentation| {
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("streaming"),
            egui::UiBuilder::new().max_rect(ctx.content_rect()),
        );
        let output = container.render(
            &mut ui,
            StreamingMessageProps::default(),
            ToolRenderView::new(host, i18n, presentation, &highlighter, None),
        );
        assert!(output.overlays.is_empty());
        output.response.unwrap();
    });
    highlighter
        .requests
        .take()
        .into_iter()
        .map(|(text, _)| text)
        .collect()
}
/// Expected rendering includes the deeply owned tool arguments as well as text.
#[cfg(not(target_arch = "wasm32"))]
fn expected(text: &str) -> Vec<String> {
    vec![
        format!("{text}\n"),
        serde_json::to_string_pretty(&serde_json::json!({"nested":{"value":text}})).unwrap(),
    ]
}
/// Two deferred updates coalesce; publication snapshots subsequent nested mutations.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn streaming_updates_coalesce_nested_edits_at_the_next_frame() {
    let clock = Rc::new(effects_support::Host::default());
    let host: Rc<dyn AttachmentHost> = clock.clone();
    let mut container = StreamingMessageContainer::new(host.clone());
    let first = message("first");
    let latest = message("latest");
    container.set_message(Some(first), false).unwrap();
    container.set_message(Some(latest.clone()), false).unwrap();
    assert_eq!(clock.live(), 1);
    assert!(rendered(&mut container, &host).is_empty());
    edit(&latest, "before frame");
    clock.advance(1);
    edit(&latest, "after frame");
    assert_eq!(rendered(&mut container, &host), expected("before frame"));
}
/// Clearing and immediate replacement revoke only the retired instance's frame.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn streaming_clear_cancels_only_its_own_pending_update() {
    let clock = Rc::new(effects_support::Host::default());
    let host: Rc<dyn AttachmentHost> = clock.clone();
    let mut container = StreamingMessageContainer::new(host.clone());
    let mut other = StreamingMessageContainer::new(host.clone());
    container
        .set_message(Some(message("retired")), false)
        .unwrap();
    other.set_message(Some(message("other")), false).unwrap();
    container.set_message(None, true).unwrap();
    assert_eq!(clock.live(), 1);
    assert!(rendered(&mut container, &host).is_empty());
    clock.advance(1);
    assert!(rendered(&mut container, &host).is_empty());
    assert_eq!(rendered(&mut other, &host), expected("other"));
    container
        .set_message(Some(message("retired again")), false)
        .unwrap();
    container
        .set_message(Some(message("immediate")), true)
        .unwrap();
    clock.advance(2);
    assert_eq!(rendered(&mut container, &host), expected("immediate"));
}
/// The first frame of a new response survives clear, with and without retired work.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn streaming_clear_keeps_the_next_turn_first_frame() {
    let clock = Rc::new(effects_support::Host::default());
    let host: Rc<dyn AttachmentHost> = clock.clone();
    let mut instances: Vec<_> = (0..2)
        .map(|_| StreamingMessageContainer::new(host.clone()))
        .collect();
    for (index, container) in instances.iter_mut().enumerate() {
        if index == 1 {
            container
                .set_message(Some(message("retired")), false)
                .unwrap();
        }
        container.set_message(None, true).unwrap();
        container
            .set_message(Some(message("next response")), false)
            .unwrap();
    }
    clock.advance(1);
    for container in &mut instances {
        assert_eq!(rendered(container, &host), expected("next response"));
        container
            .set_message(Some(message("second update")), false)
            .unwrap();
    }
    clock.advance(2);
    for container in &mut instances {
        assert_eq!(rendered(container, &host), expected("second update"));
    }
}
/// Immediate publication retains the supplied handle while another instance changes.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn streaming_immediate_updates_and_instances_stay_independent() {
    let clock = Rc::new(effects_support::Host::default());
    let host: Rc<dyn AttachmentHost> = clock.clone();
    let mut first = StreamingMessageContainer::new(host.clone());
    let mut second = StreamingMessageContainer::new(host.clone());
    let source = message("immediate");
    first.set_message(Some(source.clone()), true).unwrap();
    assert_eq!(clock.live(), 0);
    assert_eq!(rendered(&mut first, &host), expected("immediate"));
    edit(&source, "retained handle");
    second
        .set_message(Some(message("independent")), false)
        .unwrap();
    second.set_message(None, true).unwrap();
    clock.advance(1);
    assert_eq!(rendered(&mut first, &host), expected("retained handle"));
    assert!(rendered(&mut second, &host).is_empty());
}

#[cfg(target_arch = "wasm32")]
#[path = "transcript_streaming/browser.rs"]
mod browser;

/// A custom tool can open an independent overlay before a later tool fails.
#[cfg(not(target_arch = "wasm32"))]
struct OpenOverlay(Rc<std::cell::RefCell<maestro_web::Attachment>>);
#[cfg(not(target_arch = "wasm32"))]
impl maestro_web::tools::types::ToolContent for OpenOverlay {
    fn render(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        _: &maestro_app::ToolRendererState,
    ) -> maestro_web::tools::types::ToolContentOutput {
        maestro_web::tools::types::ToolContentOutput {
            response: Ok(ui.label("opened")),
            overlays: vec![maestro_web::AttachmentOverlay::open(
                self.0.clone(),
                None,
                view.host.clone(),
            )],
        }
    }
}
/// A later child failure must not consume an already-opened independent owner.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn streaming_updates_retain_children_until_clear() {
    let recorded = Rc::new(crate::attachment_support::Host::default());
    let host: Rc<dyn AttachmentHost> = recorded.clone();
    let mut container = StreamingMessageContainer::new(host.clone());
    let record = overlay_renderers();
    let source = message("unused");
    let AgentMessage::Assistant(value) = &source else {
        unreachable!()
    };
    value.write().unwrap().content = ["stream-overlay", "stream-later-failure"]
        .into_iter()
        .map(|name| {
            serde_json::from_value(
                serde_json::json!({"type":"toolCall","id":name,"name":name,"arguments":{}}),
            )
            .unwrap()
        })
        .collect();
    container.set_message(Some(source), true).unwrap();
    let mut frame = Frame::default();
    let mut opened = Vec::new();
    frame.run(vec![], |ctx, i18n, presentation| {
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("stream-overlay"),
            egui::UiBuilder::new().max_rect(ctx.content_rect()),
        );
        let output = container.render(
            &mut ui,
            StreamingMessageProps::default(),
            ToolRenderView::new(
                &host,
                i18n,
                presentation,
                &crate::highlighter_support::NoHighlight,
                None,
            ),
        );
        assert_eq!(
            output.response.unwrap_err().to_string(),
            "later child failure"
        );
        opened.extend(output.overlays);
    });
    assert_eq!(opened.len(), 1);
    container.set_message(None, true).unwrap();
    drop(container);
    assert!(frame.overlay(&mut opened[0], vec![]).unwrap());
    frame.overlay(&mut opened[0], vec![]).unwrap();
    let at = frame.control(1);
    frame
        .overlay(&mut opened[0], crate::attachment_support::click(at))
        .unwrap();
    assert!(Rc::ptr_eq(&recorded.downloads.borrow()[0], &record));
}

/// Register an opening tool and an independently failing later child.
#[cfg(not(target_arch = "wasm32"))]
fn overlay_renderers() -> Rc<std::cell::RefCell<maestro_web::Attachment>> {
    use maestro_web::tools::types::{ToolRenderResult, ToolRendererBinding, ToolUiError};
    let record = crate::attachment_support::attachment();
    let owned = record.clone();
    maestro_web::register_tool_renderer(
        "stream-overlay",
        ToolRendererBinding::from_raw::<maestro_app::JsonObject, serde_json::Value>(Rc::new(
            crate::renderer_recipes::Renderer(Rc::new(move |_, _, _| {
                Ok(ToolRenderResult {
                    content: Box::new(OpenOverlay(owned.clone())),
                    is_custom: true,
                })
            })),
        )),
    );
    maestro_web::register_tool_renderer(
        "stream-later-failure",
        ToolRendererBinding::from_raw::<maestro_app::JsonObject, serde_json::Value>(Rc::new(
            crate::renderer_recipes::Renderer(Rc::new(|_, _, _| {
                Err(ToolUiError::Presentation("later child failure".into()))
            })),
        )),
    );
    record
}

#[cfg(not(target_arch = "wasm32"))]
mod recorded;
