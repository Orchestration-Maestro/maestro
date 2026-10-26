//! Real browser PDF pages, native errors and surface transitions.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::support;
use maestro_web::browser::attachment_content::*;
use num_traits::ToPrimitive as _;
use wasm_bindgen::{JsCast as _, prelude::*};
/// Original complete fixture bytes.
fn bytes(multi: bool) -> &'static [u8] {
    if multi {
        include_bytes!("../../maestro-app/tests/fixtures/pdf/synthetic-multipage.pdf")
    } else {
        include_bytes!("../../maestro-app/tests/fixtures/pdf/synthetic-tall.pdf")
    }
}
/// Original encoded bytes for either public consumer.
pub(crate) fn encoded(multi: bool) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes(multi))
}
use crate::reading_browser_support;
use reading_browser_support::{View, check, click, js, run};
/// All native pages retain original order, raster dimensions and bounded geometry.
#[wasm_bindgen]
pub fn pdf_native_pages_keep_order_geometry_and_separators() {
    let mut view = View::new("current.pdf", &encoded(true));
    for count in 1..=3 {
        view.draw(false, vec![]);
        assert_eq!(
            js("document.querySelectorAll('.maestro-pdf img').length")
                .as_f64()
                .unwrap()
                .to_bits(),
            f64::from(count).to_bits()
        );
    }
    check(
        "(()=>{const r=document.querySelector('.maestro-pdf');const p=[...r.querySelectorAll('img')];return r.querySelectorAll('hr').length===2&&p.every(p=>p.width>0&&p.getAttribute('width')==='300'&&p.getAttribute('height')==='150')&&p[0].src!==p[1].src&&p[0].src===p[2].src&&getComputedStyle(r).padding==='16px'&&r.lastElementChild.lastElementChild.tagName==='IMG'&&getComputedStyle(p[0]).backgroundColor==='rgb(255, 255, 255)'&&getComputedStyle(p[0]).boxShadow!=='none'&&getComputedStyle(p[0]).borderRadius!=='0px'&&r.parentElement.scrollHeight>r.parentElement.clientHeight})()",
    );
    run(
        "globalThis.pages=[...document.querySelectorAll('.maestro-pdf img')].map(p=>p.src);globalThis.root=document.querySelector('.maestro-pdf');",
    );
    view.draw(false, vec![]);
    check(
        "root===document.querySelector('.maestro-pdf')&&pages.every((p,i)=>p===root.querySelectorAll('img')[i].src)",
    );
    run(
        "globalThis.savedWrapper=root.parentElement;savedWrapper.scrollTop=41.5;globalThis.savedScroll=savedWrapper.scrollTop;document.querySelector('#canvas').style.width='600px';document.querySelector('#canvas').style.height='300px';",
    );
    view.density = 1.25;
    view.draw(false, vec![]);
    check(
        "root===document.querySelector('.maestro-pdf')&&savedWrapper.scrollTop===savedScroll&&Math.abs(savedWrapper.getBoundingClientRect().width-375)<1&&Math.abs(savedWrapper.getBoundingClientRect().height-187.5)<1",
    );
    run(
        "document.querySelector('#canvas').style.width='800px';document.querySelector('#canvas').style.height='600px';",
    );
    view.density = 1.;
    for width in [2., 0.] {
        view.size = egui::vec2(width, 300.);
        view.draw(false, vec![]);
        check(
            "document.querySelector('.maestro-pdf').parentElement.getBoundingClientRect().width<=2",
        );
    }
}

/// Artifact separators keep the source's collapsed page margins.
#[wasm_bindgen]
pub fn pdf_artifact_native_page_gap_follows_source_spacing() {
    let mut view = View::new("current.pdf", &encoded(true));
    for _ in 0..3 {
        view.draw(false, vec![]);
    }
    check(
        "(()=>{const p=[...document.querySelectorAll('.maestro-pdf img')];return p.length===3&&p.slice(1).every((p,i)=>Math.abs(p.getBoundingClientRect().top-document.querySelectorAll('.maestro-pdf img')[i].getBoundingClientRect().bottom-33)<0.5)})()",
    );
}

/// The artifact viewport paints the page background without a frame or shadow.
#[wasm_bindgen]
pub fn pdf_artifact_native_viewport_keeps_background_without_frame() {
    let mut view = View::new("current.pdf", &encoded(true));
    for pack in ["brand.json", "signal.json"] {
        view.pack = support::brand::pack(pack).unwrap();
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            view.mode = mode;
            view.draw(false, vec![]);
            check(
                "(()=>{const r=document.querySelector('.maestro-pdf-artifact'),s=getComputedStyle(r),p=document.createElement('div');r.append(p);p.style.background='var(--maestro-color-background)';const ok=s.backgroundColor===getComputedStyle(p).backgroundColor&&s.borderTopWidth==='0px'&&s.boxShadow==='none';p.remove();return ok})()",
            );
        }
    }
}

/// Equal-content edits preserve completed pages and the reader's position.
#[wasm_bindgen]
pub fn pdf_same_content_rewrite_retains_pages_and_scroll() {
    let mut view = View::new("current.pdf", &encoded(true));
    view.draw(false, vec![]);
    run("globalThis.firstPage=document.querySelector('.maestro-pdf img')");
    view.element.set_content(encoded(true));
    for _ in 0..2 {
        view.draw(false, vec![]);
    }
    check("firstPage===document.querySelector('.maestro-pdf img')");
    run(
        "globalThis.retainedRoot=document.querySelector('.maestro-pdf');globalThis.retainedPages=[...retainedRoot.querySelectorAll('img')];globalThis.retainedWrapper=retainedRoot.parentElement;retainedWrapper.scrollTop=41.5;globalThis.retainedScroll=retainedWrapper.scrollTop;",
    );
    check("retainedPages.length===3&&retainedScroll>0");
    view.element.set_content(encoded(true));
    view.draw(false, vec![]);
    check(
        "retainedRoot===document.querySelector('.maestro-pdf')&&retainedRoot.querySelectorAll('img').length===3&&retainedPages.every((p,i)=>p===retainedRoot.querySelectorAll('img')[i])&&retainedWrapper.scrollTop===retainedScroll",
    );
}

/// Caller brand properties govern chrome and localized selectable error text.
#[wasm_bindgen]
pub fn pdf_native_modes_and_error_text_follow_brand_and_localization() {
    let mut view = View::new("current.pdf", &encoded(true));
    for pack in ["brand.json", "signal.json"] {
        view.pack = support::brand::pack(pack).unwrap();
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            view.mode = mode;
            view.element.set_content(encoded(true));
            view.draw(false, vec![]);
            check(
                "(()=>{const p=document.querySelector('.maestro-pdf img'),r=p.parentElement;return getComputedStyle(p).borderTopColor===getComputedStyle(r).getPropertyValue('--maestro-color-border').trim().replace(/^#(.{2})(.{2})(.{2})$/,(_,a,b,c)=>`rgb(${parseInt(a,16)}, ${parseInt(b,16)}, ${parseInt(c,16)})`)&&getComputedStyle(p).backgroundColor==='rgb(255, 255, 255)'})()",
            );
            assert_artifact_error_chrome(&mut view);
        }
    }
    let translations = view.i18n.get_translations();
    for (key, value) in [
        ("Error loading PDF", "Erreur PDF"),
        ("Error loading file", "Erreur fichier"),
        ("Download", "Télécharger"),
    ] {
        translations.borrow()["en"].borrow_mut().insert(
            key.into(),
            maestro_web::TranslationValue::Text(value.into()),
        );
    }
    view.i18n.set_translations(translations);
    view.draw(false, vec![]);
    assert_eq!(
        js("document.querySelector('.maestro-file-error h3').textContent")
            .as_string()
            .unwrap(),
        "Erreur PDF"
    );
}

/// Edits cancel partial documents; the real header saves only current bytes.
#[wasm_bindgen]
pub fn pdf_native_replacement_and_download_use_current_document() {
    let mut view = View::new("current.pdf", &encoded(true));
    view.draw(false, vec![]);
    run("globalThis.oldPage=document.querySelector('.maestro-pdf img')");
    view.element.set_content(encoded(false));
    view.draw(false, vec![]);
    check(
        "!oldPage.isConnected&&document.querySelectorAll('.maestro-pdf img').length===1&&document.querySelector('.maestro-pdf img').getAttribute('height')==='300'",
    );
    run(
        "globalThis.currentPage=document.querySelector('.maestro-pdf img');globalThis.currentRoot=currentPage.closest('.maestro-pdf');globalThis.wrapper=currentRoot.parentElement;wrapper.scrollTop=10;",
    );
    for _ in 0..4 {
        view.draw(false, vec![]);
    }
    check(
        "currentPage===document.querySelector('.maestro-pdf img')&&currentRoot===document.querySelector('.maestro-pdf')&&document.querySelectorAll('.maestro-pdf img').length===1",
    );
    view.element.set_content(String::new());
    view.draw(false, vec![]);
    check("currentPage===document.querySelector('.maestro-pdf img')");
    view.element.set_content("invalid!".into());
    view.draw(false, vec![]);
    check(
        "!currentRoot.isConnected&&document.querySelector('.maestro-file-error h3').textContent==='Error loading PDF'",
    );
    view.element.set_content(String::new());
    view.draw(false, vec![]);
    check(
        "!document.querySelector('.maestro-file-error')&&document.querySelectorAll('.maestro-pdf img').length===0",
    );
    assert_complete_pdf_encodings(&mut view);
    view.element.set_content(encoded(false));
    view.draw(false, vec![]);
    view.draw(true, vec![]);
    view.draw(true, click(egui::pos2(18., 16.)));
    check("document.querySelector('a[download]')===null");
    run("globalThis.lastRoot=document.querySelector('.maestro-pdf')");
    drop(view);
    check("!lastRoot.isConnected");
}

/// Clearing a partly rendered document keeps only its completed first page.
#[wasm_bindgen]
pub fn pdf_native_clear_while_rendering_keeps_completed_pages() {
    let mut view = View::new("current.pdf", &encoded(true));
    view.draw(false, vec![]);
    run("globalThis.firstPage=document.querySelector('.maestro-pdf img')");
    view.element.set_content(String::new());
    for _ in 0..3 {
        view.draw(false, vec![]);
    }
    check(
        "(()=>{const p=document.querySelectorAll('.maestro-pdf img');return p.length===1&&p[0]===firstPage})()",
    );
}

/// Check literal diagnostics, caller styling and selection in the current mode.
fn assert_artifact_error_chrome(view: &mut View) {
    view.element.set_content("Zg==".into());
    let diagnostic =
        maestro_app::browser::utils::attachment_utils::pdf::PdfDocument::new(vec![102])
            .err()
            .unwrap()
            .to_string();
    view.draw(false, vec![]);
    assert_eq!(
        js("document.querySelector('.maestro-file-error h3').textContent")
            .as_string()
            .unwrap(),
        "Error loading PDF"
    );
    assert_eq!(
        js("document.querySelector('.maestro-file-error p').textContent")
            .as_string()
            .unwrap(),
        diagnostic
    );
    check(
        "(()=>{const r=document.querySelector('.maestro-file-error'),p=r.querySelector('div'),s=getComputedStyle(r),c=getComputedStyle(p);return c.maxWidth==='672px'&&getComputedStyle(r).padding==='16px'&&c.backgroundColor!==s.color&&c.borderTopColor===s.color&&c.overflowWrap==='anywhere'&&s.fontFamily.replaceAll(String.fromCharCode(34),'')===s.getPropertyValue('--maestro-type-body-font').trim().replaceAll(String.fromCharCode(34),'')})()",
    );
    run(
        "globalThis.errorRoot=document.querySelector('.maestro-file-error');const range=document.createRange();range.selectNodeContents(errorRoot.querySelector('p'));getSelection().removeAllRanges();getSelection().addRange(range);globalThis.selected=getSelection().toString();",
    );
    view.draw(false, vec![]);
    check(
        "errorRoot===document.querySelector('.maestro-file-error')&&getSelection().toString()===selected&&selected.length>0",
    );
}

/// Complete raw, prefixed and folded original documents through the real body.
fn assert_complete_pdf_encodings(view: &mut View) {
    for row in [
        encoded(true),
        format!("data:application/pdf;base64,{}", encoded(true)),
        format!(
            "data:application/pdf;base64,{}",
            encoded(true)
                .as_bytes()
                .chunks(72)
                .map(|p| std::str::from_utf8(p).unwrap())
                .collect::<Vec<_>>()
                .join("\n")
        ),
    ] {
        view.element.set_content(row);
        for _ in 0..3 {
            view.draw(false, vec![]);
        }
        check(
            "document.querySelectorAll('.maestro-pdf img').length===3&&document.querySelectorAll('.maestro-pdf hr').length===2",
        );
    }
}

/// Neighboring artifact pages and error bodies retain their actual native paint.
/// # Panics
/// Panics when capture loses existing paint or adds an outer frame shadow.
#[wasm_bindgen]
pub async fn reading_frame_neighbors_keep_existing_paint() {
    let mut view = View::new("current.pdf", &encoded(true));
    view.density = js("devicePixelRatio").as_f64().unwrap().to_f32().unwrap();
    for pack in ["brand.json", "signal.json"] {
        view.pack = support::brand::pack(pack).unwrap();
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            view.mode = mode;
            view.element.set_content(encoded(true));
            for _ in 0..4 {
                view.draw(false, vec![]);
            }
            support::reading_paint::observe("neighbors-artifact").await;
            view.element.set_content("invalid pdf".into());
            view.draw(false, vec![]);
            support::reading_paint::observe("neighbors-error").await;
        }
    }
}

/// Exercise the actual native content boundary, not a controlled outer host.
/// # Panics
/// Panics if an injected phase is not reached or changes its error category.
/// # Errors
/// Returns native fixture setup or restoration failures.
#[wasm_bindgen]
pub fn pptx_native_failures_retag_only_the_content_call() -> Result<(), JsValue> {
    let host = reading_browser_support::Host::new();
    let pack = support::brand::pack("brand.json").map_err(|e| JsValue::from_str(&e.to_string()))?;
    let presentation = pack
        .presentation(maestro_theme::BrandMode::Dark)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    for retained in [false, true] {
        for phase in [
            "create",
            "replacement",
            "placement",
            "content",
            "append",
            "visibility",
        ] {
            if phase != "replacement" || retained {
                presentation_phase(&host.native, phase, retained, &presentation)?;
            }
        }
    }
    Ok(())
}

/// Fault one reached native phase, then retry the same retained host.
fn presentation_phase(
    host: &BrowserAttachmentHost,
    phase: &str,
    retained: bool,
    p: &maestro_theme::BrandPresentation<'_>,
) -> Result<(), JsValue> {
    let id = egui::Id::new(("presentation-phase", retained, phase));
    if retained {
        if matches!(phase, "create" | "replacement") {
            host.set_retained_visibility(id, true, None)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
        } else {
            native_text(host, id, TextView::Plain, p)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
        }
        if phase == "append" {
            let _ = js("document.querySelector('pre').parentElement.remove()");
        }
    }
    let install = js_sys::Function::new_with_args(
        "phase",
        include_str!("reading_browser_support/presentation_fault.js"),
    );
    let fault = install.call1(&JsValue::NULL, &phase.into())?;
    let result = native_text(host, id, TextView::Presentation, p);
    assert!(
        stop_presentation_fault(&fault)?,
        "phase not reached: {phase}"
    );
    let Err(error) = result else {
        return Err("expected reached native failure".into());
    };
    assert_eq!(
        matches!(error, AttachmentHostError::Presentation(_)),
        phase == "content"
    );
    assert!(error.to_string().contains("native phase marker"));
    let nested = maestro_web::dialogs::attachment_overlay::AttachmentUiError::from(error);
    assert!(
        std::error::Error::source(&nested)
            .is_some_and(<dyn std::error::Error>::is::<AttachmentHostError>)
    );
    native_text(host, id, TextView::Presentation, p)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    host.remove(id);
    if phase == "content" {
        ordinary_content_neighbor(host, id, p)?;
    }
    Ok(())
}

/// Restore every patched browser method before inspecting the returned error.
fn stop_presentation_fault(fault: &JsValue) -> Result<bool, JsValue> {
    let stop = js_sys::Reflect::get(fault, &"stop".into())?.dyn_into::<js_sys::Function>()?;
    stop.call0(fault)?
        .as_bool()
        .ok_or_else(|| JsValue::from_str("missing native fault observation"))
}

/// The adjacent plain-text branch must not inherit the presentation-only retag.
fn ordinary_content_neighbor(
    host: &BrowserAttachmentHost,
    id: egui::Id,
    p: &maestro_theme::BrandPresentation<'_>,
) -> Result<(), JsValue> {
    let install = js_sys::Function::new_with_args(
        "phase",
        include_str!("reading_browser_support/presentation_fault.js"),
    );
    let fault = install.call1(&JsValue::NULL, &"content".into())?;
    let result = native_text(host, id, TextView::Plain, p);
    assert!(stop_presentation_fault(&fault)?);
    let Err(error) = result else {
        return Err("expected ordinary-text native failure".into());
    };
    assert!(matches!(error, AttachmentHostError::Host(_)));
    native_text(host, id, TextView::Plain, p).map_err(|e| JsValue::from_str(&e.to_string()))?;
    host.remove(id);
    Ok(())
}

/// Present one explicit text branch through the default native host.
fn native_text(
    host: &BrowserAttachmentHost,
    id: egui::Id,
    view: TextView,
    presentation: &maestro_theme::BrandPresentation<'_>,
) -> Result<(), AttachmentHostError> {
    host.present(
        AttachmentSurface {
            id,
            rect: egui::Rect::from_min_size(egui::pos2(10., 10.), egui::vec2(100., 100.)),
            clip_rect: egui::Rect::EVERYTHING,
            pixels_per_point: 1.,
            opacity: 1.,
            occluders: vec![],
            content: AttachmentContent::Text {
                text: "literal phase text",
                view,
            },
        },
        presentation,
    )
}
