//! Composed messages retain the shared native placement and failure policy.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::support::tool_output::TextOnlyOutput as _;
use crate::{markdown_support, renderer_recipes, support};

#[cfg(test)]
mod tests {
    use super::*;
    use maestro_web::browser::attachment_content::ReadingPhase;
    use maestro_web::tools::types::ToolUiError;
    use maestro_web::{AssistantMessage, AssistantMessageProps, ToolRenderer};
    use markdown_support::{Surface, check, js};
    use std::{cell::RefCell, collections::HashSet, rc::Rc};
    use wasm_bindgen::prelude::*;

    /// Build an assistant using the public model record boundary.
    fn assistant(content: &serde_json::Value) -> maestro_app::AssistantMessage {
        serde_json::from_value(serde_json::json!({
            "role":"assistant", "content":content, "api":"openai-completions",
            "provider":"openai", "model":"controlled", "stopReason":"stop", "timestamp":0,
            "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
                "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}
        }))
        .unwrap()
    }
    /// Render the production row into the ordinary browser reading frame.
    fn draw(
        surface: &mut Surface,
        row: &mut AssistantMessage,
        message: &maestro_app::AssistantMessage,
        props: AssistantMessageProps<'_>,
    ) -> Result<egui::Response, ToolUiError> {
        let host = surface.host();
        surface.draw(|ui, frame| {
            row.render(
                ui,
                message,
                props,
                support::reading_view::view(&host, frame),
            )
            .assert_no_overlays()
        })
    }
    /// Draw a settled assistant using the ordinary optional-collection defaults.
    fn draw_plain(
        surface: &mut Surface,
        row: &mut AssistantMessage,
        message: &maestro_app::AssistantMessage,
    ) {
        draw(surface, row, message, AssistantMessageProps::default()).unwrap();
    }
    /// Native text remains selectable and correctly placed under independent axis transforms.
    /// # Panics
    /// Panics if native selection, geometry or modal admission differs.
    #[wasm_bindgen]
    pub fn message_reading_keeps_selection_fractional_placement_and_modal_admission() {
        let mut surface = Surface::new();
        let host = surface.host();
        let mut row = maestro_web::UserMessage::new(host.clone());
        let message = maestro_app::UserContent::Text("**Searchable native text** [link](https://example.test/)\n\n```js\nconst selected = 1;\n```".into());
        let _ = js(
            "const c=document.getElementById('canvas');c.style.left='13.25px';c.style.top='21.5px';c.style.width='640px';c.style.height='420px'",
        );
        draw_user(&mut surface, &mut row, &message);
        check(
            "devicePixelRatio===1.25 && document.querySelector('.maestro-markdown strong').textContent==='Searchable native text' && document.querySelector('.maestro-markdown a').href==='https://example.test/'",
        );
        let _ = js(
            "globalThis.messageRoot=document.querySelector('.maestro-markdown');const range=document.createRange();range.selectNodeContents(messageRoot.querySelector('strong'));getSelection().removeAllRanges();getSelection().addRange(range);globalThis.messageSelection=getSelection().toString();globalThis.messageSlot=messageRoot.querySelector('.control-slot')",
        );
        draw_user(&mut surface, &mut row, &message);
        check(
            "document.querySelector('.maestro-markdown')===messageRoot && getSelection().toString()===messageSelection && messageSelection==='Searchable native text'",
        );
        let expected = surface.effects.controls.borrow()[0];
        markdown_support::publish(
            "slotRect",
            &serde_json::to_string(&[
                expected.min.x,
                expected.min.y,
                expected.width(),
                expected.height(),
            ])
            .unwrap()
            .into(),
        );
        check(
            "(()=>{const c=document.getElementById('canvas').getBoundingClientRect(),f=messageRoot.closest('.maestro-reading-frame').getBoundingClientRect(),r=messageSlot.getBoundingClientRect(),e=JSON.parse(slotRect),sx=c.width/800,sy=c.height/600;return Math.abs(r.x-(f.x+e[0]*sx))<0.02 && Math.abs(r.y-(f.y+e[1]*sy))<0.02 && Math.abs(r.width-e[2]*sx)<0.02 && Math.abs(r.height-e[3]*sy)<0.02})()",
        );
        let modal = egui::Id::new("message-neighbor");
        host.set_overlay(modal, true).unwrap();
        check(
            "getComputedStyle(messageRoot.closest('.maestro-reading-frame')).visibility==='hidden'",
        );
        host.set_overlay(modal, false).unwrap();
        check(
            "getComputedStyle(messageRoot.closest('.maestro-reading-frame')).visibility==='visible' && getSelection().toString()===messageSelection",
        );
        drop(row);
        check("!messageRoot.isConnected && !messageSlot.isConnected");
    }

    /// Draw a user row without a trailing usage owner replacing control measurements.
    fn draw_user(
        surface: &mut Surface,
        row: &mut maestro_web::UserMessage,
        content: &maestro_app::UserContent,
    ) {
        let host = surface.host();
        surface.draw(|ui, frame| {
            row.render(
                ui,
                maestro_web::UserMessageInput::WithAttachments {
                    content,
                    attachments: None,
                },
                support::reading_view::view(&host, frame),
            )
            .assert_no_overlays()
            .unwrap()
        });
    }

    /// Rejected publication preserves the old emitted owner until replacement succeeds.
    /// # Panics
    /// Panics if a failed phase changes the last successful root or leaks a retired owner.
    #[wasm_bindgen]
    pub fn message_reading_preserves_last_success_and_releases_replaced_children() {
        let mut surface = Surface::new();
        let mut row = AssistantMessage::new(surface.host());
        let original = assistant(&serde_json::json!([
            {"type":"text","text":"**kept**\n\n```js\nconst kept = 1;\n```"}
        ]));
        draw_plain(&mut surface, &mut row, &original);
        let _ = js(
            "globalThis.keptMessage=document.querySelector('.maestro-markdown');globalThis.keptControl=keptMessage.querySelector('.control-slot');const r=document.createRange();r.selectNodeContents(keptMessage.querySelector('strong'));getSelection().removeAllRanges();getSelection().addRange(r)",
        );
        let replacement = assistant(&serde_json::json!([
            {"type":"text","text":"**replacement**\n\n```js\nconst next = 2;\n```"}
        ]));
        for phase in [
            Some(ReadingPhase::Measure),
            None,
            Some(ReadingPhase::Publish),
        ] {
            surface.effects.fail_phase.set(phase);
            surface.effects.fail_measure.set(phase.is_none());
            assert!(
                draw(
                    &mut surface,
                    &mut row,
                    &replacement,
                    AssistantMessageProps::default()
                )
                .is_err()
            );
            check(
                "document.querySelector('.maestro-markdown')===keptMessage && keptControl.isConnected && getSelection().toString()==='kept'",
            );
        }
        draw_plain(&mut surface, &mut row, &replacement);
        check(
            "!keptMessage.isConnected && !keptControl.isConnected && document.querySelector('.maestro-markdown strong').textContent==='replacement'",
        );
        replacement_kind_failure(&mut surface, &mut row);
        let empty = assistant(&serde_json::json!([]));
        draw_plain(&mut surface, &mut row, &empty);
        check("document.querySelectorAll('.maestro-markdown,.maestro-thinking-header').length===0");
        drop(row);
        check("document.querySelectorAll('.maestro-reading-frame').length===0");
        hidden_owners_release(&mut surface);
        independent_overlay_survives(&mut surface);
    }
    /// A failed kind change does not retire its predecessor or retain speculative callbacks.
    fn replacement_kind_failure(surface: &mut Surface, row: &mut AssistantMessage) {
        let _ = js("globalThis.beforeKind=document.querySelector('.maestro-markdown')");
        let thinking =
            assistant(&serde_json::json!([{"type":"thinking","thinking":"next thinking"}]));
        surface.effects.fail_phase.set(Some(ReadingPhase::Publish));
        assert!(draw(surface, row, &thinking, AssistantMessageProps::default()).is_err());
        check(
            "beforeKind.isConnected && document.querySelector('.maestro-thinking-header')===null",
        );
        draw(surface, row, &thinking, AssistantMessageProps::default()).unwrap();
        check(
            "!beforeKind.isConnected && document.querySelector('.maestro-thinking-header')!==null",
        );
    }

    /// Hiding usage, errors and tools revokes their roots and detached callback targets.
    fn hidden_owners_release(surface: &mut Surface) {
        renderer_recipes::reset();
        let renderer: Rc<dyn ToolRenderer> =
            Rc::new(renderer_recipes::Renderer(Rc::new(|_, _, _| {
                Ok(maestro_web::tools::types::ToolRenderResult {
                    content: Box::new(renderer_recipes::RichText("**retired tool**".into())),
                    is_custom: true,
                })
            })));
        maestro_web::register_tool_renderer(
            "retired",
            maestro_web::tools::types::ToolRendererBinding::from_raw(renderer),
        );
        let mut row = AssistantMessage::new(surface.host());
        let mut message = assistant(&serde_json::json!([
            {"type":"toolCall","id":"retired","name":"retired","arguments":{}}
        ]));
        message.usage.input = 1.;
        message.stop_reason = maestro_app::StopReason::Error;
        message.error_message = Some("retired error".into());
        let hits = Rc::new(std::cell::Cell::new(0));
        let count = hits.clone();
        draw(
            surface,
            &mut row,
            &message,
            AssistantMessageProps {
                on_cost_click: Some(Rc::new(move || count.set(count.get() + 1))),
                ..AssistantMessageProps::default()
            },
        )
        .unwrap();
        let _ = js(
            "globalThis.retiredUsage=document.querySelector('.maestro-message-usage');globalThis.retiredError=document.querySelector('.maestro-message-error');globalThis.retiredTool=document.querySelector('.maestro-markdown');retiredUsage.click()",
        );
        assert_eq!(hits.get(), 1);
        message.stop_reason = maestro_app::StopReason::Stop;
        draw(
            surface,
            &mut row,
            &message,
            AssistantMessageProps {
                hide_tool_calls: true,
                is_streaming: true,
                ..AssistantMessageProps::default()
            },
        )
        .unwrap();
        check("!retiredUsage.isConnected && !retiredError.isConnected && !retiredTool.isConnected");
        let _ = js("retiredUsage.click()");
        assert_eq!(hits.get(), 1);
        drop(row);
        check("document.querySelectorAll('.maestro-reading-frame').length===0");
    }
    /// Returned attachment overlays outlive both a removed tile and its containing row.
    fn independent_overlay_survives(surface: &mut Surface) {
        let host = surface.host();
        let mut row = maestro_web::UserMessage::new(host.clone());
        let attachment = std::sync::Arc::new(std::sync::RwLock::new(serde_json::from_value::<maestro_web::Attachment>(serde_json::json!({
            "id":"independent", "type":"document", "fileName":"independent.txt", "mimeType":"text/plain",
            "content":"YWJj", "size":3, "extractedText":"original independent text",
            "preview":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg=="
        })).unwrap()));
        let attachments = [attachment.clone()];
        let content = maestro_app::UserContent::Text("**containing user**".into());
        surface.draw(|ui, frame| {
            row.render(
                ui,
                maestro_web::UserMessageInput::WithAttachments {
                    content: &content,
                    attachments: Some(&attachments),
                },
                support::reading_view::view(&host, frame),
            )
            .assert_no_overlays()
            .unwrap()
        });
        let r = js(
            "(()=>{const r=document.querySelector('img[alt=\"independent.txt\"]').getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()",
        );
        let points = js_sys::Float32Array::new(&r);
        let at = egui::pos2(points.get_index(0), points.get_index(1));
        let mut opened = Vec::new();
        surface.click(at, |ui, frame| {
            let output = row.render(
                ui,
                maestro_web::UserMessageInput::WithAttachments {
                    content: &content,
                    attachments: Some(&attachments),
                },
                support::reading_view::view(&host, frame),
            );
            opened.extend(output.overlays);
            output.response.unwrap();
        });
        assert_eq!(opened.len(), 1);
        draw_user(surface, &mut row, &content);
        drop(row);
        attachment.write().unwrap().extracted_text = Some("independent owner still live".into());
        surface.draw(|ui, frame| {
            opened[0]
                .render(ui.ctx(), frame.i18n, frame.presentation)
                .unwrap()
        });
        check(
            "document.body.textContent.includes('independent owner still live') && !document.body.textContent.includes('containing user')",
        );
        drop(opened);
        check("document.querySelectorAll('.maestro-reading-frame').length===0");
    }

    /// Large mixed sequences dispatch each visible tool exactly once in emitted order.
    /// # Panics
    /// Panics if filtering changes order or invokes any callback twice.
    #[wasm_bindgen]
    pub fn assistant_large_sequences_dispatch_each_visible_tool_once() {
        renderer_recipes::reset();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let recorded = calls.clone();
        let renderer: Rc<dyn ToolRenderer> = Rc::new(renderer_recipes::Renderer(Rc::new(
            move |args: Option<&maestro_app::JsonObject>, _, _| {
                recorded
                    .borrow_mut()
                    .push(args.unwrap()["position"].as_u64().unwrap());
                Ok(renderer_recipes::content("visible", true))
            },
        )));
        maestro_web::register_tool_renderer(
            "repeated",
            maestro_web::tools::types::ToolRendererBinding::from_raw(renderer),
        );
        let mut surface = Surface::new();
        let mut row = AssistantMessage::new(surface.host());
        for count in [8, 16] {
            let chunks: Vec<_> = (0..count).map(|n| match n % 4 {
                0 => serde_json::json!({"type":"text","text":format!("text {n}")}),
                2 => serde_json::json!({"type":"thinking","thinking":format!("thinking {n}")}),
                _ => serde_json::json!({"type":"toolCall","id":format!("call-{n}"),"name":"repeated","arguments":{"position":n}}),
            }).collect();
            let message = assistant(&serde_json::Value::Array(chunks));
            let pending: HashSet<_> = (0..count)
                .filter(|n| n % 4 == 3)
                .map(|n| format!("call-{n}"))
                .collect();
            let tools = vec!["other".into(), "repeated".into(), "repeated".into()];
            for hide in [false, true] {
                calls.borrow_mut().clear();
                draw(
                    &mut surface,
                    &mut row,
                    &message,
                    AssistantMessageProps {
                        tools: Some(&tools),
                        pending_tool_calls: Some(&pending),
                        hide_pending_tool_calls: hide,
                        ..AssistantMessageProps::default()
                    },
                )
                .unwrap();
                let expected: Vec<u64> = (0..count)
                    .filter(|n| n % 2 == 1 && (!hide || n % 4 != 3))
                    .collect();
                assert_eq!(*calls.borrow(), expected);
            }
        }
    }
}
