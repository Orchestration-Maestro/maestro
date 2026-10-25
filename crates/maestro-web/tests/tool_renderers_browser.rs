//! Real-browser tool recipes using the retained native reading host.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::markdown_support;
use crate::support::tool_output::TextOnlyOutput as _;
use crate::tool_renderer_support;

pub(crate) use tests::{Aliased, surface};

#[cfg(test)]
mod tests {
    use super::*;
    use maestro_web::{
        render_tool,
        tools::types::{ToolParams, ToolRenderState, ToolRenderView},
    };
    use markdown_support::{Surface, check, js, publish};
    use serde_json::Value;
    use wasm_bindgen::prelude::*;

    pub(crate) fn surface() -> Surface {
        Surface::new()
    }

    #[wasm_bindgen]
    pub fn default_tool_renderer_preserves_all_recorded_branches() {
        let mut surface = surface();
        let fixture: Value =
            serde_json::from_str(include_str!("fixtures/tool_renderers.json")).unwrap();
        for row in fixture["rows"].as_array().unwrap() {
            let input = &row["input"];
            let message = input.get("result").map(|r| {
                serde_json::from_value::<maestro_app::ToolResultMessage>(r.clone()).unwrap()
            });
            let params = input.get("params").map(ToolParams::json);
            let recipe = render_tool::<Value, Value>(
                "unregistered",
                params,
                message.as_ref(),
                input["isStreaming"].as_bool().unwrap(),
            )
            .unwrap();
            assert!(!recipe.is_custom);
            let mut retained = ToolRenderState::default();
            tool_renderer_support::browser::draw(&mut surface, &recipe, &mut retained);
            publish(
                "expectedTool",
                &serde_json::to_string(&row["expected"]).unwrap().into(),
            );
            check(
                "document.querySelector('.maestro-tool-caption').textContent === JSON.parse(expectedTool).header",
            );
            check(
                "JSON.stringify([...document.querySelectorAll('.maestro-tool-label')].map(n=>n.textContent)) === JSON.stringify(JSON.parse(expectedTool).labels)",
            );
            check(
                "JSON.stringify([...document.querySelectorAll('.maestro-code')].map(n=>({code:n.querySelector('code').textContent,language:n.querySelector('.maestro-code-label').textContent}))) === JSON.stringify(JSON.parse(expectedTool).blocks)",
            );
            check("document.querySelectorAll('.maestro-code script').length === 0");
            tool_renderer_support::browser::default_layout();
            drop(retained);
            check("document.querySelectorAll('.maestro-code, .maestro-tool-caption').length === 0");
        }
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
        tool_renderer_support::browser::console_frame(surface, console);
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
        tool_renderer_support::browser::console_frame(surface, console);
        assert!(
            surface
                .painted
                .iter()
                .any(|(text, _)| Some(text.as_str()) == expected["copiedText"].as_str())
        );
        let _ = js("consoleRestores.pop()()");
        tool_renderer_support::browser::console_frame(surface, console);
        assert!(!surface.painted.iter().any(|(text, _)| text == "Copied!"));
    }

    /// Keep an explicit alias of the same typed owner passed to native child recipes.
    pub(crate) struct Aliased {
        pub(crate) inner: Box<dyn maestro_web::tools::types::ToolContent>,
        pub(crate) alias: std::rc::Rc<std::cell::RefCell<Option<maestro_app::ToolRendererState>>>,
    }
    impl maestro_web::tools::types::ToolContent for Aliased {
        fn render(
            &self,
            ui: &mut egui::Ui,
            view: ToolRenderView<'_>,
            state: &maestro_app::ToolRendererState,
        ) -> maestro_web::tools::types::ToolContentOutput {
            let old = self.alias.replace(Some(state.clone()));
            drop(old);
            self.inner.render(ui, view, state)
        }
    }
    struct Replacement;
    impl maestro_web::tools::types::ToolContent for Replacement {
        fn render(
            &self,
            ui: &mut egui::Ui,
            _: ToolRenderView<'_>,
            _: &maestro_app::ToolRendererState,
        ) -> maestro_web::tools::types::ToolContentOutput {
            let response = Ok(ui.label("replacement"));
            maestro_web::tools::types::ToolContentOutput {
                response,
                overlays: Vec::new(),
            }
        }
    }

    /// Inactive native content becomes invisible even while a state alias keeps it alive.
    #[wasm_bindgen]
    pub fn tool_content_preserves_native_roots_until_row_or_recipe_changes() {
        use maestro_web::tools::types::ToolRenderResult;
        let mut surface = surface();
        let alias = std::rc::Rc::new(std::cell::RefCell::new(None));
        let input = serde_json::json!({"value":42});
        let inner = render_tool::<Value, Value>(
            "unregistered",
            Some(ToolParams::json(&input)),
            None,
            false,
        )
        .unwrap();
        let recipe: ToolRenderResult = ToolRenderResult {
            content: Box::new(Aliased {
                inner: inner.content,
                alias: alias.clone(),
            }),
            is_custom: false,
        };
        let mut retained = ToolRenderState::default();
        tool_renderer_support::browser::draw(&mut surface, &recipe, &mut retained);
        let _ = js(
            "globalThis.oldToolRoot=document.querySelector('.maestro-code'); if (!oldToolRoot) throw new Error('missing native listing')",
        );
        drop(recipe);
        let replacement: ToolRenderResult = ToolRenderResult {
            content: Box::new(Replacement),
            is_custom: true,
        };
        tool_renderer_support::browser::draw(&mut surface, &replacement, &mut retained);
        assert!(alias.borrow().is_some());
        check("!oldToolRoot.isConnected || getComputedStyle(oldToolRoot).visibility === 'hidden'");
        let before_release = surface.effects.removed.borrow().len();
        let old = alias.borrow_mut().take();
        drop(old);
        assert!(surface.effects.removed.borrow().len() > before_release);
        check("!oldToolRoot.isConnected");
    }

    /// A separately retained recipe owns its native widget state independently of its parent.
    struct Nested {
        recipe: maestro_web::tools::types::ToolRenderResult,
        row: std::cell::RefCell<ToolRenderState>,
    }
    impl maestro_web::tools::types::ToolContent for Nested {
        fn render(
            &self,
            ui: &mut egui::Ui,
            view: ToolRenderView<'_>,
            _: &maestro_app::ToolRendererState,
        ) -> maestro_web::tools::types::ToolContentOutput {
            let mut row = self.row.take();
            let result = self.recipe.render(ui, view, &mut row);
            let old = self.row.replace(row);
            drop(old);
            result
        }
    }

    /// Detaching an outer recipe also detaches independently owned nested native content.
    #[wasm_bindgen]
    pub fn retained_nested_tool_recipe_detaches_with_outer_row() {
        use maestro_web::tools::types::ToolRenderResult;
        let mut surface = surface();
        let input = serde_json::json!({"nested":42});
        let inner = render_tool::<Value, Value>(
            "unregistered",
            Some(ToolParams::json(&input)),
            None,
            false,
        )
        .unwrap();
        let recipe: ToolRenderResult = ToolRenderResult {
            content: Box::new(Nested {
                recipe: inner,
                row: std::cell::RefCell::default(),
            }),
            is_custom: false,
        };
        let mut retained = ToolRenderState::default();
        tool_renderer_support::browser::draw(&mut surface, &recipe, &mut retained);
        let _ = js(
            "globalThis.nestedToolRoot=document.querySelector('.maestro-code'); if (!nestedToolRoot) throw new Error('missing nested listing')",
        );
        let replacement: ToolRenderResult = ToolRenderResult {
            content: Box::new(Replacement),
            is_custom: true,
        };
        tool_renderer_support::browser::draw(&mut surface, &replacement, &mut retained);
        check(
            "!nestedToolRoot.isConnected || getComputedStyle(nestedToolRoot).visibility === 'hidden'",
        );
        tool_renderer_support::browser::draw(&mut surface, &recipe, &mut retained);
        check(
            "nestedToolRoot.isConnected && document.querySelector('.maestro-code') === nestedToolRoot",
        );
        drop(retained);
        check("!nestedToolRoot.isConnected");
        drop(recipe);
    }

    /// Failed re-entry must not reconnect the previous visible native placement.
    #[wasm_bindgen]
    pub fn detached_tool_reentry_failure_keeps_native_root_detached() {
        use maestro_web::tools::types::ToolRenderResult;
        let mut surface = surface();
        let input = serde_json::json!({"nested":42});
        let recipe: ToolRenderResult = ToolRenderResult {
            content: Box::new(Nested {
                recipe: render_tool::<Value, Value>(
                    "unknown",
                    Some(ToolParams::json(&input)),
                    None,
                    false,
                )
                .unwrap(),
                row: std::cell::RefCell::default(),
            }),
            is_custom: false,
        };
        let mut row = ToolRenderState::default();
        tool_renderer_support::browser::draw(&mut surface, &recipe, &mut row);
        let _ = js(
            "globalThis.detachedRoot=document.querySelector('.maestro-tool-caption'); globalThis.detachedFrame=detachedRoot.closest('.maestro-reading-frame')",
        );
        let replacement: ToolRenderResult = ToolRenderResult {
            content: Box::new(Replacement),
            is_custom: true,
        };
        tool_renderer_support::browser::draw(&mut surface, &replacement, &mut row);
        check("!detachedFrame.isConnected");
        let _ = js("document.getElementById('canvas').width=0");
        let host = surface.host();
        let result = surface.draw(|ui, frame| {
            recipe.render(
                ui,
                tool_renderer_support::browser::view(&host, frame),
                &mut row,
            )
        });
        assert!(
            result.assert_no_overlays().is_err(),
            "zero-width canvas rejects placement"
        );
        check("!detachedFrame.isConnected");
        check("!detachedRoot.isConnected");
        let _ = js("document.getElementById('canvas').width=800");
        tool_renderer_support::browser::draw(&mut surface, &recipe, &mut row);
        check(
            "detachedRoot.isConnected && document.querySelector('.maestro-tool-caption') === detachedRoot",
        );
    }

    struct SharedNested(std::rc::Rc<Nested>);
    impl maestro_web::tools::types::ToolContent for SharedNested {
        fn render(
            &self,
            ui: &mut egui::Ui,
            view: ToolRenderView<'_>,
            state: &maestro_app::ToolRendererState,
        ) -> maestro_web::tools::types::ToolContentOutput {
            self.0.render(ui, view, state)
        }
    }
    #[wasm_bindgen]
    pub fn native_replacement_refreshes_placement_ancestry() {
        use maestro_web::tools::types::ToolRenderResult;
        let mut surface = surface();
        let input = serde_json::json!({"replaced":42});
        let nested = std::rc::Rc::new(Nested {
            recipe: render_tool::<Value, Value>(
                "unknown",
                Some(ToolParams::json(&input)),
                None,
                false,
            )
            .unwrap(),
            row: std::cell::RefCell::default(),
        });
        let first: ToolRenderResult = ToolRenderResult {
            content: Box::new(SharedNested(nested.clone())),
            is_custom: true,
        };
        let second: ToolRenderResult = ToolRenderResult {
            content: Box::new(SharedNested(nested.clone())),
            is_custom: true,
        };
        let mut old_row = ToolRenderState::default();
        let mut new_row = ToolRenderState::default();
        tool_renderer_support::browser::draw(&mut surface, &first, &mut old_row);
        let _ = js("globalThis.movedRoot=document.querySelector('.maestro-code')");
        check("movedRoot !== null && movedRoot.isConnected");
        tool_renderer_support::browser::draw(&mut surface, &second, &mut new_row);
        drop(old_row);
        check("movedRoot.isConnected && getComputedStyle(movedRoot).visibility !== 'hidden'");
        drop(new_row);
        check("!movedRoot.isConnected");
        assert!(surface.effects.removed.borrow().is_empty());
        drop(first);
        drop(second);
        drop(nested);
        assert!(!surface.effects.removed.borrow().is_empty());
        check("document.querySelectorAll('.maestro-code').length === 0");
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
