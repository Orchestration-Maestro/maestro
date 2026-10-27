//! Real-browser tool recipes using the retained native reading host.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::markdown_support;
use crate::tool_renderer_support;

mod page;

#[cfg(test)]
mod tests {
    use super::*;
    use markdown_support::{Surface, check, js, publish};
    use serde_json::Value;
    use wasm_bindgen::prelude::*;

    fn surface() -> Surface {
        Surface::new()
    }

    /// Actual native clipboard consumer rejects writing even when selection would succeed.
    #[wasm_bindgen]
    pub async fn clipboard_direct_write_does_not_admit_fallback() {
        use maestro_web::browser::attachment_content::{
            AttachmentHost as _, BrowserAttachmentHost,
        };
        use wasm_bindgen::JsCast as _;
        let _surface = surface();
        let _ = js(
            "globalThis.copyFallbacks=0; Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:()=>Promise.reject(new Error('controlled rejection'))}}); document.execCommand=()=>{copyFallbacks++;return true}",
        );
        let canvas = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id("canvas")
            .unwrap()
            .dyn_into()
            .unwrap();
        let host = BrowserAttachmentHost::new(canvas).unwrap();
        assert!(
            !host
                .copy_to_clipboard(
                    "literal clipboard",
                    maestro_web::browser::attachment_content::ClipboardMode::Direct
                )
                .await,
            "direct failure must stay failed"
        );
        check("copyFallbacks === 0");
    }

    /// A rejected direct write never turns the visible copy control into success.
    #[wasm_bindgen]
    pub async fn copy_button_direct_failure_stays_idle() {
        use maestro_web::{
            browser::attachment_content::BrowserAttachmentHost, components::copy_button::CopyButton,
        };
        use std::rc::Rc;
        use wasm_bindgen::JsCast as _;
        let mut surface = surface();
        let _ = js(
            "globalThis.copyFallbacks=0; globalThis.copyDiagnostics=0; globalThis.copySettled=new Promise(resolve=>{ const timer=window.setTimeout.bind(window); window.setTimeout=(fn,ms,...args)=>{if(ms===2000)resolve(); return timer(fn,ms,...args)}; const error=console.error.bind(console); console.error=(...args)=>{copyDiagnostics++;error(...args);resolve()}; }); Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:()=>Promise.reject(new Error('controlled rejection'))}}); document.execCommand=()=>{copyFallbacks++;return true}",
        );
        let canvas = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id("canvas")
            .unwrap()
            .dyn_into()
            .unwrap();
        let host = Rc::new(BrowserAttachmentHost::new(canvas).unwrap());
        let mut button = CopyButton::new(host, &surface.i18n).unwrap();
        button.show_text = true;
        let response =
            surface.draw(|ui, view| button.render(ui, view.i18n, view.presentation).unwrap());
        surface.click(response.rect.center(), |ui, view| {
            button.render(ui, view.i18n, view.presentation).unwrap()
        });
        wasm_bindgen_futures::JsFuture::from(
            js("copySettled").dyn_into::<js_sys::Promise>().unwrap(),
        )
        .await
        .unwrap();
        surface.draw(|ui, view| button.render(ui, view.i18n, view.presentation).unwrap());
        assert!(
            !surface.painted.iter().any(|(text, _)| text == "Copied!"),
            "failed write must not paint copied feedback"
        );
        check("copyFallbacks === 0 && copyDiagnostics === 1");
    }

    /// The native console body retains literal text, including markup and empty output.
    #[wasm_bindgen]
    pub async fn console_block_preserves_literal_output_and_error_variant() {
        use maestro_web::ConsoleBlock;
        let mut surface = surface();
        let _ = js(
            "globalThis.consoleRestores=[]; const original=window.setTimeout.bind(window); window.setTimeout=(fn,delay,...args)=>{if(delay===1500){consoleRestores.push(fn);return 9000+consoleRestores.length} return original(fn,delay,...args)}",
        );
        let mut console = ConsoleBlock::new(surface.host());
        for case in tool_renderer_support::cases()["console"]
            .as_array()
            .unwrap()
        {
            console_case(&mut surface, &mut console, case).await;
        }
        drop(console);
        check("document.querySelectorAll('.maestro-console').length === 0");
    }

    async fn console_case(
        surface: &mut Surface,
        console: &mut maestro_web::ConsoleBlock,
        case: &Value,
    ) {
        use maestro_web::components::console_block::ConsoleVariant;
        let expected = &case["expected"];
        console.content = case["content"].as_str().unwrap().into();
        console.variant = if case["variant"] == "error" {
            ConsoleVariant::Error
        } else {
            ConsoleVariant::Default
        };
        console_frame(surface, console);
        publish("expectedConsole", &expected.to_string().into());
        check(
            "document.querySelector('.maestro-console pre').textContent === JSON.parse(expectedConsole).pre",
        );
        check("document.querySelectorAll('.maestro-console pre b').length === 0");
        check(
            "document.querySelector('.maestro-console-label').textContent === JSON.parse(expectedConsole).label",
        );
        let color = js("getComputedStyle(document.querySelector('.maestro-console pre')).color")
            .as_string()
            .unwrap();
        let pack = surface.pack.clone();
        let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
        let wanted =
            egui::Color32::from_hex(presentation.colors[expected["textRole"].as_str().unwrap()])
                .unwrap();
        assert_eq!(
            color,
            format!("rgb({}, {}, {})", wanted.r(), wanted.g(), wanted.b())
        );
        let at = markdown_support::first_slot_center();
        surface.click(at, |ui, frame| {
            console
                .render(ui, frame.i18n, frame.presentation, frame.modal)
                .unwrap()
        });
        assert!(surface.output.iter().any(|event| matches!(event, egui::output::OutputEvent::Clicked(info) if info.label.as_deref() == expected["title"].as_str())));
        markdown_support::settle().await;
        console_frame(surface, console);
        assert!(
            surface
                .painted
                .iter()
                .any(|(text, _)| Some(text.as_str()) == expected["copiedText"].as_str())
        );
        let _ = js("consoleRestores.pop()()");
        console_frame(surface, console);
        assert!(!surface.painted.iter().any(|(text, _)| text == "Copied!"));
    }

    /// Draw the same console through the shared native frame owner.
    fn console_frame(surface: &mut Surface, console: &mut maestro_web::ConsoleBlock) {
        surface.draw(|ui, frame| {
            console
                .render(ui, frame.i18n, frame.presentation, frame.modal)
                .unwrap()
        });
    }

    /// Natural native content must grow a short horizontal line instead of clipping its control.
    #[wasm_bindgen]
    pub fn native_reading_grows_a_horizontal_caption_line() {
        let mut surface = surface();
        let mut block = maestro_web::components::code_block::CodeBlock::new(surface.host());
        block.code = "caption file.txt".into();
        surface.draw(|ui, view| {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 16.),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_height(16.);
                    block.render(ui, view).unwrap()
                },
            )
        });
        check(
            "document.querySelector('.maestro-reading-frame').getBoundingClientRect().height >= document.querySelector('.control-slot').getBoundingClientRect().height",
        );
    }
}
