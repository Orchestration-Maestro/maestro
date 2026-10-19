//! Actual document witnesses for retained Markdown and SVG artifact branches.
#![cfg(all(test, target_arch = "wasm32"))]
#[cfg(test)]
pub mod highlighter_support;
#[cfg(test)]
pub mod markdown_svg_support;
#[cfg(test)]
mod panel_browser_support;
#[cfg(test)]
pub mod pointer_support;
#[path = "artifact_support/presentation.rs"]
#[cfg(test)]
pub mod presentation;
#[cfg(test)]
pub mod reading_browser_support;
#[cfg(test)]
pub mod session_header_support;
#[cfg(test)]
pub mod support;
#[cfg(test)]
mod tests {
    use crate::markdown_svg_support::{corpus, key};
    use crate::reading_browser_support::{View, check, event, js, quoted, run};
    use wasm_bindgen::prelude::*;
    /// The four measured artifact header controls, left to right.
    fn controls(view: &mut View) -> Vec<egui::Rect> {
        view.draw(true, vec![]);
        let mut controls = view.context.interactive_rects_last_pass();
        controls.sort_by(|a, b| a.left().total_cmp(&b.left()));
        assert_eq!(controls.len(), 4);
        controls
    }
    /// Activate an actual measured artifact header endpoint.
    fn select(view: &mut View, index: usize) {
        let controls = controls(view);
        view.draw(
            true,
            crate::pointer_support::click(controls[index].center()),
        );
    }
    /// Source wraps long words but never trims or reclassifies authored Markdown.
    #[wasm_bindgen]
    pub fn markdown_source_wraps_without_changing_text() {
        source_cases("markdown", true);
    }
    /// Exercise fixed highlighting and wrapping once per distinct source input.
    fn source_cases(kind: &str, wraps: bool) {
        for case in cases(kind) {
            let mut view = View::new(&case.filename, &case.content);
            select(&mut view, 1);
            for (filename, text) in [
                (&case.filename, &case.content),
                (&case.renamed_filename, &case.content),
                (&case.renamed_filename, &case.edited_content),
            ] {
                view.element.set_filename(filename.clone());
                view.element.set_content(text.clone());
                view.draw(false, vec![]);
                check(&format!(
                    "(()=>{{const c=document.querySelector('.maestro-text-artifact code');return c.textContent==={} && c.className==={}}})()",
                    quoted(text),
                    quoted(&case.language_class)
                ));
            }
        }
        let mut view = View::new(if wraps { "long.md" } else { "long.svg" }, &"x".repeat(400));
        select(&mut view, 1);
        view.size.x = 160.;
        view.draw(false, vec![]);
        check(if wraps {
            "(()=>{const r=document.querySelector('.maestro-text-artifact'),p=r.parentElement;return r.getBoundingClientRect().height>100 && p.scrollWidth<=p.clientWidth+1 && getComputedStyle(r).whiteSpace==='pre-wrap'})()"
        } else {
            "(()=>{const r=document.querySelector('.maestro-text-artifact'),p=r.parentElement;return p.scrollWidth>p.clientWidth && getComputedStyle(r).whiteSpace==='pre'})()"
        });
    }
    /// Source and image inputs are locale-independent; localized controls are covered natively.
    fn cases(kind: &str) -> impl Iterator<Item = crate::markdown_svg_support::Case> {
        corpus().content.into_iter().filter(move |c| c.kind == kind)
    }
    /// Await the actual native image outcome rather than guessing readiness.
    async fn image_outcome(expected: &str) {
        let result = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(js("document.querySelector('.maestro-svg-artifact img:not([hidden])').decode().then(()=> 'load',()=> 'error')"))).await.unwrap();
        assert_eq!(result.as_string().as_deref(), Some(expected));
    }
    /// Inspect the exact Blob bytes at the real image consumer.
    async fn blob_text(expected: &str) {
        let result = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(js("urlBlobs.get(document.querySelector('.maestro-svg-artifact img:not([hidden])').src).text()"))).await.unwrap();
        assert_eq!(result.as_string().as_deref(), Some(expected));
    }
    /// Preview is the shared selectable document owner, including its code controls.
    #[wasm_bindgen]
    pub fn markdown_preview_uses_the_shared_block() {
        for case in cases("markdown") {
            let mut view = View::new(&case.filename, &case.content);
            view.draw(false, vec![]);
            check(&format!(
                "document.querySelector('.maestro-markdown').getBoundingClientRect().left === {}",
                view.pack.presentation(view.mode).unwrap().spacing["md"]
            ));
            check(
                "document.querySelector('.maestro-markdown')!==null && document.querySelector('.maestro-text-artifact')===null && !globalThis.injected",
            );
            if case.content.starts_with("# Heading") {
                check(
                    "document.querySelector('.maestro-markdown h1').textContent==='Heading' && document.querySelector('.maestro-markdown strong').textContent==='bold'",
                );
            }
            if case.content.is_empty() {
                check("document.querySelector('.maestro-markdown').textContent===''");
            }
            if case.content.contains("<script>") {
                check(
                    "!document.querySelector('.maestro-markdown script') && document.querySelector('.maestro-markdown').textContent.includes('<script>')",
                );
            }
        }
        let mut view = View::new(
            "example.md",
            "# Sample\n\n```rust\nlet answer = 42;\n```\n\n$ x^2 $",
        );
        view.draw(false, vec![]);
        check(
            "document.querySelector('.maestro-markdown .maestro-code')!==null && document.querySelector('.maestro-markdown .katex')!==null",
        );
        assert!(!view.context.interactive_rects_last_pass().is_empty());
    }
    /// The full artifact body clips content while its padding scrolls away.
    #[wasm_bindgen]
    pub fn markdown_padding_scrolls_inside_the_full_body() {
        let panel = crate::panel_browser_support::Panel::new();
        panel.create("tall.md", &"word ".repeat(2000));
        let root = js("(()=>{const v=document.querySelector('.maestro-reading-viewport').getBoundingClientRect();return v.left===1 && v.top===41 && v.width===399 && v.height===259})()").as_bool().unwrap();
        check("document.querySelector('.maestro-markdown p').getBoundingClientRect().top===57");
        run("document.querySelector('.maestro-reading-viewport').scrollTop=64");
        panel.draw(vec![]);
        let symptom = js("(()=>{const v=document.querySelector('.maestro-reading-viewport'),r=v.getBoundingClientRect(),p=document.querySelector('.maestro-markdown p');return v.scrollTop===64 && p.getBoundingClientRect().top===-7 && document.elementFromPoint(r.left+17,42)===p && document.elementFromPoint(r.left+17,40)!==p})()").as_bool().unwrap();
        assert!(
            root && symptom,
            "full-body viewport={root}; padding scrolls under body clip={symptom}"
        );
    }
    /// SVG code stays literal XML after edits and retains horizontal scrolling.
    #[wasm_bindgen]
    pub fn svg_source_keeps_xml_and_horizontal_overflow() {
        source_cases("svg", false);
    }
    /// SVG documents load as inert, contained, pointer-interactive native images.
    #[wasm_bindgen]
    pub async fn svg_preview_is_an_inert_contained_image() {
        run(include_str!("markdown_svg_support/url_audit.js"));
        for case in cases("svg").filter(|c| c.outcome.as_deref() == Some("load")) {
            let mut view = View::new(&case.filename, &case.content);
            view.draw(false, vec![]);
            image_outcome("load").await;
            blob_text(&case.content).await;
            check(&format!(
                "document.querySelector('.maestro-svg-artifact img').alt==={}",
                quoted(&case.filename)
            ));
            check(
                "(()=>{const r=document.querySelector('.maestro-svg-artifact'),i=r.querySelector('img');const a=r.getBoundingClientRect(),b=i.getBoundingClientRect(),p=parseFloat(getComputedStyle(r).paddingTop);return Math.abs(b.width-(a.width-2*p))<=1 && Math.abs(b.height-(a.height-2*p))<=1 && getComputedStyle(i).objectFit==='contain' && getComputedStyle(i).pointerEvents!=='none' && !r.querySelector('svg,script') && !globalThis.injected && document.elementFromPoint(b.x+b.width/2,b.y+b.height/2)===i})()",
            );
        }
        check("urlBlobs.size===0");
    }
    /// Empty and malformed SVG preserve no-image and native error states separately.
    #[wasm_bindgen]
    pub async fn svg_empty_and_invalid_content_keep_native_states() {
        run(include_str!("markdown_svg_support/url_audit.js"));
        for case in cases("svg").filter(|c| c.outcome.as_deref() != Some("load")) {
            let mut view = View::new(&case.filename, &case.content);
            view.draw(false, vec![]);
            if let Some(expected) = case.outcome {
                image_outcome(&expected).await;
                blob_text(&case.content).await;
                check(
                    "!document.querySelector('.maestro-svg-artifact').textContent.includes('Image Error') && !document.querySelector('.maestro-svg-artifact img').src.startsWith('data:')",
                );
            } else {
                check(
                    "!document.querySelector('.maestro-svg-artifact img:not([hidden])') && urlBlobs.size===0",
                );
            }
            view.element.set_content(String::new());
            view.draw(false, vec![]);
            check(
                "!document.querySelector('.maestro-svg-artifact img:not([hidden])') && urlBlobs.size===0",
            );
            view.element.set_content(" ".into());
            view.draw(false, vec![]);
            image_outcome("error").await;
            blob_text(" ").await;
        }
        check("urlBlobs.size===0");
    }
    /// Resource identity follows edits and real attachment, never selection or hiding.
    #[wasm_bindgen]
    pub async fn svg_urls_follow_content_and_real_attachment() {
        run(include_str!("markdown_svg_support/url_audit.js"));
        let case = corpus().lifecycle;
        let mut view = View::new("a.svg", &case.contents[0]);
        view.element.connected_callback().unwrap();
        view.draw(false, vec![]);
        run("globalThis.initial=document.querySelector('.maestro-svg-artifact img').src");
        view.element.set_content(case.contents[0].clone());
        view.draw(false, vec![]);
        view.element.set_visibility(false, None).unwrap();
        check("urlBlobs.has(initial) && urlEvents.length===1");
        select(&mut view, 1);
        view.element.set_content(case.contents[1].clone());
        view.draw(false, vec![]);
        check("!urlBlobs.has(initial) && urlBlobs.size===1");
        view.element.set_content(case.contents[2].clone());
        view.draw(false, vec![]);
        check("urlBlobs.size===0");
        view.element.set_content(case.contents[3].clone());
        view.draw(false, vec![]);
        check("urlBlobs.size===1");
        view.element.disconnected_callback().unwrap();
        check("urlBlobs.size===0");
        view.element.connected_callback().unwrap();
        check("urlBlobs.size===1");
        view.element.disconnected_callback().unwrap();
        check("urlBlobs.size===0");
        let actual = js_sys::JSON::stringify(&js("urlEvents"))
            .unwrap()
            .as_string()
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&actual).unwrap(),
            serde_json::to_value(&case.events).unwrap()
        );
        drop(view);
        let mut first = View::new("one.svg", &case.independent[0]);
        first.draw(false, vec![]);
        let mut second = View::new("two.svg", &case.independent[1]);
        second.draw(false, vec![]);
        check("urlBlobs.size===2");
        drop(first);
        check("urlBlobs.size===1");
        blob_text(&case.independent[1]).await;
        drop(second);
        check("urlBlobs.size===0 && !document.querySelector('.maestro-svg-artifact')");
        deferred_creation_failure();
    }
    /// The next fallible lifecycle operation reports an infallible setter's error.
    fn deferred_creation_failure() {
        run(
            "globalThis.savedCreate=URL.createObjectURL;URL.createObjectURL=()=>{throw new Error('allocation rejected')}",
        );
        let mut failed = View::new("failure.svg", "<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
        run("URL.createObjectURL=savedCreate");
        let root = matches!(
            failed.element.connected_callback(),
            Err(maestro_web::tools::artifacts::ArtifactUiError::Host(_))
        );
        let symptom = js("urlBlobs.size===0").as_bool().unwrap();
        assert!(
            root && symptom,
            "deferred callback reports error={root}; no resource allocated after failure={symptom}"
        );
        failed.element.connected_callback().unwrap();
        check("urlBlobs.size===1");
        drop(failed);
        check("urlBlobs.size===0");
    }
    /// The first switched frame hides the retained inactive native branch.
    #[wasm_bindgen]
    pub fn artifact_switch_hides_retained_inactive_roots() {
        for name in ["a.md", "a.svg"] {
            let mut view = View::new(name, "<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
            view.draw(false, vec![]);
            run(
                "globalThis.preview=document.querySelector('.maestro-markdown,.maestro-svg-artifact').parentElement",
            );
            select(&mut view, 1);
            view.draw(false, vec![]);
            check(
                "getComputedStyle(preview).visibility==='hidden' && document.querySelector('.maestro-text-artifact').getBoundingClientRect().height>0",
            );
            run("globalThis.source=document.querySelector('.maestro-text-artifact').parentElement");
            view.element
                .set_visibility(false, Some(egui::Id::new("modal")))
                .unwrap();
            view.element.set_content("updated".into());
            select(&mut view, 0);
            view.draw(false, vec![]);
            run(
                "globalThis.preview=document.querySelector('.maestro-markdown,.maestro-svg-artifact').parentElement",
            );
            check(
                "getComputedStyle(source).visibility==='hidden' && getComputedStyle(preview).visibility==='hidden'",
            );
            view.element.set_visibility(true, None).unwrap();
            view.draw(false, vec![]);
            check(
                "getComputedStyle(preview).visibility==='visible' && getComputedStyle(source).visibility==='hidden'",
            );
        }
        panel_attachment();
    }
    /// Exercise real panel lifecycle forwarding, including an already-connected insert.
    fn panel_attachment() {
        run(include_str!("markdown_svg_support/url_audit.js"));
        let panel = crate::panel_browser_support::Panel::new();
        panel.create("a.svg", "<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
        check("urlBlobs.size===1");
        panel.panel.set_collapsed(true);
        panel.draw(vec![]);
        check("urlBlobs.size===1");
        panel.panel.set_collapsed(false);
        panel.panel.set_overlay(true);
        panel.draw(vec![]);
        check("urlBlobs.size===1");
        panel.panel.disconnected_callback().unwrap();
        check("urlBlobs.size===0");
        panel.panel.connected_callback(&panel.caller).unwrap();
        panel.draw(vec![]);
        check("urlBlobs.size===1");
        panel.create("b.txt", "control");
        panel.caller.show_artifact("b.txt");
        panel.draw(vec![]);
        check("urlBlobs.size===1");
        panel.panel.remove_artifact_element("a.svg");
        check("urlBlobs.size===0");
        let host = std::rc::Rc::downgrade(&panel.host);
        drop(panel);
        assert!(host.upgrade().is_none());
    }
    /// Keyboard and clipped pointer input reach the same retained header actions.
    #[wasm_bindgen]
    pub async fn artifact_controls_keep_keyboard_and_clipped_pointer_admission() {
        run(
            "globalThis.copied=[];navigator.clipboard.writeText=text=>{copied.push(text);return Promise.resolve()};globalThis.saved=[];HTMLAnchorElement.prototype.click=function(){saved.push(this.download)}",
        );
        let panel = crate::panel_browser_support::Panel::new();
        panel.create("a.md", "literal & 雪");
        for _ in 0..3 {
            panel.draw(key(egui::Key::Tab));
        }
        panel.draw(key(egui::Key::Space));
        check("document.querySelector('.maestro-text-artifact code').textContent==='literal & 雪'");
        panel.draw(key(egui::Key::Tab));
        panel.draw(key(egui::Key::Space));
        event(js("Promise.resolve()")).await;
        check("copied.length===1 && copied[0]==='literal & 雪'");
        panel.draw(key(egui::Key::Tab));
        panel.draw(key(egui::Key::Space));
        check("saved.length===1 && saved[0]==='a.md'");
        drop(panel);
        let mut view = View::new("clipped.md", "control");
        let controls = controls(&mut view);
        view.size.x = controls[1].left() + 4.;
        view.draw(true, vec![]);
        view.draw(
            true,
            crate::pointer_support::click(controls[1].right_center()),
        );
        view.draw(false, vec![]);
        check(
            "document.querySelector('.maestro-markdown')!==null && !document.querySelector('.maestro-text-artifact')",
        );
    }
}
