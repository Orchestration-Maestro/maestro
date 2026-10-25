//! Debug records observed through literal native code listings.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{markdown_support, support};

use maestro_web::ToolMessageDebugView;
use markdown_support::{Surface, check, publish};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// Publish one composed debug frame through the shared reading harness.
    fn draw(surface: &mut Surface, row: &mut ToolMessageDebugView) {
        let host = surface.host();
        surface.draw(|ui, frame| {
            row.render(ui, support::reading_view::view(&host, frame))
                .unwrap()
        });
    }
    /// Build an empty result with independently selected details.
    fn result(details: Option<Value>) -> maestro_app::ToolResultMessage {
        maestro_app::ToolResultMessage {
            tool_call_id: "call".into(),
            tool_name: "tool".into(),
            content: vec![],
            details,
            is_error: false,
            timestamp: 1.,
        }
    }
    /// Observe displayed and copied source plus the actual native language label.
    fn listing(index: usize, text: &str, language: &str) {
        publish("listingIndex", &u32::try_from(index).unwrap().into());
        publish("listingText", &text.into());
        publish("listingLanguage", &language.into());
        check(
            "document.querySelectorAll('.maestro-code code')[listingIndex].textContent === listingText",
        );
        check(
            "document.querySelectorAll('.maestro-code-label')[listingIndex].textContent === listingLanguage",
        );
    }

    /// Missing, projected JSON and literal text reach distinct code children.
    /// # Panics
    /// Panics if rendering or a native observation fails.
    #[wasm_bindgen]
    pub fn debug_values_keep_missing_null_and_json_text_distinct() {
        let mut surface = Surface::new();
        let mut row = ToolMessageDebugView::new(surface.host());
        row.has_result = true;
        for (input, text, language) in [
            (None, "", "json"),
            (Some(Value::Null), "null", "json"),
            (Some(json!(false)), "false", "json"),
            (Some(json!(0)), "0", "json"),
            (Some(json!(-0.0)), "0", "json"),
            (Some(json!(1e21)), "1e+21", "json"),
            (Some(json!("")), "", "text"),
            (Some(json!("literal <b>&")), "literal <b>&", "text"),
            (Some(json!("null")), "null", "json"),
            (Some(json!("[1,true]")), "[\n  1,\n  true\n]", "json"),
            (
                Some(json!({"2":"two","1":"one","z":0})),
                "{\n  \"1\": \"one\",\n  \"2\": \"two\",\n  \"z\": 0\n}",
                "json",
            ),
            (Some(json!([1, false])), "[\n  1,\n  false\n]", "json"),
            (Some(json!("1e309")), "null", "json"),
            (Some(json!("-1e309")), "null", "json"),
            (Some(json!("{\"n\":1e309}")), "{\n  \"n\": null\n}", "json"),
            (Some(json!("1e308")), "1e+308", "json"),
            (
                Some(json!("{\"b\":1,\"a\":2,\"b\":3}")),
                "{\n  \"b\": 3,\n  \"a\": 2\n}",
                "json",
            ),
            (
                Some(json!("{\"x\":\"\\ud800\",\"x\":\"ok\"}")),
                "{\n  \"x\": \"ok\"\n}",
                "json",
            ),
            (Some(json!("\"raw\ncontrol\"")), "\"raw\ncontrol\"", "text"),
            (Some(json!("\u{feff}null")), "\u{feff}null", "text"),
            (Some(json!("\u{85}null")), "\u{85}null", "text"),
        ] {
            row.call_args.clone_from(&input);
            row.result = Some(result(input));
            draw(&mut surface, &mut row);
            listing(0, text, "json");
            listing(2, text, language);
        }
        check("document.querySelectorAll('.maestro-code b').length === 0");
    }

    /// Text blocks retain empty positions and duplicate text around images.
    /// # Panics
    /// Panics if rendering or a native observation fails.
    #[wasm_bindgen]
    pub fn debug_text_output_keeps_only_ordered_text_blocks() {
        let mut surface = Surface::new();
        let mut row = ToolMessageDebugView::new(surface.host());
        row.has_result = true;
        let text = |value: &str| json!({"type":"text","text":value});
        let image = json!({"type":"image","data":"AA==","mimeType":"image/png"});
        for (blocks, expected, language) in [
            (json!([]), "", "text"),
            (json!([image.clone()]), "", "text"),
            (json!([text(""), text("second")]), "\nsecond", "text"),
            (
                json!([text("one"), image, text("two"), text("one")]),
                "one\ntwo\none",
                "text",
            ),
            (json!([text("{\"x\":1}")]), "{\n  \"x\": 1\n}", "json"),
            (json!([text("raw <b>&")]), "raw <b>&", "text"),
        ] {
            let mut payload = result(None);
            payload.content = serde_json::from_value(blocks).unwrap();
            row.result = Some(payload);
            draw(&mut surface, &mut row);
            listing(1, expected, language);
        }
    }

    /// All three debug code children copy the original base64-looking text.
    /// # Panics
    /// Panics if rendering, clipboard completion or a native observation fails.
    #[wasm_bindgen]
    pub async fn debug_listings_never_decode_literal_base64() {
        let mut surface = Surface::new();
        let mut row = ToolMessageDebugView::new(surface.host());
        row.call_args = Some(json!("YWJj"));
        row.has_result = true;
        let mut payload = result(Some(json!("YWJj")));
        payload.content = serde_json::from_value(json!([{"type":"text","text":"YWJj"}])).unwrap();
        row.result = Some(payload);
        draw(&mut surface, &mut row);
        draw(&mut surface, &mut row);
        let host = surface.host();
        for index in 0..3 {
            listing(index, "YWJj", if index == 0 { "json" } else { "text" });
            surface.click(markdown_support::slot_center(index), |ui, frame| {
                row.render(ui, support::reading_view::view(&host, frame))
                    .unwrap()
            });
            wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&JsValue::NULL))
                .await
                .unwrap();
            draw(&mut surface, &mut row);
        }
        assert_eq!(*surface.effects.copies.borrow(), ["YWJj", "YWJj", "YWJj"]);
    }

    /// Result visibility does not imply that a payload exists.
    /// # Panics
    /// Panics if rendering or a native observation fails.
    #[wasm_bindgen]
    pub fn debug_result_presence_is_independent_of_the_payload() {
        let mut surface = Surface::new();
        let mut row = ToolMessageDebugView::new(surface.host());
        for (language, call, result_label, absent) in [
            ("en", "Call", "Result", "(no result)"),
            ("de", "Aufruf", "Ergebnis", "(kein Ergebnis)"),
        ] {
            surface.i18n.set_language(language).unwrap();
            for payload in [None, Some(result(None))] {
                row.result = payload;
                row.has_result = false;
                draw(&mut surface, &mut row);
                publish("debugCall", &call.into());
                publish("debugResult", &result_label.into());
                publish("debugAbsent", &absent.into());
                check(
                    "document.querySelectorAll('.maestro-tool-label')[0].textContent === debugCall",
                );
                check(
                    "document.querySelectorAll('.maestro-tool-label')[1].textContent === debugResult",
                );
                check(
                    "document.querySelector('.maestro-tool-detail').textContent === debugAbsent && getComputedStyle(document.querySelector('.maestro-tool-detail')).fontWeight === '400'",
                );
                check("document.querySelectorAll('.maestro-code').length === 1");
                row.has_result = true;
                draw(&mut surface, &mut row);
                check("document.querySelectorAll('.maestro-code').length === 3");
                listing(1, "", "text");
                listing(2, "", "json");
            }
        }
    }
}
