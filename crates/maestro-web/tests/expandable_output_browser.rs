//! Mounted output disclosure behavior in native browser documents.
#![cfg(all(test, target_arch = "wasm32"))]
#[cfg(test)]
pub(crate) mod tests {
    use crate::{export_mount, markdown_support, reading_slots_support, support};
    use maestro_web::browser::attachment_content::AttachmentHost as _;
    use maestro_web::export_html::template::{
        bind_expandable_output, format_expandable_output, safe_marked_parse,
    };
    use reading_slots_support::{Slots, js};
    use std::rc::Rc;
    use wasm_bindgen::prelude::*;

    /// Generated output binds its root and keeps the guard in the mounted parent.
    #[wasm_bindgen]
    pub fn expandable_outputs_keep_selection_and_mounted_lifetimes() {
        let engine = maestro_theme::SyntectHighlighter::new().unwrap();
        let html = format_expandable_output("one\n\nthree", 1, None, &engine);
        disclosures_keep_selection_and_mounted_lifetimes(&html);
    }

    /// Replay root, selection, live-class and teardown transitions for caller-produced HTML.
    pub(crate) fn disclosures_keep_selection_and_mounted_lifetimes(html: &str) {
        let mut slots = Slots::new();
        let root = export_mount::mount(&mut slots, format!("{html}{html}"));
        let factory = Rc::clone(&slots.content);
        let outputs = root.query_selector_all(".tool-output.expandable").unwrap();
        let first = outputs
            .item(0)
            .unwrap()
            .unchecked_into::<web_sys::Element>();
        let second = outputs
            .item(1)
            .unwrap()
            .unchecked_into::<web_sys::Element>();
        js(
            &root,
            "value.querySelector('.tool-output.expandable').classList.add('retained-class');value.ownerDocument.defaultView.getSelection().removeAllRanges()",
        );
        click(&first);
        assert!(expanded(&first));
        assert!(!expanded(&second));
        js(
            &first,
            "const s=value.ownerDocument.defaultView.getSelection(),r=value.ownerDocument.createRange();r.selectNodeContents(value.querySelector('.output-full'));s.addRange(r)",
        );
        click(&first);
        assert!(expanded(&first));
        js(
            &root,
            "value.ownerDocument.defaultView.getSelection().removeAllRanges()",
        );
        click(&first);
        assert!(!expanded(&first));
        js(&first, "value.classList.add('expanded')");
        click(&first);
        assert!(!expanded(&first));
        js(&first, "value.classList.remove('expanded')");
        click(&first);
        assert!(expanded(&first));
        assert!(first.class_name().contains("retained-class"));
        slots
            .place(
                maestro_web::browser::attachment_content::ReadingPhase::Publish,
                egui::vec2(600., 700.),
                &[],
            )
            .unwrap();
        assert!(expanded(&first));
        root_and_foreign_disclosures(&root);
        slots.host.remove(slots.id);
        click(&first);
        assert!(expanded(&first));
        drop(factory);
    }

    /// Dispatch a native click to a retained container.
    fn click(element: &web_sys::Element) {
        element.unchecked_ref::<web_sys::HtmlElement>().click();
    }
    /// Observe the class slot shared with external writers.
    fn expanded(element: &web_sys::Element) -> bool {
        element.class_list().contains("expanded")
    }

    /// A root-self binding and foreign-document descendants use their own window selection.
    fn root_and_foreign_disclosures(parent: &web_sys::Element) {
        let document = parent.owner_document().unwrap();
        let own = document.create_element("div").unwrap();
        own.set_class_name("tool-output expandable retained-class");
        let guard = bind_expandable_output(&own).unwrap();
        click(&own);
        assert!(expanded(&own));
        drop(guard);
        click(&own);
        assert!(expanded(&own));
        let frame = document
            .create_element("iframe")
            .unwrap()
            .unchecked_into::<web_sys::HtmlIFrameElement>();
        document.body().unwrap().append_child(&frame).unwrap();
        let foreign_document = frame.content_document().unwrap();
        let foreign = foreign_document.create_element("section").unwrap();
        foreign.set_inner_html("<div class='tool-output expandable'>foreign text</div>");
        foreign_document
            .body()
            .unwrap()
            .append_child(&foreign)
            .unwrap();
        let guard = bind_expandable_output(&foreign).unwrap();
        let child = foreign.first_element_child().unwrap();
        js(
            parent,
            "const s=window.getSelection(),r=document.createRange();r.selectNodeContents(value);s.addRange(r)",
        );
        click(&child);
        assert!(expanded(&child));
        js(parent, "window.getSelection().removeAllRanges()");
        js(
            &child,
            "const s=value.ownerDocument.defaultView.getSelection(),r=value.ownerDocument.createRange();r.selectNodeContents(value);s.addRange(r)",
        );
        click(&child);
        assert!(expanded(&child));
        js(
            &child,
            "value.ownerDocument.defaultView.getSelection().removeAllRanges()",
        );
        click(&child);
        assert!(!expanded(&child));
        drop(guard);
        click(&child);
        assert!(!expanded(&child));
        frame.remove();
    }

    /// Plain blank rows reserve inherited line height in short, preview and expanded bodies.
    #[wasm_bindgen]
    pub fn expandable_blank_lines_keep_painted_height() {
        let mut slots = Slots::new();
        let engine = maestro_theme::SyntectHighlighter::new().unwrap();
        let html = [
            format_expandable_output("one\n\nthree", 5, None, &engine),
            format_expandable_output("one\n\nthree", 2, None, &engine),
            format_expandable_output("one\n\nthree\n\nfive", 3, Some("text"), &engine),
        ]
        .join("");
        let root = export_mount::mount(&mut slots, html);
        let short = root.first_element_child().unwrap();
        let second = short.next_element_sibling().unwrap();
        assert_plain_rows(&short, "", 3);
        assert_plain_rows(&second, ".output-preview", 2);
        click(&second);
        assert_plain_rows(&second, ".output-full", 3);
        let code = root.last_element_child().unwrap();
        assert_code_rows(&code, ".output-preview", "one\n\nthree", 3);
        click(&code);
        assert_code_rows(&code, ".output-full", "one\n\nthree\n\nfive", 5);
    }

    /// Every expected plain row is visible, including each explicitly empty row.
    fn assert_plain_rows(root: &web_sys::Element, body: &str, count: usize) {
        let body = if body.is_empty() {
            root.clone()
        } else {
            root.query_selector(body).unwrap().unwrap()
        };
        assert_eq!(js(&body, &format!("const rows=[...value.children].filter(e=>!e.classList.contains('expand-hint'));return rows.length==={count}&&rows[1].textContent===''&&rows.every(e=>e.getBoundingClientRect().height>0&&Math.abs(e.getBoundingClientRect().height-rows[0].getBoundingClientRect().height)<0.01)")).as_bool(), Some(true));
    }

    /// The mounted code body reserves measured line height for its interior empty rows.
    fn assert_code_rows(root: &web_sys::Element, body: &str, text: &str, count: usize) {
        let body = root.query_selector(body).unwrap().unwrap();
        assert_eq!(
            body.query_selector("code")
                .unwrap()
                .unwrap()
                .text_content()
                .as_deref(),
            Some(text)
        );
        assert_eq!(js(&body, &format!("const ordinary=value.closest('.maestro-export').firstElementChild.firstElementChild;const line=ordinary.getBoundingClientRect().height;return line>0&&Math.abs(value.querySelector('pre').getBoundingClientRect().height-line*{count})<0.1")).as_bool(), Some(true));
    }

    /// Selected font metrics match an independently styled native text specimen.
    fn assert_typography(
        root: &web_sys::Element,
        presentation: &maestro_theme::BrandPresentation<'_>,
    ) {
        let typography = &presentation.typography["mono"];
        let font = &presentation.fonts[&typography.font];
        let expected = serde_json::json!({
            "family": font.family,
            "fallbacks": font.fallbacks,
            "size": typography.size,
            "lineHeight": typography.line_height,
            "weight": typography.weight,
        });
        assert!(typography_matches(root, &expected));
    }

    /// CSS text of a recorded number or string value.
    fn css(value: &serde_json::Value) -> String {
        value
            .as_str()
            .map_or_else(|| value.to_string(), str::to_owned)
    }

    /// The bounds of the text in `element`'s first child.
    fn text_bounds(element: &web_sys::Element) -> web_sys::DomRect {
        use crate::browser_fixture_support::invoke;
        let document = element.owner_document().unwrap();
        let range = invoke(&document, "createRange", &[]).unwrap();
        invoke(
            &range,
            "selectNodeContents",
            &[&element.first_child().unwrap()],
        )
        .unwrap();
        invoke(&range, "getBoundingClientRect", &[])
            .unwrap()
            .unchecked_into()
    }

    /// Whether the exported paragraph below `root` measures like an independently styled
    /// specimen of the same text in the `expected` family, fallbacks, size, line height and weight.
    fn typography_matches(root: &web_sys::Element, expected: &serde_json::Value) -> bool {
        use wasm_bindgen::JsCast as _;
        let document = root.owner_document().unwrap();
        let target = root.query_selector(".markdown-content p").unwrap().unwrap();
        let reference: web_sys::HtmlElement =
            document.create_element("div").unwrap().unchecked_into();
        reference.set_text_content(target.first_child().unwrap().text_content().as_deref());
        let style = reference.style();
        style.set_css_text("position:fixed;visibility:hidden;white-space:pre");
        let fallbacks = expected["fallbacks"].as_array().unwrap().iter().map(css);
        let family = std::iter::once(expected["family"].to_string()).chain(fallbacks);
        style
            .set_property("font-family", &family.collect::<Vec<_>>().join(","))
            .unwrap();
        style
            .set_property("font-size", &format!("{}px", css(&expected["size"])))
            .unwrap();
        style
            .set_property("line-height", &css(&expected["lineHeight"]))
            .unwrap();
        style
            .set_property("font-weight", &css(&expected["weight"]))
            .unwrap();
        document.body().unwrap().append_child(&reference).unwrap();
        let (actual, wanted) = (text_bounds(&target), text_bounds(&reference));
        let window = web_sys::window().unwrap();
        let computed = |element: &web_sys::Element, property: &str| {
            let style = window.get_computed_style(element).unwrap().unwrap();
            style.get_property_value(property).unwrap()
        };
        let close = |a: f64, b: f64| (a - b).abs() < 0.01;
        let matches = computed(&target, "font-family") == computed(&reference, "font-family")
            && computed(&target, "font-size") == computed(&reference, "font-size")
            && close(actual.width(), wanted.width())
            && close(actual.height(), wanted.height())
            && close(
                target.get_bounding_client_rect().height(),
                reference.get_bounding_client_rect().height(),
            )
            && wanted.width() > 0.
            && wanted.height() > 0.;
        reference.remove();
        matches
    }

    /// Selected packs paint export roles without importing the chat layout.
    #[wasm_bindgen]
    pub async fn export_rendering_uses_the_selected_brand_roles() {
        let mut slots = Slots::new();
        let engine = markdown_support::Highlighter(vec![
            Some(maestro_theme::ThemeColor::SyntaxComment),
            Some(maestro_theme::ThemeColor::SyntaxKeyword),
            Some(maestro_theme::ThemeColor::SyntaxFunction),
            Some(maestro_theme::ThemeColor::SyntaxVariable),
            Some(maestro_theme::ThemeColor::SyntaxString),
            Some(maestro_theme::ThemeColor::SyntaxNumber),
            Some(maestro_theme::ThemeColor::SyntaxType),
            Some(maestro_theme::ThemeColor::SyntaxOperator),
            Some(maestro_theme::ThemeColor::SyntaxPunctuation),
        ]);
        let markdown = safe_marked_parse(
            "# Heading\n\nMMMM ordinary [link](#target) `inline`\n\n> Quote\n\n- List\n\n---\n\n| default | left | center | right |\n| --- | :--- | :---: | ---: |\n| a | b | c | d |\n\n![mark](/brand.svg)\n\n```javascript\ncomment\nkeyword\nfunction\nvariable\nstring\nnumber\ntype\noperator\npunctuation\n```",
            &engine,
        );
        let output = format_expandable_output("preview\nfull", 1, None, &engine);
        let long = "wide ".repeat(100);
        let html = format!(
            "<div class='markdown-content'>{markdown}</div>{output}<div class='tool-output'><div class='plain-row'>{long}</div><div class='ansi-line'><span style='background:var(--maestro-color-accent);color:var(--maestro-color-background);text-decoration:underline'>{long}</span></div></div>"
        );
        for pack in ["brand.json", "signal.json"] {
            slots.pack = support::brand::pack(pack).unwrap();
            for mode in [
                maestro_theme::BrandMode::Dark,
                maestro_theme::BrandMode::Light,
            ] {
                slots.mode = mode;
                let root = export_mount::mount(&mut slots, html.clone());
                let presentation = slots.pack.presentation(mode).unwrap();
                assert_typography(&root, &presentation);
                let colors: serde_json::Map<String, serde_json::Value> = presentation
                    .colors
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).into()))
                    .collect();
                js_sys::Reflect::set(
                    &js_sys::global(),
                    &"colors".into(),
                    &serde_json::Value::Object(colors).to_string().into(),
                )
                .unwrap();
                assert_eq!(js(&root, "const cells=[...value.querySelectorAll('th')];return cells.every((e,i)=>{const r=e.ownerDocument.createRange();r.selectNodeContents(e);const t=r.getBoundingClientRect(),b=e.getBoundingClientRect();return i<2?t.left-b.left<12:i===2?Math.abs(t.left+t.right-b.left-b.right)<2:b.right-t.right<12})").as_bool(), Some(true));
                assert_eq!(js(&root, "const p=value.querySelector('.plain-row'),a=value.querySelector('.ansi-line');return p.getBoundingClientRect().height>a.getBoundingClientRect().height&&a.scrollWidth>a.clientWidth&&getComputedStyle(a).whiteSpace==='pre'").as_bool(), Some(true));
                support::reading_paint::observe("export-roles").await;
            }
        }
    }
}
