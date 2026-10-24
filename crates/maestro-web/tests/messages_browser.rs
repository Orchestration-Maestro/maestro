//! Composed transcript rows through the real browser reading host.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::support::tool_output::TextOnlyOutput as _;
use crate::{markdown_support, renderer_recipes, support};

use maestro_web::{UserMessage, UserMessageInput};
use markdown_support::{Surface, check, js, publish};
use wasm_bindgen::prelude::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// Observe first-text selection through the actual Markdown owner.
    /// # Panics
    /// Panics if rendering or a browser observation fails.
    #[wasm_bindgen]
    pub fn user_rows_select_the_first_text_block() {
        let _ = js(
            "if (!document.getElementById('canvas')) { const c=document.createElement('canvas'); c.id='canvas'; c.width=800; c.height=600; document.body.appendChild(c); }",
        );
        let mut surface = Surface::new();
        let host = surface.host();
        let mut row = UserMessage::new(host.clone());
        let image = serde_json::json!({"type":"image","data":"AA==","mimeType":"image/png"});
        let text = |value: &str| serde_json::json!({"type":"text","text":value});
        for (content, expected) in [
            (serde_json::json!("  original text  "), "original text"),
            (serde_json::json!([]), ""),
            (serde_json::json!([image.clone()]), ""),
            (
                serde_json::json!([image, text("first"), text("later")]),
                "first",
            ),
            (serde_json::json!([text(""), text("later")]), ""),
            (
                serde_json::json!([text("first"), text("first"), text("later")]),
                "first",
            ),
        ] {
            let message: maestro_app::UserMessage = serde_json::from_value(
                serde_json::json!({"role":"user","content":content,"timestamp":0}),
            )
            .unwrap();
            draw_user(&mut surface, &mut row, UserMessageInput::User(&message));
            publish("expectedMessage", &expected.into());
            check(
                "document.querySelector('.maestro-markdown').textContent.trim() === expectedMessage",
            );
        }
        let message: maestro_app::UserMessage = serde_json::from_value(serde_json::json!({
            "role":"user", "content":"    literal indentation", "timestamp":0
        }))
        .unwrap();
        draw_user(&mut surface, &mut row, UserMessageInput::User(&message));
        check(
            "document.querySelector('.maestro-markdown .maestro-code code').textContent === 'literal indentation\\n'",
        );
        let message: maestro_app::UserMessage = serde_json::from_value(serde_json::json!({
            "role":"user", "content":"scrolling text\n\n".repeat(80), "timestamp":0
        }))
        .unwrap();
        surface.size = (320., 160.);
        draw_user(&mut surface, &mut row, UserMessageInput::User(&message));
        check("document.querySelector('.maestro-user-decoration') !== null");
        check(
            "(()=>{const d=document.querySelector('.maestro-user-decoration'), v=document.querySelector('.maestro-markdown').closest('.maestro-reading-viewport'); const r=d.getBoundingClientRect(), t=v.firstElementChild.getBoundingClientRect().top; v.scrollTop=40; const after=d.getBoundingClientRect(); return v.scrollTop>0 && v.firstElementChild.getBoundingClientRect().top<t && r.top===after.top && r.height===after.height && d.parentElement===d.closest('.maestro-reading-frame') && !v.contains(d)})()",
        );
        drop(row);
        check(
            "document.querySelectorAll('.maestro-markdown,.maestro-user-decoration').length === 0",
        );
    }

    /// Native intrinsic measurement bounds both short text and wrapping attachment rows.
    /// # Panics
    /// Panics if reading allocation or its decorated bubble stretches beyond content.
    #[wasm_bindgen]
    pub async fn user_bubbles_measure_intrinsic_width_before_decoration() {
        let mut surface = Surface::new();
        let mut row = UserMessage::new(surface.host());
        let attachments = [attachment("note.txt")];
        for (text, tiles, expected) in [
            ("User message".to_owned(), None, 147.0625),
            ("User message ".repeat(40), None, 768.),
            ("Hi".to_owned(), Some(&attachments[..]), 97.6),
        ] {
            let content = maestro_app::UserContent::Text(text);
            draw_user(
                &mut surface,
                &mut row,
                UserMessageInput::WithAttachments {
                    content: &content,
                    attachments: tiles,
                },
            );
            publish(
                "expectedBubbleWidth",
                &wasm_bindgen::JsValue::from_f64(expected),
            );
            check(
                "(()=>{const f=document.querySelector('.maestro-markdown').closest('.maestro-reading-frame').getBoundingClientRect(),b=document.querySelector('.maestro-user-decoration').getBoundingClientRect(); console.log('BUBBLE WIDTH',f.width,b.width,expectedBubbleWidth); return Math.abs(b.width-expectedBubbleWidth)<0.1 && f.width<=b.width-32 && (expectedBubbleWidth!==147.0625 || Math.abs(f.width-113.4625)<0.1) && b.right<=784.01})()",
            );
            support::reading_paint::observe("user-intrinsic-width").await;
        }
    }

    /// Draw one user input through the actual native host and return independent overlays.
    fn draw_user(
        surface: &mut Surface,
        row: &mut UserMessage,
        input: UserMessageInput<'_>,
    ) -> Vec<maestro_web::AttachmentOverlay> {
        let host = surface.host();
        surface.draw(|ui, frame| {
            let output = row.render(ui, input, support::reading_view::view(&host, frame));
            let overlays = output.overlays;
            output.response.unwrap();
            overlays
        })
    }
    /// Draw ordinary settled assistant policy without hiding default detail selection.
    fn draw_plain_assistant(
        surface: &mut Surface,
        row: &mut AssistantMessage,
        message: &maestro_app::AssistantMessage,
    ) {
        draw_assistant(surface, row, message, AssistantMessageProps::default());
    }
    /// Build a controlled assistant record through the public message boundary.
    fn assistant(content: &serde_json::Value, stop: &str) -> maestro_app::AssistantMessage {
        serde_json::from_value(serde_json::json!({
        "role":"assistant", "content":content, "api":"openai-completions", "provider":"openai", "model":"controlled",
        "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
        "stopReason":stop,"timestamp":0
    })).unwrap()
    }

    use maestro_web::{
        AssistantMessage, AssistantMessageProps, ToolRenderer, register_tool_renderer,
        tools::types::ToolRendererBinding,
    };
    use std::{
        cell::RefCell,
        collections::{HashMap, HashSet},
        rc::Rc,
    };
    /// Callback observations from actual composed tool dispatch.
    type ToolObservations = Rc<RefCell<Vec<(Option<bool>, bool)>>>;
    /// Register the shared raw recipe fixture and retain its observed result policy.
    fn visibility_renderer() -> ToolObservations {
        renderer_recipes::reset();
        let observed = Rc::new(RefCell::new(Vec::new()));
        let captured = observed.clone();
        let renderer: Rc<dyn ToolRenderer> = Rc::new(renderer_recipes::Renderer(Rc::new(
            move |_, result, streaming| {
                captured.borrow_mut().push((
                    result
                        .as_ref()
                        .map(maestro_web::tools::types::ToolResultMessageView::is_error),
                    streaming,
                ));
                Ok(renderer_recipes::content("visible tool", true))
            },
        )));
        register_tool_renderer("tool", ToolRendererBinding::from_raw(renderer));
        observed
    }
    /// Draw a supplied assistant policy through the shared native reading frame.
    fn draw_assistant(
        surface: &mut Surface,
        row: &mut AssistantMessage,
        message: &maestro_app::AssistantMessage,
        props: AssistantMessageProps<'_>,
    ) {
        let host = surface.host();
        surface.draw(|ui, frame| {
            row.render(
                ui,
                message,
                props,
                support::reading_view::view(&host, frame),
            )
            .assert_no_overlays()
            .unwrap()
        });
    }
    /// Each visibility flag preserves its independent pending/result/abort policy.
    /// # Panics
    /// Panics if dispatch or a composed-row observation fails.
    #[wasm_bindgen]
    pub fn assistant_tool_visibility_covers_each_pending_result_combination() {
        let _ = js(
            "if (!document.getElementById('canvas')) { const c=document.createElement('canvas'); c.id='canvas'; c.width=800; c.height=600; document.body.appendChild(c); }",
        );
        let observed = visibility_renderer();
        let mut surface = Surface::new();
        let host = surface.host();
        let mut row = AssistantMessage::new(host.clone());
        let call =
            serde_json::json!([{"type":"toolCall","id":"call","name":"tool","arguments":{}}]);
        let pending = HashSet::from(["call".into()]);
        let mut result = renderer_recipes::message(None::<serde_json::Value>);
        result.is_error = false;
        let results = HashMap::from([("call".into(), result)]);
        let tools = vec!["other".into(), "tool".into(), "tool".into()];
        for bits in 0..32 {
            let [hide, hide_pending, is_pending, has_result, aborted]: [bool; 5] =
                std::array::from_fn(|bit| bits & (1 << bit) != 0);
            let message = assistant(&call, if aborted { "aborted" } else { "stop" });
            observed.borrow_mut().clear();
            draw_assistant(
                &mut surface,
                &mut row,
                &message,
                AssistantMessageProps {
                    tools: Some(&tools),
                    pending_tool_calls: is_pending.then_some(&pending),
                    hide_tool_calls: hide,
                    tool_results_by_id: has_result.then_some(&results),
                    hide_pending_tool_calls: hide_pending,
                    ..AssistantMessageProps::default()
                },
            );
            let visible = [
                true, false, true, false, true, false, false, false, true, false, true, false,
                true, false, true, false,
            ][bits % 16];
            let error = match (has_result, aborted) {
                (true, _) => Some(false),
                (false, true) => Some(true),
                (false, false) => None,
            };
            let expected = if visible {
                vec![(error, is_pending && (has_result || !aborted))]
            } else {
                vec![]
            };
            assert_eq!(*observed.borrow(), expected, "combination {bits}");
        }
        let message = assistant(&call, "stop");
        observed.borrow_mut().clear();
        let props: AssistantMessageProps<'_> = AssistantMessageProps::default();
        draw_assistant(&mut surface, &mut row, &message, props);
        assert_eq!(*observed.borrow(), [(None, false)]);
    }
    /// Register owned rich text with a live custom-card policy.
    fn register_rich(name: &str, text: &str, custom: Rc<std::cell::Cell<bool>>) {
        let text = text.to_owned();
        let renderer: Rc<dyn ToolRenderer> =
            Rc::new(renderer_recipes::Renderer(Rc::new(move |_, _, _| {
                Ok(maestro_web::tools::types::ToolRenderResult {
                    is_custom: custom.get(),
                    content: Box::new(renderer_recipes::RichText(text.clone())),
                })
            })));
        register_tool_renderer(name, ToolRendererBinding::from_raw(renderer));
    }
    /// Source filtering keeps emitted-position disclosure owners and mixed visual order.
    /// # Panics
    /// Panics if native content, ownership or ordering differs.
    #[wasm_bindgen]
    pub fn assistant_rows_keep_mixed_order_and_filtered_slot_lifetimes() {
        renderer_recipes::reset();
        register_rich("mixed", "**tool**", Rc::new(std::cell::Cell::new(true)));
        let mut surface = Surface::new();
        let mut row = AssistantMessage::new(surface.host());
        let content = serde_json::json!([
            {"type":"text","text":""}, {"type":"text","text":" \t\r\n"}, {"type":"text","text":"\u{feff}"},
            {"type":"text","text":"  **first**  "}, {"type":"thinking","thinking":"**one**"},
            {"type":"toolCall","id":"repeat","name":"mixed","arguments":{}},
            {"type":"text","text":"**first**"}, {"type":"thinking","thinking":"**two**"},
            {"type":"text","text":"\u{85}"}
        ]);
        let mut message = assistant(&content, "stop");
        draw_plain_assistant(&mut surface, &mut row, &message);
        check(
            "document.querySelectorAll('.maestro-thinking-header').length===2 && document.querySelectorAll('.maestro-markdown strong').length===3 && [...document.querySelectorAll('.maestro-markdown')].some(n=>n.textContent.includes('\\u0085'))",
        );
        check(
            "(()=>{const nodes=[...document.querySelectorAll('.maestro-markdown strong,.maestro-thinking-header')].sort((a,b)=>a.getBoundingClientRect().top-b.getBoundingClientRect().top);return nodes.map(n=>n.textContent).join('|')==='first|Thinking...|tool|first|Thinking...'})()",
        );
        let _ = js(
            "globalThis.firstThinking=document.querySelector('.maestro-thinking-header'); firstThinking.click()",
        );
        draw_plain_assistant(&mut surface, &mut row, &message);
        check("document.querySelector('.maestro-thinking strong').textContent==='one'");
        message.content = serde_json::from_value(serde_json::json!([
            {"type":"thinking","thinking":" "}, {"type":"thinking","thinking":"**second source**"}
        ]))
        .unwrap();
        draw_plain_assistant(&mut surface, &mut row, &message);
        // The first emitted kind changes from text to thinking, so the old first header retires.
        check("!firstThinking.isConnected && document.querySelector('.maestro-thinking')===null");
        message.content=serde_json::from_value(serde_json::json!([
            {"type":"thinking","thinking":"**first source**"},{"type":"thinking","thinking":"**second source**"}
        ])).unwrap();
        draw_plain_assistant(&mut surface, &mut row, &message);
        let _ = js(
            "globalThis.reusedThinking=document.querySelector('.maestro-thinking-header');globalThis.retiredThinking=document.querySelectorAll('.maestro-thinking-header')[1]; reusedThinking.click()",
        );
        draw_plain_assistant(&mut surface, &mut row, &message);
        message.content = serde_json::from_value(serde_json::json!([
            {"type":"thinking","thinking":""}, {"type":"thinking","thinking":"**updated**"}
        ]))
        .unwrap();
        draw_plain_assistant(&mut surface, &mut row, &message);
        check(
            "document.querySelector('.maestro-thinking-header')===reusedThinking && document.querySelector('.maestro-thinking strong').textContent==='updated' && !retiredThinking.isConnected",
        );
        drop(row);
        check("document.querySelectorAll('.maestro-markdown,.maestro-thinking-header').length===0");
    }

    /// Nonzero usage fixture consumed by literal usage observations.
    fn usage_message() -> maestro_app::AssistantMessage {
        let mut message = assistant(&serde_json::json!([]), "stop");
        message.usage.input = 1200.;
        message.usage.output = 25.;
        message.usage.cache_read = 4.;
        message.usage.cache_write = 2.;
        message.usage.cost.total = 0.125;
        message
    }
    /// The real usage target admits only its current handler and disappears while streaming.
    /// # Panics
    /// Panics if a rendered usage string or callback count differs.
    #[wasm_bindgen]
    pub fn assistant_usage_respects_streaming_and_handler_changes() {
        use std::cell::Cell;
        let mut surface = Surface::new();
        let mut row = AssistantMessage::new(surface.host());
        let mut message = usage_message();
        publish(
            "expectedUsage",
            &maestro_web::format_usage(Some(&message.usage)).into(),
        );
        let hits = Rc::new(Cell::new(0));
        for (handler, expected) in [(1, 1), (10, 11), (0, 11)] {
            let count = Rc::clone(&hits);
            let action: Option<Rc<dyn Fn()>> = (handler > 0)
                .then(|| Rc::new(move || count.set(count.get() + handler)) as Rc<dyn Fn()>);
            draw_assistant(
                &mut surface,
                &mut row,
                &message,
                AssistantMessageProps {
                    on_cost_click: action,
                    ..Default::default()
                },
            );
            check("document.querySelector('.maestro-message-usage').textContent===expectedUsage");
            let _ = js(
                "globalThis.usageTarget=document.querySelector('.maestro-message-usage'); usageTarget.click()",
            );
            assert_eq!(hits.get(), expected);
        }
        for stop in ["stop", "error", "aborted"] {
            message.stop_reason = serde_json::from_value(stop.into()).unwrap();
            for error in [None, Some(""), Some("literal error")] {
                message.error_message = error.map(str::to_owned);
                draw_plain_assistant(&mut surface, &mut row, &message);
                check(
                    "document.querySelector('.maestro-message-usage').textContent===expectedUsage",
                );
                draw_assistant(
                    &mut surface,
                    &mut row,
                    &message,
                    AssistantMessageProps {
                        is_streaming: true,
                        ..Default::default()
                    },
                );
                check(
                    "document.querySelector('.maestro-message-usage')===null && !usageTarget.isConnected",
                );
                let _ = js("usageTarget.click()");
                assert_eq!(hits.get(), 11);
            }
        }
        message.usage = maestro_app::Usage::default();
        draw_plain_assistant(&mut surface, &mut row, &message);
        check("document.querySelector('.maestro-message-usage').textContent===''");
        assert_usage_release(&mut surface, row, &message, &hits);
    }

    /// A detached clickable usage target cannot retain the row's admitted action.
    fn assert_usage_release(
        surface: &mut Surface,
        mut row: AssistantMessage,
        message: &maestro_app::AssistantMessage,
        hits: &Rc<std::cell::Cell<u32>>,
    ) {
        let captured = hits.clone();
        draw_assistant(
            surface,
            &mut row,
            message,
            AssistantMessageProps {
                on_cost_click: Some(Rc::new(move || captured.set(captured.get() + 100))),
                ..Default::default()
            },
        );
        let _ = js("globalThis.finalUsage=document.querySelector('.maestro-message-usage')");
        drop(row);
        let _ = js("finalUsage.click()");
        assert_eq!(hits.get(), 11);
        check("document.querySelector('.maestro-message-usage')===null && !finalUsage.isConnected");
    }
    /// Error markup stays literal and both localized outcomes use their own admission policy.
    /// # Panics
    /// Panics if localization or actual native outcome content differs.
    #[wasm_bindgen]
    pub fn assistant_errors_and_aborts_stay_literal_and_localized() {
        let mut surface = Surface::new();
        let mut row = AssistantMessage::new(surface.host());
        for (language, prefix, abort) in [
            ("en", "Error:", "Request aborted"),
            ("de", "Fehler:", "Anfrage abgebrochen"),
        ] {
            surface.i18n.set_language(language).unwrap();
            publish("errorPrefix", &prefix.into());
            publish("abortText", &abort.into());
            check_outcomes(&mut surface, &mut row);
        }
    }
    /// Draw an independently returned overlay after its row is released.
    fn draw_overlay(surface: &mut Surface, overlay: &mut maestro_web::AttachmentOverlay) {
        surface.draw(|ui, frame| {
            overlay
                .render(ui.ctx(), frame.i18n, frame.presentation)
                .unwrap()
        });
    }
    /// Open tiles with real canvas gestures and keep independently returned ownership.
    fn open_user(
        surface: &mut Surface,
        row: &mut UserMessage,
        input: UserMessageInput<'_>,
        at: egui::Pos2,
    ) -> Vec<maestro_web::AttachmentOverlay> {
        let host = surface.host();
        let mut opened = Vec::new();
        surface.click(at, |ui, frame| {
            let output = row.render(ui, input, support::reading_view::view(&host, frame));
            opened.extend(output.overlays);
            output.response.unwrap();
        });
        opened
    }
    /// Construct an original mutable record with a valid thumbnail and inspectable text.
    fn attachment(name: &str) -> Rc<RefCell<maestro_web::Attachment>> {
        Rc::new(RefCell::new(serde_json::from_value(serde_json::json!({
            "id":name,"type":"document","fileName":name,"mimeType":"text/plain","content":"YWJj","size":3,
            "extractedText":name,
            "preview":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg=="
        })).unwrap()))
    }
    /// Tiles retain positional surfaces and opened overlays keep the original selected handle.
    /// # Panics
    /// Panics if tile order, release or independent overlay ownership differs.
    #[wasm_bindgen]
    pub fn user_rows_keep_attachment_tiles_and_opened_overlay_owners() {
        let mut surface = Surface::new();
        let mut row = UserMessage::new(surface.host());
        let first = attachment("first.txt");
        let second = attachment("second.txt");
        let content = maestro_app::UserContent::Text("**user**".into());
        for attachments in [
            None,
            Some(&[][..]),
            Some(&[first.clone(), second.clone()][..]),
        ] {
            draw_user(
                &mut surface,
                &mut row,
                UserMessageInput::WithAttachments {
                    content: &content,
                    attachments,
                },
            );
            publish(
                "tileCount",
                &attachments.map_or(0, <[_]>::len).to_string().into(),
            );
            check("document.querySelectorAll('img[alt]').length===Number(tileCount)");
        }
        let _ = js(
            "globalThis.firstTile=document.querySelector('img[alt=\"first.txt\"]'); globalThis.secondTile=document.querySelector('img[alt=\"second.txt\"]')",
        );
        let attachments = [second.clone(), first];
        draw_user(
            &mut surface,
            &mut row,
            UserMessageInput::WithAttachments {
                content: &content,
                attachments: Some(&attachments),
            },
        );
        check(
            "document.querySelector('img[alt=\"second.txt\"]')===firstTile && document.querySelector('img[alt=\"first.txt\"]')===secondTile",
        );
        let r=js("JSON.stringify((()=>{const r=firstTile.getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})())").as_string().unwrap();
        let [x, y]: [f32; 2] = serde_json::from_str(&r).unwrap();
        let mut opened = open_user(
            &mut surface,
            &mut row,
            UserMessageInput::WithAttachments {
                content: &content,
                attachments: Some(&attachments),
            },
            egui::pos2(x, y),
        );
        assert_eq!(opened.len(), 1);
        second.borrow_mut().extracted_text = Some("edited after opening".into());
        // Ordinary records ignore attachment-looking extra fields.
        let ordinary=serde_json::from_value(serde_json::json!({"role":"user","content":"ordinary","timestamp":0,"attachments":[{}]})).unwrap();
        draw_user(&mut surface, &mut row, UserMessageInput::User(&ordinary));
        check("!firstTile.isConnected && !secondTile.isConnected");
        drop(row);
        draw_overlay(&mut surface, &mut opened[0]);
        check("document.body.textContent.includes('edited after opening')");
        drop(opened);
        check("document.querySelectorAll('.maestro-reading-frame,img[alt]').length===0");
    }

    /// A native details handle deliberately has no serialization or decoding implementation.
    struct NativeDetails(Rc<std::cell::Cell<u32>>);
    /// Authoritative result metadata plus the observed live native detail value.
    type ResultEnvelope = (String, String, f64, bool, usize, Option<u32>);
    /// Callback observations of authoritative result metadata.
    struct Envelope {
        selected: String,
        streaming: bool,
        result: Option<ResultEnvelope>,
    }
    /// Read authoritative result fields while the callback borrow is alive.
    fn result_envelope(
        r: &maestro_web::tools::types::ToolResultMessageView<'_, NativeDetails>,
        handle: &Rc<std::cell::Cell<u32>>,
        content_address: usize,
    ) -> ResultEnvelope {
        assert_eq!(r.role(), "toolResult");
        if r.timestamp().to_bits() == 17.25f64.to_bits() {
            assert_eq!(r.content().as_ptr() as usize, content_address);
        }
        let detail = r.details().map(|d| {
            assert!(Rc::ptr_eq(&d.0, handle));
            d.0.get()
        });
        (
            r.tool_call_id().into(),
            r.tool_name().into(),
            r.timestamp(),
            r.is_error(),
            r.content().len(),
            detail,
        )
    }
    /// Draw a generic result without adding serialization bounds.
    fn draw_tool<D: 'static>(
        surface: &mut Surface,
        row: &mut maestro_web::ToolMessage,
        props: maestro_web::ToolMessageProps<'_, D>,
    ) {
        surface.draw_reading(|ui, view| row.render(ui, props, view).assert_no_overlays().unwrap());
    }
    /// Check abort precedence and the original envelope independently of the renderer.
    fn assert_envelope(
        observation: &Envelope,
        tool: Option<&str>,
        [pending, streaming, has_result, aborted]: [bool; 4],
        [before, after]: [f64; 2],
    ) {
        assert_eq!(
            observation.selected,
            tool.filter(|t| !t.is_empty()).unwrap_or("call-name")
        );
        assert_eq!(observation.streaming, !aborted && (pending || streaming));
        if aborted {
            let r = observation.result.as_ref().unwrap();
            assert_eq!(
                (&*r.0, &*r.1, r.3, r.4, r.5),
                ("call", "call-name", true, 0, None)
            );
            assert!(r.2 >= before && r.2 <= after && r.2.fract().abs() < f64::EPSILON);
        } else {
            assert_eq!(
                observation.result,
                has_result.then(|| (
                    "original call".into(),
                    "original tool".into(),
                    17.25,
                    true,
                    2,
                    Some(43)
                ))
            );
        }
    }
    /// Register a native-detail observer and render an owned rich recipe.
    fn envelope_renderer(
        name: &str,
        call: &maestro_app::ToolCall,
        handle: &Rc<std::cell::Cell<u32>>,
        seen: &Rc<RefCell<Vec<Envelope>>>,
        original: &maestro_app::ToolResultMessage<NativeDetails>,
    ) {
        let address = std::ptr::from_ref(&call.arguments) as usize;
        let content_address = original.content.as_ptr() as usize;
        let selected = name.to_owned();
        let handle = handle.clone();
        let seen = seen.clone();
        let renderer: Rc<dyn ToolRenderer<maestro_app::JsonObject, NativeDetails>> =
            Rc::new(renderer_recipes::Renderer::<
                maestro_app::JsonObject,
                NativeDetails,
            >(Rc::new(move |params, result, streaming| {
                assert_eq!(std::ptr::from_ref(params.unwrap()) as usize, address);
                let result = result.map(|r| result_envelope(&r, &handle, content_address));
                seen.borrow_mut().push(Envelope {
                    selected: selected.clone(),
                    streaming,
                    result,
                });
                Ok(maestro_web::tools::types::ToolRenderResult {
                    is_custom: true,
                    content: Box::new(renderer_recipes::RichText(format!(
                        "**native details {}**",
                        handle.get()
                    ))),
                })
            })));
        register_tool_renderer(name, ToolRendererBinding::from_raw(renderer));
    }
    /// A composed tool preserves borrowed objects, native details and explicit abort precedence.
    /// # Panics
    /// Panics if callback admission or an authoritative envelope field differs.
    #[wasm_bindgen]
    pub fn tool_rows_keep_result_envelopes_and_abort_precedence() {
        use maestro_web::{ToolMessage, ToolMessageProps};
        let mut surface = Surface::new();
        let call:maestro_app::ToolCall=serde_json::from_value(serde_json::json!({"type":"toolCall","id":"call","name":"call-name","arguments":{"x":1}})).unwrap();
        let handle = Rc::new(std::cell::Cell::new(42));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let result = renderer_recipes::message(Some(NativeDetails(handle.clone())));
        envelope_renderer("call-name", &call, &handle, &seen, &result);
        envelope_renderer("override", &call, &handle, &seen, &result);
        let mut row = ToolMessage::new();
        handle.set(43);
        for tool in [None, Some(""), Some("override")] {
            for bits in 0..16 {
                let [pending, streaming, has_result, aborted]: [bool; 4] =
                    std::array::from_fn(|i| bits & (1 << i) != 0);
                let before = js_sys::Date::now();
                draw_tool(
                    &mut surface,
                    &mut row,
                    ToolMessageProps {
                        tool_call: &call,
                        tool,
                        result: has_result.then_some(&result),
                        pending,
                        aborted,
                        is_streaming: streaming,
                    },
                );
                let after = js_sys::Date::now();
                assert_envelope(
                    seen.borrow().last().unwrap(),
                    tool,
                    [pending, streaming, has_result, aborted],
                    [before, after],
                );
            }
        }
        check(
            "document.querySelector('.maestro-markdown strong').textContent==='native details 43'",
        );
        drop(row);
        assert_native_assistant(&mut surface, &call, result, &handle, &seen);
    }
    /// Check every outcome for the selected language.
    fn check_outcomes(surface: &mut Surface, row: &mut AssistantMessage) {
        for stop in ["stop", "error", "aborted"] {
            check_stop_outcome(surface, row, stop);
        }
    }
    /// Check error payload and streaming independently for one stop reason.
    fn check_stop_outcome(surface: &mut Surface, row: &mut AssistantMessage, stop: &str) {
        let mut message = assistant(&serde_json::json!([]), stop);
        for error in [None, Some(""), Some("<b>literal & error</b>")] {
            message.error_message = error.map(str::to_owned);
            for streaming in [false, true] {
                draw_assistant(
                    surface,
                    row,
                    &message,
                    AssistantMessageProps {
                        is_streaming: streaming,
                        ..Default::default()
                    },
                );
                publish(
                    "hasError",
                    &(stop == "error" && error.is_some_and(|e| !e.is_empty())).into(),
                );
                publish("hasAbort", &(stop == "aborted").into());
                check(
                    "(()=>{const e=document.querySelector('.maestro-message-error'),a=document.querySelector('.maestro-message-abort');return !!e===hasError && !!a===hasAbort && (!e || (e.querySelector('strong').textContent===errorPrefix && e.textContent===errorPrefix+' <b>literal & error</b>' && !e.querySelector('b'))) && (!a || (a.textContent===abortText && getComputedStyle(a).fontStyle==='italic'))})()",
                );
            }
        }
    }
    /// The assistant result map preserves the same native detail envelope.
    fn assert_native_assistant(
        surface: &mut Surface,
        call: &maestro_app::ToolCall,
        result: maestro_app::ToolResultMessage<NativeDetails>,
        handle: &Rc<std::cell::Cell<u32>>,
        seen: &Rc<RefCell<Vec<Envelope>>>,
    ) {
        let message = assistant(&serde_json::json!([call]), "stop");
        let maestro_app::AssistantContent::ToolCall(ref assistant_call) = message.content[0] else {
            unreachable!()
        };
        envelope_renderer("call-name", assistant_call, handle, seen, &result);
        let results = HashMap::from([("call".into(), result)]);
        let mut row = AssistantMessage::new(surface.host());
        let host = surface.host();
        surface.draw(|ui, frame| {
            row.render(
                ui,
                &message,
                AssistantMessageProps {
                    tool_results_by_id: Some(&results),
                    ..Default::default()
                },
                support::reading_view::view(&host, frame),
            )
            .assert_no_overlays()
            .unwrap()
        });
        assert_eq!(
            seen.borrow().last().unwrap().result.as_ref().unwrap().5,
            Some(43)
        );
        drop(row);
        check("document.querySelector('.maestro-markdown')===null");
    }
    /// Only default content paints a padded panel, rounded border and small shadow.
    /// # Panics
    /// Panics if native recipe content or final-clip card pixels differ.
    #[wasm_bindgen]
    pub fn tool_rows_observe_the_custom_card_choice() {
        use maestro_web::ToolMessage;
        let mut surface = Surface::new();
        let host = surface.host();
        let custom = Rc::new(std::cell::Cell::new(false));
        register_rich("card", "**card content**", custom.clone());
        let call = serde_json::from_value(
            serde_json::json!({"type":"toolCall","id":"card","name":"card","arguments":{}}),
        )
        .unwrap();
        let mut row = ToolMessage::new();
        let props = maestro_web::ToolMessageProps::<serde_json::Value> {
            tool_call: &call,
            tool: None,
            result: None,
            pending: false,
            aborted: false,
            is_streaming: false,
        };
        let rect = surface
            .draw(|ui, frame| {
                row.render(ui, props, support::reading_view::view(&host, frame))
                    .assert_no_overlays()
                    .unwrap()
            })
            .rect;
        let panel = egui::Color32::from_hex(
            surface
                .pack
                .presentation(maestro_theme::BrandMode::Dark)
                .unwrap()
                .colors["panel"],
        )
        .unwrap();
        let inside = egui::pos2(rect.right() - 8., rect.center().y);
        let painted = surface
            .paint
            .pixel(&surface.context, &surface.shapes, inside);
        assert!(
            painted
                .into_iter()
                .zip(panel.to_array())
                .all(|(actual, expected)| (actual - f32::from(expected)).abs() < 0.01)
        );
        let shadow = surface.paint.pixel(
            &surface.context,
            &surface.shapes,
            egui::pos2(rect.center().x, rect.bottom() + 0.5),
        );
        assert!(shadow[3] > 0. && shadow[3] < 255.);
        check("document.querySelector('.maestro-markdown strong').textContent==='card content'");
        custom.set(true);
        draw_tool(&mut surface, &mut row, props);
        assert_eq!(
            surface
                .paint
                .pixel(&surface.context, &surface.shapes, inside)
                .map(f32::to_bits),
            [0.0_f32; 4].map(f32::to_bits)
        );
        check("document.querySelector('.maestro-markdown strong').textContent==='card content'");
    }
}
