//! Public native disclosure interactions and mounted lifetime.
use super::js;
use crate::session_header_browser::Header;
use wasm_bindgen::prelude::*;

fn header() -> Header {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/session_header_disclosures.json")).unwrap();
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "expansions")
        .unwrap();
    let mut header = Header::new();
    header.document = crate::session_header_support::document(&serde_json::json!("disclosures"));
    header.document.data.insert(
        "systemPrompt".into(),
        row["input"]["data"]["systemPrompt"].clone(),
    );
    header
        .document
        .data
        .insert("tools".into(), row["input"]["data"]["tools"].clone());
    header.draw(egui::vec2(600., 600.));
    header
}
fn root() -> web_sys::Element {
    crate::session_header_browser::panel()
        .parent_element()
        .unwrap()
}
fn expanded(root: &web_sys::Element) -> Vec<bool> {
    serde_json::from_str(&js(root, "return JSON.stringify([...value.querySelectorAll('.expandable,.tool-item:has(.tool-params-hint)')].map(e=>e.classList.contains('expanded')||e.classList.contains('params-expanded')))").as_string().unwrap()).unwrap()
}
fn click(root: &web_sys::Element, index: usize) {
    js(
        root,
        &format!(
            "value.querySelectorAll('.expandable,.tool-item:has(.tool-params-hint)')[{index}].dispatchEvent(new Event('click',{{bubbles:true}}))"
        ),
    );
}
fn clear_selection(root: &web_sys::Element) {
    js(
        root,
        "value.ownerDocument.defaultView.getSelection().removeAllRanges()",
    );
}
/// Independent native expansion and live selection suppression.
/// # Panics
/// Panics if the recorded state sequence differs.
#[wasm_bindgen]
pub fn header_expansion_is_independent_and_selection_safe() {
    let _header = header();
    let root = root();
    clear_selection(&root);
    assert_eq!(expanded(&root), [false, false, false]);
    for (index, expected) in [
        [true, false, false],
        [true, true, false],
        [true, true, true],
    ]
    .iter()
    .enumerate()
    {
        click(&root, index);
        assert_eq!(expanded(&root), *expected);
    }
    js(
        &root,
        "const s=window.getSelection(),r=document.createRange();r.selectNodeContents(value.querySelector('.system-prompt-header'));s.addRange(r)",
    );
    for i in 0..3 {
        click(&root, i);
    }
    assert_eq!(expanded(&root), [true, true, true]);
    clear_selection(&root);
    click(&root, 0);
    click(&root, 1);
    assert_eq!(expanded(&root), [false, false, true]);
}

/// Native panel hierarchy, whitespace, colors and independent decoration.
/// # Panics
/// Panics if native computed styles differ from the header contract.
#[wasm_bindgen]
pub async fn header_disclosure_styles_paint_native_hierarchy() {
    let mut _header = header();
    let following = _header.draw(egui::vec2(600., 2000.));
    let root = root();
    let spacing = _header
        .pack
        .presentation(maestro_theme::BrandMode::Dark)
        .unwrap()
        .spacing["lg"];
    let tools = root.query_selector(".tools-list").unwrap().unwrap();
    assert!(
        (f64::from(following) - tools.get_bounding_client_rect().bottom() - spacing).abs() < 0.1
    );
    assert!(root.class_name().contains("maestro-session-viewer-header"));
    assert!(js(&root,"const p=value.querySelector('.system-prompt'),t=value.querySelector('.tools-list');return p.parentElement===value && t.parentElement===value && p.previousElementSibling.classList.contains('maestro-session-header') && t.previousElementSibling===p").as_bool().unwrap());
    assert!(js(&root,"const s=e=>getComputedStyle(value.querySelector(e));return s('.system-prompt-full').display==='none' && s('.system-prompt-preview').whiteSpace==='pre-wrap' && s('.tool-params-content').display==='none' && s('.tool-item-desc').whiteSpace==='normal' && s('.tool-params-content').borderLeftStyle==='solid' && s('.system-prompt-header').fontWeight==='700' && s('.tools-header').color===s('.system-prompt-header').color").as_bool().unwrap());
    assert_eq!(
        js(
            &root,
            "return getComputedStyle(value.querySelector('.tool-params-hint'),'::after').content"
        )
        .as_string()
        .unwrap(),
        "\"[click to show parameters]\""
    );
    clear_selection(&root);
    click(&root, 0);
    click(&root, 1);
    assert!(js(&root,"return getComputedStyle(value.querySelector('.system-prompt-full')).display==='block' && getComputedStyle(value.querySelector('.system-prompt-preview')).display==='none' && getComputedStyle(value.querySelector('.tool-params-content')).display==='block'").as_bool().unwrap());
    assert_eq!(
        js(
            &root,
            "return getComputedStyle(value.querySelector('.tool-params-hint'),'::after').content"
        )
        .as_string()
        .unwrap(),
        "\"[hide parameters]\""
    );
    drop(_header);
    style_corpus().await;
}

/// Clicks on container whitespace, headings and descriptions bubble to their owner.
/// # Panics
/// Panics if only a text fragment, rather than its complete container, toggles.
#[wasm_bindgen]
pub fn header_complete_containers_toggle_without_text_hits() {
    let _header = header();
    let root = root();
    clear_selection(&root);
    for (i, selector) in [
        ".system-prompt-header",
        ".system-prompt-expand-hint",
        ".system-prompt",
        ".system-prompt-preview",
    ]
    .iter()
    .enumerate()
    {
        js(
            &root,
            &format!(
                "value.querySelector({selector:?}).dispatchEvent(new Event('click',{{bubbles:true}}))"
            ),
        );
        assert_eq!(expanded(&root), [i % 2 == 0, false, false]);
    }
    assert_eq!(expanded(&root), [false, false, false]);
    for (i, selector) in [
        ".tool-item-name",
        ".tool-item-desc",
        ".tool-params-hint",
        ".tool-item",
    ]
    .iter()
    .enumerate()
    {
        js(
            &root,
            &format!(
                "value.querySelector({selector:?}).dispatchEvent(new Event('click',{{bubbles:true}}))"
            ),
        );
        assert_eq!(expanded(&root), [false, i % 2 == 0, false]);
    }
    assert_eq!(expanded(&root), [false, false, false]);
    js(
        &root,
        "value.querySelector('.system-prompt').dispatchEvent(new MouseEvent('click',{bubbles:true,detail:2}))",
    );
    assert_eq!(expanded(&root), [true, false, false]);
    click(&root, 0);
    click(&root, 1);
    assert_eq!(expanded(&root), [false, true, false]);
}

/// Expansion and selection endpoints survive unchanged redraw and native readback.
/// # Panics
/// Panics if repaint remounts the owner or uses a collapsed height snapshot.
#[wasm_bindgen]
pub fn header_unchanged_redraw_retains_expansion_and_selection() {
    let mut header = header();
    let root = root();
    clear_selection(&root);
    root.set_attribute("style", "min-width:900px").unwrap();
    header.draw(egui::vec2(300., 40.));
    let collapsed = root.get_bounding_client_rect().height();
    click(&root, 0);
    click(&root, 1);
    let expanded_height = root.get_bounding_client_rect().height();
    assert!(expanded_height > collapsed);
    js(
        &root,
        "const r=document.createRange();r.selectNodeContents(value.querySelector('.system-prompt-full'));window.getSelection().addRange(r);const w=value.parentElement;w.scrollTop=17.6;w.scrollLeft=25.6",
    );
    let saved = js(
        &root,
        "const w=value.parentElement,s=window.getSelection();return {root:value,anchor:s.anchorNode,focus:s.focusNode,anchorOffset:s.anchorOffset,focusOffset:s.focusOffset,left:w.scrollLeft,top:w.scrollTop,text:s.toString()}",
    );
    assert!(
        js(&saved, "return value.left%1!==0 && value.top%1!==0")
            .as_bool()
            .unwrap()
    );
    header.draw(egui::vec2(300., 40.));
    assert!(js(&saved,"const s=window.getSelection(),w=value.root.parentElement;return document.querySelector('.maestro-session-viewer-header')===value.root && s.anchorNode===value.anchor && s.focusNode===value.focus && s.anchorOffset===value.anchorOffset && s.focusOffset===value.focusOffset && s.toString()===value.text && w.scrollLeft===value.left && w.scrollTop===value.top").as_bool().unwrap());
    assert_eq!(expanded(&root), [true, true, false]);
    assert!((root.get_bounding_client_rect().height() - expanded_height).abs() < 0.1);
    clear_selection(&root);
}

/// Obtain the real public factory after the document, state and caller have dropped.
fn owned_factory() -> std::rc::Rc<maestro_web::browser::attachment_content::ReadingContent> {
    factory_for(
        &serde_json::json!({"entries":[],"systemPrompt":"p\n".repeat(12),"tools":[{"name":"owned <tool>","description":"d","parameters":{"properties":{"x":{"type":"string"}}}}]}),
    )
}

/// Prepare a factory while dropping every borrowed source operand before returning it.
fn factory_for(
    data: &serde_json::Value,
) -> std::rc::Rc<maestro_web::browser::attachment_content::ReadingContent> {
    let host = std::rc::Rc::new(crate::session_header_support::Host::default());
    host.height.set(120.);
    let mut state = maestro_web::export_html::template::HeaderState::new(host.clone());
    let pack =
        maestro_theme::load_brand_pack("brand.json", &crate::session_header_browser::Pack).unwrap();
    use base64::Engine as _;
    let doc = crate::session_header_browser::load_document(
        &base64::engine::general_purpose::STANDARD.encode(data.to_string()),
        "",
        None,
    )
    .unwrap();
    crate::session_header_support::draw(
        &mut state,
        &doc,
        &pack.presentation(maestro_theme::BrandMode::Dark).unwrap(),
        egui::Layout::top_down(egui::Align::Min),
        egui::vec2(300., 40.),
    )
    .0
    .unwrap();
    let factory = std::rc::Rc::clone(&host.admissions.borrow()[0]);
    factory
}

/// Root removal revokes callbacks even with a surviving SDK factory alias.
/// # Panics
/// Panics if callbacks outlive their mounted owner or partial setup leaks live state.
#[wasm_bindgen]
pub fn header_root_release_detaches_callbacks_with_factory_alias() {
    let factory = owned_factory();
    let mut slots = super::Slots::new();
    slots.content = std::rc::Rc::clone(&factory);
    slots.publish(egui::vec2(600., 600.), &[]);
    let root = root();
    clear_selection(&root);
    click(&root, 0);
    assert_eq!(expanded(&root), [true, false]);
    assert!(root.text_content().unwrap().contains("owned <tool>"));
    assert!(root.query_selector("tool").unwrap().is_none());
    drop(slots);
    assert!(!root.is_connected());
    click(&root, 0);
    assert_eq!(expanded(&root), [true, false]);
    registration_failures(&factory);
    drop(factory);
}

/// Fault native installation/removal below the shared subscription abstraction.
fn registration_failures(factory: &maestro_web::browser::attachment_content::ReadingContent) {
    let document = web_sys::window().unwrap().document().unwrap();
    for failed_removal in [false, true] {
        let hook = js(
            &document,
            &format!(
                "const failedRemoval={failed_removal}; {}",
                include_str!("disclosure_registration.js")
            ),
        );
        let result = (factory.mount)(&document);
        let evidence = js(&hook, "return value.stop()");
        assert!(result.is_err());
        assert!(js(&evidence,"return value.added.length===1 && value.removed.length===1 && value.added[0][0]===value.removed[0][0] && value.added[0][1]===value.removed[0][1]").as_bool().unwrap());
        js(
            &evidence,
            "const [target,callback]=value.added[0],before=target.className;target.dispatchEvent(new Event('click'));if(target.className!==before)throw Error('released callback changed state')",
        );
    }
}

/// Draw a changed candidate through the same public caller while retaining errors.
fn redraw(
    header: &mut Header,
) -> std::io::Result<Vec<maestro_web::export_html::template::HeaderAction>> {
    let mut result = None;
    let presentation = header
        .pack
        .presentation(maestro_theme::BrandMode::Dark)
        .unwrap();
    let output = header.context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(600., 600.),
            )),
            ..Default::default()
        },
        |ui| {
            result = Some(maestro_web::export_html::template::render_header(
                ui,
                &header.document,
                &mut header.state,
                maestro_web::export_html::template::HeaderView {
                    presentation: &presentation,
                    localize_date: &crate::session_header_browser::localize_header_date,
                },
            ));
        },
    );
    drop(output);
    result.unwrap()
}

/// Candidate errors preserve old expansion and successful replacement starts fresh.
/// # Panics
/// Panics if a factory carries expansion across mounts or failure drops the old owner.
#[wasm_bindgen]
pub fn header_replacement_resets_disclosures_without_losing_old_on_failure() {
    let mut header = header();
    let root = root();
    clear_selection(&root);
    click(&root, 0);
    click(&root, 1);
    header.document.data.insert(
        "systemPrompt".into(),
        serde_json::json!("changed\n".repeat(12)),
    );
    for kind in ["mount", "read", "publish"] {
        js(
            &root,
            "const s=window.getSelection(),r=document.createRange();r.selectNodeContents(value.querySelector('.system-prompt-full'));s.removeAllRanges();s.addRange(r)",
        );
        let selection = js(
            &root,
            "const s=window.getSelection();return {anchor:s.anchorNode,focus:s.focusNode,text:s.toString()}",
        );
        let hook = js(
            &root,
            &format!(
                "const kind={kind:?};{}",
                include_str!("disclosure_replacement.js")
            ),
        );
        let result = redraw(&mut header);
        js(&hook, "value()");
        assert!(result.is_err(), "{kind}");
        assert!(root.is_connected());
        assert!(js(&selection,"const s=window.getSelection();return s.anchorNode===value.anchor && s.focusNode===value.focus && s.toString()===value.text").as_bool().unwrap());
        clear_selection(&root);
        assert_eq!(expanded(&root), [true, true, false]);
        click(&root, 1);
        assert_eq!(expanded(&root), [true, false, false]);
        click(&root, 1);
    }
    redraw(&mut header).unwrap();
    assert!(!root.is_connected());
    let replacement = self::root();
    assert_eq!(expanded(&replacement), [false, false, false]);
    click(&replacement, 0);
    assert_eq!(expanded(&replacement), [true, false, false]);
    click(&root, 0);
    assert_eq!(expanded(&root), [true, true, false]);
}

/// Consume every successful recorded row through an owned factory and native readback.
async fn style_corpus() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../fixtures/session_header_disclosures.json")).unwrap();
    for (pack, mode) in [
        ("brand.json", maestro_theme::BrandMode::Dark),
        ("brand.json", maestro_theme::BrandMode::Light),
        ("signal.json", maestro_theme::BrandMode::Dark),
    ] {
        let mut slots = super::Slots::new();
        slots.pack =
            maestro_theme::load_brand_pack(pack, &crate::session_header_browser::Pack).unwrap();
        slots.mode = mode;
        let mut count = 0;
        for row in cases.iter().filter(|r| r["branch"] == "success") {
            slots.content = factory_for(&row["input"]["data"]);
            let layout = slots.publish(egui::vec2(350., 600.), &[]);
            let root = root();
            let output: serde_json::Value = serde_json::from_str(
                &js(&root, include_str!("disclosure_projection.js"))
                    .as_string()
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(output, row["output"], "{}", row["name"]);
            assert!(layout.height > 0.);
            let bounds = root.get_bounding_client_rect();
            assert!((f64::from(layout.height) - bounds.height()).abs() < 0.1);
            assert!(js(&root,"const panels=[...value.children].filter(e=>e.matches('.maestro-session-header,.system-prompt,.tools-list'));return panels.every((p,i)=>p.parentElement===value && (!i || p.getBoundingClientRect().top>=panels[i-1].getBoundingClientRect().bottom))").as_bool().unwrap());
            clear_selection(&root);
            js(
                &root,
                "for(const e of value.querySelectorAll('.expandable,.tool-item'))e.dispatchEvent(new Event('click'));for(const e of value.querySelectorAll('.tool-params-content')){const s=getComputedStyle(e);if(s.display!=='block'||parseFloat(s.marginLeft)<=0||parseFloat(s.paddingLeft)<=0||parseFloat(s.borderLeftWidth)<=0)throw Error('parameter geometry');}for(const e of value.querySelectorAll('.tool-item:not(:has(.tool-params-hint))'))if(e.classList.contains('params-expanded'))throw Error('plain tool toggled');for(const e of value.querySelectorAll('.tool-param-desc'))if(e.textContent==='  '&&e.getBoundingClientRect().height!==0)throw Error('whitespace height')",
            );
            rendered_whitespace(&root, &row["name"]);
            let presentation = slots.pack.presentation(mode).unwrap();
            let roles = serde_json::json!({"text":presentation.colors["text"],"muted":presentation.colors["muted"],"accent":presentation.colors["accent"],"warning":presentation.colors["warning"],"border":presentation.colors["border"]});
            assert!(js(&root,&format!("const roles={roles};const color=k=>{{const e=document.createElement('span');e.style.color=roles[k];return e.style.color}};return [['.system-prompt-header','accent'],['.tools-header','accent'],['.tool-item-name','text'],['.tool-param-type','muted'],['.tool-param-required','warning']].every(([s,k])=>[...value.querySelectorAll(s)].every(e=>getComputedStyle(e).color===color(k))) && [...value.querySelectorAll('.tool-param-type,.tool-params-hint')].every(e=>getComputedStyle(e).fontStyle==='italic') && [...value.querySelectorAll('.tool-params-content')].every(e=>getComputedStyle(e).borderLeftColor===color('border'))")).as_bool().unwrap());
            if ["canonical-properties", "expansions"]
                .iter()
                .any(|name| row["name"] == *name)
            {
                let request = js(
                    &root,
                    &format!(
                        "if(typeof window.captureHeader==='function')return window.captureHeader({:?})",
                        format!(
                            "{}-{}-{}",
                            pack,
                            if matches!(mode, maestro_theme::BrandMode::Dark) {
                                "dark"
                            } else {
                                "light"
                            },
                            row["name"].as_str().unwrap()
                        )
                    ),
                );
                if request.is_instance_of::<js_sys::Promise>() {
                    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(request))
                        .await
                        .unwrap();
                }
            }
            count += 1;
        }
        assert_eq!(count, 80);
    }
}

/// Native line boxes of expanded rows whose whitespace rendering the text projection cannot see.
fn rendered_whitespace(root: &web_sys::Element, name: &serde_json::Value) {
    let checks = match name.as_str() {
        Some("normal-description-whitespace") => {
            "const d=value.querySelector('.tool-item-desc'),style=d.style.cssText,wide=lines(d);d.style.cssText='display:block;width:1px';const narrow=lines(d);d.style.cssText=style;return {collapsed:d.innerText==='one two three\\u00a0four',unbroken:wide===1,nonbreaking:narrow===3,empty:value.querySelector('.tool-param-desc').getBoundingClientRect().height===0}"
        }
        Some("prompt-trailing-newline") => {
            "return {preserved:lines(value.querySelector('.system-prompt-full'))===10}"
        }
        _ => return,
    };
    let failed = js(
        root,
        &format!(
            "const lines=e=>{{const r=document.createRange();r.selectNodeContents(e);return new Set([...r.getClientRects()].filter(q=>q.height>0).map(q=>Math.round(q.top))).size}};const ok=(()=>{{{checks}}})();return Object.keys(ok).filter(k=>!ok[k]).join()"
        ),
    );
    assert_eq!(failed.as_string().as_deref(), Some(""), "{name}");
}
