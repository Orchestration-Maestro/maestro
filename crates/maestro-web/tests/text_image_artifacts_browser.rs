//! Actual-document source and image artifacts through the retained panel boundary.
#![cfg(all(test, target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use crate::reading_browser_support::{View, check, event, js, quoted, run};
    use crate::text_image_support::browser_records::corpus;
    use wasm_bindgen::prelude::*;

    /// Construct the explicit text view, independent of the application's file classifier.
    fn text_view(filename: &str, content: &str) -> View {
        let mut view = View::new("a.txt", content);
        view.element.set_filename(filename.into());
        view
    }
    /// Construct the explicit image view, independent of the application's file classifier.
    fn image_view(filename: &str, content: &str) -> View {
        let mut view = View::new("a.png", content);
        view.element.set_filename(filename.into());
        view
    }
    /// Native syntax spans share the selected pack's actual stylesheet and typeface.
    #[wasm_bindgen]
    pub fn text_code_body_uses_shared_highlighting() {
        for case in corpus().text_render.into_iter().filter(|case| {
            ["app.js", "APP.TS", "script.ps1", "empty.py"].contains(&case.input.filename.as_str())
        }) {
            let mut view = text_view(&case.input.filename, &case.input.content);
            view.draw(false, vec![]);
            check(&format!(
                "document.querySelector('.maestro-text-artifact code').className === {} && document.querySelector('.maestro-text-artifact code').textContent === {}",
                quoted(case.output.class.as_deref().unwrap()),
                quoted(&case.output.text)
            ));
            if ["app.js", "APP.TS"].contains(&case.input.filename.as_str()) {
                check(
                    "(()=>{const c=document.querySelector('.maestro-text-artifact code');const s=c.querySelector('span[class^=hljs-]');if(!s)return false;const actual=getComputedStyle(s).color;return actual!==getComputedStyle(c).color && actual!=='';})()",
                );
                let colors = view.pack.presentation(view.mode).unwrap().css_properties();
                let roles: serde_json::Value = serde_json::from_str(include_str!(
                    "../../maestro-theme/assets/theme/brand-roles.json"
                ))
                .unwrap();
                let pairs: Vec<(String, String)> =
                    serde_json::from_value(roles["colors"].clone()).unwrap();
                let pairs: std::collections::BTreeMap<_, _> = pairs.into_iter().collect();
                let class =
                    js("document.querySelector('.maestro-text-artifact code span').className")
                        .as_string()
                        .unwrap();
                let name = class.strip_prefix("hljs-").unwrap();
                let key = format!("syntax{}{}", name[..1].to_uppercase(), &name[1..]);
                let role = &pairs[&key];
                let expected = &colors[&format!("--maestro-color-{role}")];
                check(&format!(
                    "(()=>{{const s=document.querySelector('.maestro-text-artifact code span');const e=document.createElement('span');e.style.color={};s.parentElement.append(e);const same=getComputedStyle(e).color===getComputedStyle(s).color;e.remove();return same}})()",
                    quoted(expected)
                ));
            } else {
                check("document.querySelector('.maestro-text-artifact code span')===null");
            }
            check(
                "(()=>{const r=document.querySelector('.maestro-text-artifact');return getComputedStyle(r).fontFamily===getComputedStyle(r).getPropertyValue('--maestro-type-mono-font').trim()})()",
            );
        }
    }
    /// Unsupported source labels retain a visible literal body rather than going blank.
    #[wasm_bindgen]
    pub fn text_unknown_language_renders_plain_text() {
        for case in corpus().text_render.into_iter().filter(|case| {
            ["style.sass", "run.bat", "App.vue", "App.svelte"]
                .contains(&case.input.filename.as_str())
        }) {
            let mut view = text_view(&case.input.filename, &case.input.content);
            view.draw(false, vec![]);
            check(&format!(
                "(()=>{{const r=document.querySelector('.maestro-text-artifact');const c=r.querySelector('code');return r.getBoundingClientRect().height>0 && c.className==={} && c.textContent==={}}})()",
                quoted(case.output.class.as_deref().unwrap()),
                quoted(&case.output.text)
            ));
        }
    }
    /// Markup remains literal; long lines scroll within the shared reading viewport.
    #[wasm_bindgen]
    pub fn text_plain_body_keeps_literal_text() {
        for case in corpus()
            .text_render
            .into_iter()
            .filter(|case| ["notes.txt", "header.h"].contains(&case.input.filename.as_str()))
        {
            let mut view = text_view(&case.input.filename, &case.input.content);
            view.draw(false, vec![]);
            assert!(case.output.class.is_none());
            check(&format!(
                "(()=>{{const r=document.querySelector('.maestro-text-artifact');const c=r.querySelector('code');const s=getComputedStyle(r);return !c.hasAttribute('class') || c.className==='' ? c.textContent==={} && !r.querySelector('script') && s.whiteSpace==='pre' && s.paddingTop===s.getPropertyValue('--maestro-spacing-md').trim() : false}})()",
                quoted(&case.output.text)
            ));
            view.element.set_content("x".repeat(400));
            view.draw(false, vec![]);
            check(
                "(()=>{const r=document.querySelector('.maestro-text-artifact').parentElement;return r.scrollWidth>r.clientWidth})()",
            );
        }
    }
    /// Edits preserve root and scroll, renames preserve selection, and dropping disconnects.
    #[wasm_bindgen]
    pub fn text_body_root_survives_edits() {
        let mut view = text_view(
            "notes.txt",
            &"line......................................................\n".repeat(100),
        );
        view.draw(false, vec![]);
        run(
            "globalThis.savedRoot=document.querySelector('.maestro-text-artifact');globalThis.viewport=savedRoot.parentElement;viewport.scrollTop=50;globalThis.savedScroll=viewport.scrollTop",
        );
        check("savedScroll>0");
        view.element.set_content(
            "changed......................................................\n".repeat(100),
        );
        view.draw(false, vec![]);
        check(
            "savedRoot===document.querySelector('.maestro-text-artifact') && viewport.scrollTop===savedScroll",
        );
        let mut selections = Vec::new();
        for (before, after) in [("notes.txt", "renamed.txt"), ("a.js", "b.js")] {
            view.element.set_filename(before.into());
            view.element
                .set_content("const message = \"hello & goodbye\";".into());
            view.draw(false, vec![]);
            run(
                "const code=savedRoot.querySelector('code');const range=document.createRange();range.selectNodeContents(code);getSelection().removeAllRanges();getSelection().addRange(range)",
            );
            check("getSelection().toString()==='const message = \"hello & goodbye\";'");
            view.element.set_filename(after.into());
            view.draw(false, vec![]);
            selections.push(js("getSelection().toString()").as_string().unwrap());
        }
        assert_eq!(selections, ["const message = \"hello & goodbye\";"; 2]);
        view.element.set_filename("a.js".into());
        view.draw(false, vec![]);
        check(
            "savedRoot===document.querySelector('.maestro-text-artifact') && savedRoot.querySelector('code').className==='hljs language-javascript'",
        );
        view.element.set_filename("a.txt".into());
        view.draw(false, vec![]);
        check(
            "savedRoot.querySelector('code').className==='' && !savedRoot.querySelector('code span')",
        );
        drop(view);
        check("!savedRoot.isConnected");
    }
    /// Raw base64 is framed once; authored data URLs and native image identities stay literal.
    #[wasm_bindgen]
    pub fn image_source_wraps_raw_base64_and_keeps_data_urls() {
        for case in corpus().image_url {
            let mut view = image_view(&case.input.filename, &case.input.content);
            view.draw(false, vec![]);
            check(&format!(
                "(()=>{{const i=document.querySelector('.maestro-image-artifact img');globalThis.savedImage=i;return i.getAttribute('src')==={} && i.alt==={}}})()",
                quoted(&case.output),
                quoted(&case.input.filename)
            ));
            view.element.set_content(case.input.content);
            view.draw(false, vec![]);
            check("savedImage===document.querySelector('.maestro-image-artifact img')");
            drop(view);
            check("!savedImage.isConnected");
        }
    }
    /// Native error delivery switches once to the pack fallback and edits retry real images.
    #[wasm_bindgen]
    pub async fn image_load_failure_shows_branded_fallback() {
        for case in corpus().image_render {
            let mut view = image_view(&case.input.filename, &case.input.content);
            view.draw(false, vec![]);
            check(&format!(
                "(()=>{{const i=document.querySelector('.maestro-image-artifact img');return i.getAttribute('src')==={} && i.alt==={}}})()",
                quoted(&case.output.src),
                quoted(&case.output.alt)
            ));
            event(js("new Promise(resolve=>document.querySelector('.maestro-image-artifact img').addEventListener('error',resolve,{once:true}))")).await;
            let fallback = case.output.fallback.replace(
                "{muted}",
                view.pack.presentation(view.mode).unwrap().colors["muted"].trim_start_matches('#'),
            );
            check(&format!(
                "document.querySelector('.maestro-image-artifact img').getAttribute('src')==={}",
                quoted(&fallback)
            ));
            event(js(
                "document.querySelector('.maestro-image-artifact img').decode()",
            ))
            .await;
            run(
                "globalThis.sourceWrites=0;globalThis.sourceObserver=new MutationObserver(records=>sourceWrites+=records.filter(r=>r.attributeName==='src').length);sourceObserver.observe(document.querySelector('.maestro-image-artifact img'),{attributes:true})",
            );
            for _ in 0..2 {
                view.draw(false, vec![]);
            }
            event(js("Promise.resolve()")).await;
            check("sourceWrites===0");
            run("sourceObserver.disconnect()");
            check(&format!(
                "document.querySelector('.maestro-image-artifact img').getAttribute('src')==={}",
                quoted(&fallback)
            ));
            view.mode = maestro_theme::BrandMode::Light;
            view.draw(false, vec![]);
            let fallback = case.output.fallback.replace(
                "{muted}",
                view.pack.presentation(view.mode).unwrap().colors["muted"].trim_start_matches('#'),
            );
            check(&format!(
                "document.querySelector('.maestro-image-artifact img').getAttribute('src')==={}",
                quoted(&fallback)
            ));
            let loaded = js(
                "new Promise(resolve=>document.querySelector('.maestro-image-artifact img').addEventListener('load',resolve,{once:true}))",
            );
            let source = png(1, 1);
            view.element.set_content(source.clone());
            view.draw(false, vec![]);
            event(loaded).await;
            check(&format!(
                "(()=>{{const r=document.querySelector('.maestro-image-artifact');return r.children.length===2 && r.querySelector('img').getAttribute('src')==={}}})()",
                quoted(&source)
            ));
        }
    }
    /// Build a deterministic real PNG with the browser's own codec.
    fn png(width: u32, height: u32) -> String {
        js(&format!("(()=>{{const c=document.createElement('canvas');c.width={width};c.height={height};const x=c.getContext('2d');x.fillRect(0,0,c.width,c.height);return c.toDataURL('image/png')}})()")).as_string().unwrap()
    }
    /// Images retain native hit testing, containment, aspect ratio and selected-pack decoration.
    #[wasm_bindgen]
    pub async fn image_body_centers_the_contained_image() {
        for width in [1, 2000] {
            let height = if width == 1 { 1 } else { 100 };
            let mut view = image_view("a.png", &png(width, height));
            view.draw(false, vec![]);
            event(js(
                "document.querySelector('.maestro-image-artifact img').decode()",
            ))
            .await;
            check(
                "(()=>{const r=document.querySelector('.maestro-image-artifact'),i=r.querySelector('img');const a=r.getBoundingClientRect(),b=i.getBoundingClientRect(),s=getComputedStyle(r),p=parseFloat(s.paddingTop);return b.left>=a.left+p-1 && b.right<=a.right-p+1 && b.top>=a.top+p-1 && b.bottom<=a.bottom-p+1 && Math.abs((b.left+b.right-a.left-a.right)/2)<=1 && Math.abs((b.top+b.bottom-a.top-a.bottom)/2)<=1 && !i.hasAttribute('title') && getComputedStyle(i).borderRadius==='0px' && document.elementFromPoint((b.left+b.right)/2,(b.top+b.bottom)/2)===i})()",
            );
            check(&format!(
                "(()=>{{const i=document.querySelector('.maestro-image-artifact img'),r=i.getBoundingClientRect();return Math.abs(r.width/r.height-{})<0.01 && ({}===1 || r.width<{})}})()",
                f64::from(width) / f64::from(height),
                width,
                width
            ));
            let background = view.pack.presentation(view.mode).unwrap().colors["background"];
            check(&format!(
                "(()=>{{const r=document.querySelector('.maestro-image-artifact');const e=document.createElement('span');e.style.background={};r.append(e);const same=getComputedStyle(r).backgroundColor===getComputedStyle(e).backgroundColor;e.remove();return same}})()",
                quoted(background)
            ));
        }
    }
}
