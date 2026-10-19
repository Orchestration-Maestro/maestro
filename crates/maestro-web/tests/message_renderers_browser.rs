//! Registered native messages through the retained rich-text consumer.
#![cfg(all(test, target_arch = "wasm32"))]
#[path = "attachment_support/canvas_paint.rs"]
pub mod canvas_paint;
pub mod markdown_support;
pub mod message_renderer_support;
#[path = "tool_renderer_support/recipes.rs"]
pub mod renderer_recipes;
pub mod support;

#[cfg(test)]
mod tests {
    use super::*;
    use maestro_web::{
        register_message_renderer, render_message,
        tools::types::{ToolRenderResult, ToolRenderState},
    };
    use markdown_support::{Surface, check, js};
    use message_renderer_support::{NativeMessage, Renderer, message};
    use std::{
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicU32, Ordering},
        },
    };
    use wasm_bindgen::prelude::*;

    /// Rich recipes keep native selection, current content and modal/lifetime admission.
    /// # Panics
    /// Panics if registration, native content or retained ownership differs.
    #[wasm_bindgen]
    pub fn custom_message_content_reaches_the_native_reading_consumer() {
        let mut surface = Surface::new();
        let handle = Arc::new(AtomicU32::new(1));
        let input = message("browser-custom", handle.clone());
        register_message_renderer(
            "browser-custom",
            Rc::new(Renderer(Rc::new(|message| {
                let maestro_app::AgentMessage::Custom(value) = message else {
                    unreachable!()
                };
                let value = value.read().unwrap();
                assert_eq!(value.handle.load(Ordering::Relaxed), 42);
                Ok(Box::new(renderer_recipes::RichText(
                    "**Native message** [link](https://example.test/)".into(),
                )))
            }))),
        );
        handle.store(42, Ordering::Relaxed);
        let mut row = ToolRenderState::default();
        draw_registered(&mut surface, &mut row, &input);
        check(
            "document.querySelector('.maestro-markdown strong').textContent==='Native message' && document.querySelector('.maestro-markdown a').href==='https://example.test/'",
        );
        let _ = js(
            "globalThis.customRoot=document.querySelector('.maestro-markdown');const range=document.createRange();range.selectNodeContents(customRoot.querySelector('strong'));getSelection().removeAllRanges();getSelection().addRange(range);globalThis.customSelection=getSelection().toString()",
        );
        draw_registered(&mut surface, &mut row, &input);
        check(
            "document.querySelector('.maestro-markdown')===customRoot && getSelection().toString()===customSelection",
        );
        let modal = egui::Id::new("custom-neighbor");
        surface.host().set_overlay(modal, true).unwrap();
        check(
            "getComputedStyle(customRoot.closest('.maestro-reading-frame')).visibility==='hidden'",
        );
        surface.host().set_overlay(modal, false).unwrap();
        check(
            "getComputedStyle(customRoot.closest('.maestro-reading-frame')).visibility==='visible'",
        );
        register_message_renderer(
            "browser-custom",
            Rc::new(Renderer(Rc::new(|_| {
                Ok(Box::new(renderer_recipes::RichText(
                    "**Replacement message**".into(),
                )))
            }))),
        );
        draw_registered(&mut surface, &mut row, &input);
        check(
            "document.querySelector('.maestro-markdown strong').textContent==='Replacement message' && !customRoot.isConnected",
        );
        drop(row);
        check(
            "document.querySelector('.maestro-markdown')===null && document.querySelectorAll('.maestro-reading-frame').length===0",
        );
    }
    /// Prepare owned content through the public registry, using the ordinary retained row.
    fn draw_registered(
        surface: &mut Surface,
        row: &mut ToolRenderState,
        input: &maestro_app::AgentMessage<NativeMessage>,
    ) {
        let host = surface.host();
        let content = render_message(input).unwrap().unwrap();
        let recipe = ToolRenderResult {
            content,
            is_custom: true,
        };
        surface.draw(|ui, frame| {
            recipe
                .render(ui, support::reading_view::view(&host, frame), row)
                .unwrap()
        });
    }
}
