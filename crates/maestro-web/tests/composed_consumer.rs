//! Production transcript witnesses through the shared native reading consumer.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::support::tool_output::TextOnlyOutput as _;
use crate::{markdown_support, message_renderer_support, renderer_recipes, support};

#[cfg(test)]
mod tests {
    use super::*;
    use maestro_web::{
        ThinkingBlock, ToolMessage, ToolMessageDebugView, ToolMessageProps, get_message_renderer,
        register_message_renderer, register_tool_renderer, render_message,
        tools::types::{
            ToolContent, ToolRenderResult, ToolRenderState, ToolRenderer, ToolRendererBinding,
        },
    };
    use markdown_support::{Surface, check, js};
    use message_renderer_support::{NativeMessage, Renderer};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicU32, Ordering},
        },
    };
    use wasm_bindgen::prelude::*;

    /// Read the live native payload after dropping its message guard.
    fn custom_recipe(message: &maestro_app::AgentMessage<NativeMessage>) -> Box<dyn ToolContent> {
        let maestro_app::AgentMessage::Custom(payload) = message else {
            unreachable!()
        };
        let value = payload.read().unwrap().handle.load(Ordering::Relaxed);
        Box::new(renderer_recipes::RichText(format!(
            "**native {value}** [link](https://example.test/)"
        )))
    }
    /// Draw an owned custom recipe using the one retained tool row-state owner.
    fn draw_recipe(surface: &mut Surface, row: &mut ToolRenderState, recipe: &ToolRenderResult) {
        let host = surface.host();
        surface.draw(|ui, frame| {
            recipe
                .render(ui, support::reading_view::view(&host, frame), row)
                .assert_no_overlays()
                .unwrap()
        });
    }
    /// Retained callbacks preserve live native inputs and release their final rich-text owner.
    /// # Panics
    /// Panics if callback capture, first-frame content or native ownership differs.
    #[wasm_bindgen]
    pub fn composed_custom_message_keeps_native_callback_and_final_owner() {
        let mut surface = Surface::new();
        let handle = Arc::new(AtomicU32::new(7));
        let message = message_renderer_support::message("composed-native", handle.clone());
        register_message_renderer(
            "composed-native",
            Rc::new(Renderer(Rc::new(|message| {
                register_message_renderer(
                    "composed-native",
                    Rc::new(Renderer(Rc::new(|message| Ok(custom_recipe(message))))),
                );
                Ok(custom_recipe(message))
            }))),
        );
        let retained = get_message_renderer::<NativeMessage>("composed-native").unwrap();
        let recipe = ToolRenderResult {
            content: render_message(&message).unwrap().unwrap(),
            is_custom: true,
        };
        let mut row = ToolRenderState::default();
        draw_recipe(&mut surface, &mut row, &recipe);
        let _ = js(
            "globalThis.nativeRoot=document.querySelector('.maestro-markdown');globalThis.nativeLink=nativeRoot.querySelector('a');const r=document.createRange();r.selectNodeContents(nativeLink);getSelection().removeAllRanges();getSelection().addRange(r)",
        );
        check(
            "nativeRoot.querySelector('strong').textContent==='native 7' && nativeLink.href==='https://example.test/' && getSelection().toString()==='link'",
        );
        draw_recipe(&mut surface, &mut row, &recipe);
        check(
            "document.querySelector('.maestro-markdown')===nativeRoot && getSelection().toString()==='link'",
        );
        let modal = egui::Id::new("composed-modal");
        surface.host().set_overlay(modal, true).unwrap();
        check(
            "getComputedStyle(nativeRoot.closest('.maestro-reading-frame')).visibility==='hidden'",
        );
        surface.host().set_overlay(modal, false).unwrap();
        check(
            "getComputedStyle(nativeRoot.closest('.maestro-reading-frame')).visibility==='visible'",
        );
        handle.store(8, Ordering::Relaxed);
        let next = ToolRenderResult {
            content: retained.render(&message).unwrap(),
            is_custom: true,
        };
        draw_recipe(&mut surface, &mut row, &next);
        check("document.querySelector('.maestro-markdown strong').textContent==='native 8'");
        drop(row);
        check("document.querySelector('.maestro-markdown')===null && !nativeRoot.isConnected");
    }

    /// Draw the three literal debug children through the production row.
    fn draw_debug(surface: &mut Surface, row: &mut ToolMessageDebugView) {
        surface.draw_reading(|ui, view| row.render(ui, view).unwrap());
    }
    /// Literal base64, ordered output, absent details and strict overflow reach actual listings.
    /// # Panics
    /// Panics if the native code consumer changes the displayed values.
    #[wasm_bindgen]
    pub fn debug_consumer_keeps_literal_output_and_absent_details() {
        let mut surface = Surface::new();
        let mut row = ToolMessageDebugView::new(surface.host());
        row.call_args = Some(serde_json::json!("YWJj"));
        row.has_result = true;
        row.result = Some(serde_json::from_value(serde_json::json!({
            "role":"toolResult","toolCallId":"c","toolName":"x","timestamp":0,"isError":false,
            "content":[{"type":"text","text":"first"},{"type":"image","data":"AA==","mimeType":"image/png"},{"type":"text","text":"last"}]
        })).unwrap());
        draw_debug(&mut surface, &mut row);
        check(
            "JSON.stringify([...document.querySelectorAll('.maestro-code code')].map(x=>x.textContent))===JSON.stringify(['YWJj','first\\nlast',''])",
        );
        row.result.as_mut().unwrap().details = Some(serde_json::Value::Null);
        draw_debug(&mut surface, &mut row);
        check("[...document.querySelectorAll('.maestro-code code')].at(-1).textContent==='null'");
        row.result.as_mut().unwrap().details = Some(serde_json::json!("YWJj"));
        draw_debug(&mut surface, &mut row);
        check("[...document.querySelectorAll('.maestro-code code')].at(-1).textContent==='YWJj'");
        row.result.as_mut().unwrap().content = vec![maestro_app::UserBlock::Text(
            maestro_app::TextContent::new("1e309".into()),
        )];
        draw_debug(&mut surface, &mut row);
        check("[...document.querySelectorAll('.maestro-code code')][1].textContent==='null'");
        drop(row);
        check("document.querySelector('.maestro-code')===null");
    }

    /// Draw one disclosure with the shared browser frame and syntax engine.
    fn draw_thinking(surface: &mut Surface, row: &mut ThinkingBlock) {
        surface.draw_reading(|ui, view| row.render(ui, view).unwrap());
    }
    /// One native disclosure callback survives streaming and is revoked on final release.
    /// # Panics
    /// Panics if disclosure, streaming or callback ownership differs.
    #[wasm_bindgen]
    pub fn thinking_consumer_retains_its_callback_across_streaming_and_release() {
        let mut surface = Surface::new();
        let mut row = ThinkingBlock::new(surface.host());
        row.content = "**first**".into();
        draw_thinking(&mut surface, &mut row);
        let _ = js("globalThis.header=document.querySelector('.maestro-thinking-header')");
        check(
            "header.textContent==='Thinking...' && document.querySelector('.maestro-markdown')===null",
        );
        let _ = js("header.click()");
        draw_thinking(&mut surface, &mut row);
        check("document.querySelector('.maestro-markdown strong').textContent==='first'");
        row.content = "second".into();
        row.is_streaming = true;
        draw_thinking(&mut surface, &mut row);
        check(
            "header===document.querySelector('.maestro-thinking-header') && header.classList.contains('maestro-shimmer') && document.querySelector('.maestro-markdown p').textContent==='second'",
        );
        row.is_streaming = false;
        draw_thinking(&mut surface, &mut row);
        check("!header.classList.contains('maestro-shimmer')");
        let mut second = ThinkingBlock::new(surface.host());
        let host = surface.host();
        surface.draw(|ui, frame| {
            row.render(ui, support::reading_view::view(&host, frame))
                .unwrap();
            second
                .render(ui, support::reading_view::view(&host, frame))
                .unwrap();
        });
        check(
            "document.querySelectorAll('.maestro-thinking-header').length===2 && document.querySelectorAll('.maestro-markdown').length===1",
        );
        let _ = js("header.click()");
        draw_thinking(&mut surface, &mut row);
        check("document.querySelector('.maestro-markdown')===null");
        drop(row);
        check("!header.isConnected");
        let _ = js("header.click()");
        drop(second);
        check("document.querySelector('.maestro-thinking-header')===null");
    }

    /// Draw a tool call without converting its original argument or details operands.
    fn draw_tool<D: 'static>(
        surface: &mut Surface,
        row: &mut ToolMessage,
        props: ToolMessageProps<'_, D>,
    ) {
        surface.draw_reading(|ui, view| row.render(ui, props, view).assert_no_overlays().unwrap());
    }
    /// Model object arguments and synthetic abort policy reach the registered native consumer.
    /// # Panics
    /// Panics if pointer identity, envelope metadata or abort timing differs.
    #[wasm_bindgen]
    pub fn composed_tool_message_forwards_model_and_synthetic_abort() {
        let mut surface = Surface::new();
        let call: maestro_app::ToolCall = serde_json::from_value(serde_json::json!({"type":"toolCall","id":"model","name":"registered","arguments":{"value":1}})).unwrap();
        let pointer = std::ptr::from_ref(&call.arguments) as usize;
        let seen = Rc::new(RefCell::new(Vec::new()));
        let recorded = seen.clone();
        let renderer: Rc<dyn ToolRenderer> = Rc::new(renderer_recipes::Renderer(Rc::new(
            move |args: Option<&maestro_app::JsonObject>, result, stream| {
                assert_eq!(std::ptr::from_ref(args.unwrap()) as usize, pointer);
                let result = result.unwrap();
                assert_eq!(result.tool_call_id(), "model");
                assert_eq!(result.tool_name(), "registered");
                recorded
                    .borrow_mut()
                    .push((stream, result.is_error(), result.timestamp()));
                if result.is_error() {
                    assert!(result.content().is_empty() && result.details().is_none());
                }
                Ok(ToolRenderResult {
                    content: Box::new(renderer_recipes::RichText("**tool output**".into())),
                    is_custom: true,
                })
            },
        )));
        register_tool_renderer("registered", ToolRendererBinding::from_raw(renderer));
        let result: maestro_app::ToolResultMessage = serde_json::from_value(serde_json::json!({"role":"toolResult","toolCallId":"model","toolName":"registered","timestamp":17,"isError":false,"content":[]})).unwrap();
        let mut row = ToolMessage::new();
        draw_tool(
            &mut surface,
            &mut row,
            ToolMessageProps {
                tool_call: &call,
                tool: None,
                result: Some(&result),
                pending: true,
                aborted: false,
                is_streaming: false,
            },
        );
        assert_eq!(seen.borrow()[0], (true, false, 17.));
        check("document.querySelector('.maestro-markdown strong').textContent==='tool output'");
        let before = js_sys::Date::now().floor();
        draw_tool(
            &mut surface,
            &mut row,
            ToolMessageProps {
                tool_call: &call,
                tool: None,
                result: Some(&result),
                pending: true,
                aborted: true,
                is_streaming: true,
            },
        );
        let after = js_sys::Date::now().floor();
        let observed = seen.borrow()[1];
        assert!(!observed.0 && observed.1 && observed.2.fract() == 0.);
        assert!(observed.2 >= before && observed.2 <= after);
        drop(row);
        check("document.querySelector('.maestro-markdown')===null");
    }

    /// Draw original attachment handles through the actual user row.
    fn draw_user(
        surface: &mut Surface,
        row: &mut maestro_web::UserMessage,
        attachments: &[Rc<RefCell<maestro_web::Attachment>>],
    ) {
        let host = surface.host();
        let content = maestro_app::UserContent::Text("**user**".into());
        surface.draw(|ui, frame| {
            row.render(
                ui,
                maestro_web::UserMessageInput::WithAttachments {
                    content: &content,
                    attachments: Some(attachments),
                },
                support::reading_view::view(&host, frame),
            )
            .assert_no_overlays()
            .unwrap()
        });
    }
    /// A retained tile observes mutations of the caller's original attachment record.
    /// # Panics
    /// Panics if the row clones a record snapshot or leaves a stale tile after release.
    #[wasm_bindgen]
    pub fn user_consumer_keeps_original_attachment_handle_without_byte_clones() {
        let mut surface = Surface::new();
        let attachment = Rc::new(RefCell::new(serde_json::from_value::<maestro_web::Attachment>(serde_json::json!({
            "id":"a","type":"image","fileName":"first.png","mimeType":"image/png","content":"","size":0,
            "preview":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg=="
        })).unwrap()));
        let attachments = [attachment.clone()];
        let mut row = maestro_web::UserMessage::new(surface.host());
        draw_user(&mut surface, &mut row, &attachments);
        check(
            "document.querySelector('.maestro-markdown strong').textContent==='user' && document.querySelector('img[alt=\"first.png\"]')!==null",
        );
        attachment.borrow_mut().file_name = "second.png".into();
        draw_user(&mut surface, &mut row, &attachments);
        check(
            "document.querySelector('img[alt=\"second.png\"]')!==null && document.querySelector('img[alt=\"first.png\"]')===null",
        );
        drop(row);
        check("document.querySelector('img[alt=\"second.png\"]')===null");
        assert_eq!(attachment.borrow().file_name, "second.png");
    }

    /// Non-serializable detail handle passed directly to the registered callback.
    struct NativeDetails(Rc<Cell<u32>>);
    /// Native details preserve identity and mutations through real `ToolMessage` dispatch.
    /// # Panics
    /// Panics if native details are converted or detached from their original owner.
    #[wasm_bindgen]
    pub fn composed_tool_message_keeps_non_serializable_native_details() {
        let _: maestro_web::AssistantMessageProps<'_, NativeDetails> =
            maestro_web::AssistantMessageProps::default();
        let mut surface = Surface::new();
        let handle = Rc::new(Cell::new(42));
        let expected = handle.clone();
        let renderer: Rc<dyn ToolRenderer<maestro_app::JsonObject, NativeDetails>> =
            Rc::new(renderer_recipes::Renderer(Rc::new(
                move |_,
                      result: Option<
                    maestro_web::tools::types::ToolResultMessageView<'_, NativeDetails>,
                >,
                      _| {
                    assert!(Rc::ptr_eq(&result.unwrap().details().unwrap().0, &expected));
                    Ok(ToolRenderResult {
                        content: Box::new(renderer_recipes::RichText(format!(
                            "**native details {}**",
                            expected.get()
                        ))),
                        is_custom: true,
                    })
                },
            )));
        register_tool_renderer("native-details", ToolRendererBinding::from_raw(renderer));
        let call = serde_json::from_value(serde_json::json!({"type":"toolCall","id":"native","name":"native-details","arguments":{}})).unwrap();
        let result = maestro_app::ToolResultMessage {
            tool_call_id: "native".into(),
            tool_name: "native-details".into(),
            content: vec![],
            details: Some(NativeDetails(handle.clone())),
            is_error: false,
            timestamp: 0.,
        };
        let mut row = ToolMessage::new();
        handle.set(43);
        draw_tool(
            &mut surface,
            &mut row,
            ToolMessageProps {
                tool_call: &call,
                tool: None,
                result: Some(&result),
                pending: false,
                aborted: false,
                is_streaming: false,
            },
        );
        check(
            "document.querySelector('.maestro-markdown strong').textContent==='native details 43'",
        );
        drop(row);
        check("document.querySelector('.maestro-markdown')===null");
    }
}
