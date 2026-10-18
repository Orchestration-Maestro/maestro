//! Mounted output disclosure behavior in native browser documents.
#![cfg(all(test, target_arch = "wasm32"))]
#[path = "attachment_support/canvas_paint.rs"]
pub mod canvas_paint;
#[path = "export_support/browser.rs"]
mod export_mount;
pub mod markdown_support;
pub mod reading_slots_support;
mod support;
#[cfg(test)]
mod tests {
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
        let mut slots = Slots::new();
        let engine = maestro_theme::SyntectHighlighter::new().unwrap();
        let html = format_expandable_output("one\n\nthree", 1, None, &engine);
        let root = export_mount::mount(&mut slots, format!("{html}{html}"));
        let factory = Rc::clone(&slots.content);
        let first = root.first_element_child().unwrap();
        let second = first.next_element_sibling().unwrap();
        js(
            &root,
            "value.firstElementChild.classList.add('retained-class');value.ownerDocument.defaultView.getSelection().removeAllRanges()",
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
            format_expandable_output("one\n\nthree", 1, Some("text"), &engine),
        ]
        .join("");
        let root = export_mount::mount(&mut slots, html);
        let verify = "const rows=[...value.querySelectorAll('.tool-output>div:not(.output-preview):not(.output-full),.output-preview>div:not(.expand-hint),.expanded .output-full>div')].filter(e=>e.getBoundingClientRect().height>0);const blank=rows.filter(e=>e.textContent==='');return blank.length>0&&blank.every(e=>Math.abs(e.getBoundingClientRect().height-e.previousElementSibling.getBoundingClientRect().height)<0.01&&e.getBoundingClientRect().height>0)";
        assert_eq!(js(&root, verify).as_bool(), Some(true));
        let second = root
            .first_element_child()
            .unwrap()
            .next_element_sibling()
            .unwrap();
        click(&second);
        assert_eq!(js(&root, verify).as_bool(), Some(true));
        let code = root.last_element_child().unwrap();
        assert_eq!(
            js(
                &code,
                "return value.querySelector('.output-full code').textContent==='one\\n\\nthree'"
            )
            .as_bool(),
            Some(true)
        );
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
            slots.pack = maestro_theme::load_brand_pack(pack, &support::brand::Pack).unwrap();
            for mode in [
                maestro_theme::BrandMode::Dark,
                maestro_theme::BrandMode::Light,
            ] {
                slots.mode = mode;
                let root = export_mount::mount(&mut slots, html.clone());
                let presentation = slots.pack.presentation(mode).unwrap();
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
