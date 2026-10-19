//! Mounted offline cards use the existing native reading and disclosure owners.
#![cfg(all(test, target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use crate::{
        export_mount,
        offline_tool_card_support::document,
        reading_slots_support::{Slots, js},
        support,
    };
    use maestro_theme::BrandMode;
    use maestro_web::export_html::template::render_tool_call;
    use serde_json::{Value, json};
    use wasm_bindgen::prelude::*;

    /// Produce a complete card at the public formatter seam.
    fn card(name: &str, arguments: Value, result: Option<Value>) -> String {
        let entries = result.map_or_else(Vec::new, |message| {
            vec![json!({"type":"message","message":message})]
        });
        let doc = document(&json!({"entries":entries}));
        render_tool_call(
            &doc,
            &json!({"id":"c","name":name,"arguments":arguments}),
            &crate::highlighter_support::NoHighlight,
        )
        .unwrap()
    }
    /// Supplied bodies reuse the full existing root, selection and release regression.
    #[wasm_bindgen]
    pub fn tool_card_disclosures_keep_selection_and_independent_roots() {
        let doc = document(
            &json!({"entries":[],"renderedTools":{"c":{"callHtml":"<b>Call</b>","resultHtmlCollapsed":"collapsed body","resultHtmlExpanded":"expanded body"}}}),
        );
        let html = render_tool_call(
            &doc,
            &json!({"id":"c","name":"custom"}),
            &crate::highlighter_support::NoHighlight,
        )
        .unwrap();
        crate::expandable_output_browser::tests::disclosures_keep_selection_and_mounted_lifetimes(
            html,
        );
    }
    /// Decoded images, unwrapped diffs and selectable blank output survive native layout.
    #[wasm_bindgen]
    pub async fn tool_card_image_and_diff_layout_uses_native_content() {
        use base64::Engine as _;
        let image = base64::engine::general_purpose::STANDARD.encode("<svg xmlns='http://www.w3.org/2000/svg' width='100' height='1000'><rect width='100' height='1000' fill='black'/></svg>");
        let read = card(
            "read",
            json!({"path":"image"}),
            Some(
                json!({"role":"toolResult","toolCallId":"c","content":[{"type":"image","mimeType":"image/svg+xml","data":image},{"type":"text","text":"one\n\nthree"}]}),
            ),
        );
        let diff = card(
            "edit",
            json!({"path":"file"}),
            Some(
                json!({"role":"toolResult","toolCallId":"c","details":{"diff":format!("+{}\n-removed\n context", "wide ".repeat(100))}}),
            ),
        );
        let mut slots = Slots::new();
        let root = export_mount::mount(&mut slots, format!("{read}{diff}"));
        wasm_bindgen_futures::JsFuture::from(
            js(
                &root,
                "return Promise.all([...value.querySelectorAll('img')].map(i=>i.decode()))",
            )
            .unchecked_into::<js_sys::Promise>(),
        )
        .await
        .unwrap();
        assert_eq!(js(&root, "const i=value.querySelector('img'),b=i.getBoundingClientRect();return Math.abs(b.width-50)<.01&&Math.abs(b.height-500)<.01").as_bool(), Some(true));
        assert_eq!(js(&root, "const d=value.querySelector('.tool-diff'),line=d.firstElementChild;return d.scrollWidth>d.clientWidth&&line.getBoundingClientRect().height<40&&getComputedStyle(d).whiteSpace==='pre'").as_bool(), Some(true));
        assert_eq!(js(&root, "const o=value.querySelector('.tool-output');const r=o.ownerDocument.createRange();r.selectNodeContents(o);o.ownerDocument.defaultView.getSelection().addRange(r);return o.children[1].textContent===''&&o.children[1].getBoundingClientRect().height>0&&r.toString().includes('one')&&r.toString().includes('three')").as_bool(), Some(true));
        js(
            &root,
            "value.ownerDocument.defaultView.getSelection().removeAllRanges()",
        );
        support::reading_paint::observe("offline-tool-card-layout").await;
        js(&root, "value.style.width='80px'");
        assert_eq!(js(&root, "const i=value.querySelector('img'),p=i.parentElement.getBoundingClientRect();return i.getBoundingClientRect().width<=p.width&&i.getBoundingClientRect().width>0").as_bool(), Some(true));
    }
    /// Actual selected-pack roles and supplied decorated spaces reach the final paint clip.
    #[wasm_bindgen]
    pub async fn tool_cards_use_selected_brand_roles_without_changing_custom_html() {
        let result = |error| json!({"role":"toolResult","toolCallId":"c","isError":error,"content":[{"type":"text","text":"output"}]});
        let pending = card("bash", json!({"command":"pending"}), None);
        let success = card(
            "read",
            json!({"path":"/home/user/file","offset":2,"limit":3}),
            Some(result(false)),
        );
        let error = card(
            "write",
            json!({"path":true,"content":17}),
            Some(result(true)),
        );
        let count = card(
            "write",
            json!({"path":"count", "content":"line\n".repeat(10)}),
            None,
        );
        let diff = card(
            "edit",
            json!({"path":"diff"}),
            Some(
                json!({"role":"toolResult","toolCallId":"c","details":{"diff":"+MMMM added\n-MMMM removed\n MMMM context"}}),
            ),
        );
        let doc = document(
            &json!({"entries":[],"renderedTools":{"c":{"callHtml":"<span style='background:#123456;color:#abcdef;text-decoration:underline;white-space:pre'>    </span>","resultHtmlCollapsed":"short","resultHtmlExpanded":"full output"}}}),
        );
        let custom = render_tool_call(
            &doc,
            &json!({"id":"c","name":"custom"}),
            &crate::highlighter_support::NoHighlight,
        )
        .unwrap();
        let mut slots = Slots::new();
        for pack in ["brand.json", "signal.json"] {
            slots.pack = maestro_theme::load_brand_pack(pack, &support::brand::Pack).unwrap();
            for mode in [BrandMode::Dark, BrandMode::Light] {
                slots.mode = mode;
                export_mount::mount(
                    &mut slots,
                    format!("{pending}{success}{error}{count}{diff}{custom}"),
                );
                let presentation = slots.pack.presentation(mode).unwrap();
                let colors: serde_json::Map<String, Value> = presentation
                    .colors
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).into()))
                    .collect();
                js_sys::Reflect::set(
                    &js_sys::global(),
                    &"colors".into(),
                    &Value::Object(colors).to_string().into(),
                )
                .unwrap();
                support::reading_paint::observe("offline-tool-card-roles").await;
            }
        }
    }
}
