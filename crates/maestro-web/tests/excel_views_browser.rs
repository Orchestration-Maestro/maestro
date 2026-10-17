//! Actual browser observations of spreadsheet content, controls and ownership.
#![cfg(all(test, target_arch = "wasm32"))]
mod panel_browser_support;
#[path = "artifact_support/presentation.rs"]
mod presentation;
mod reading_browser_support;
pub mod support;
use maestro_web::AttachmentOverlay;
use reading_browser_support::{View, click, js};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
/// Qualified original modern workbook bytes.
fn bytes(name: &str) -> &'static [u8] {
    match name {
        "zero" => {
            include_bytes!("../../maestro-app/tests/fixtures/spreadsheet_html/zero-sheets.xlsx")
        }
        "scroll" => include_bytes!("fixtures/excel/scroll.xlsx"),
        "two" => {
            include_bytes!("../../maestro-app/tests/fixtures/spreadsheet_html/two-sheets.xlsx")
        }
        "rich" => include_bytes!("../../maestro-app/tests/fixtures/spreadsheet_html/rich.xlsx"),
        "merged" => include_bytes!("../../maestro-app/tests/fixtures/spreadsheet_html/merged.xlsx"),
        "links" => {
            include_bytes!("../../maestro-app/tests/fixtures/spreadsheet_html/active-links.xlsx")
        }
        "literal" => {
            include_bytes!("../../maestro-app/tests/fixtures/spreadsheet_html/literal-rich.xlsx")
        }
        "quoted" => {
            include_bytes!("../../maestro-app/tests/fixtures/spreadsheet_html/quoted-sheet.xlsx")
        }
        _ => include_bytes!("../../maestro-app/tests/fixtures/spreadsheet_html/one-sheet.xlsx"),
    }
}
/// Preserve the complete original byte input.
fn encoded(name: &str) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes(name))
}
/// Evaluate the actual mounted DOM, not prepared layout operands.
fn check(code: &str) {
    assert!(
        js(code).as_bool().unwrap(),
        "native observation failed: {code}"
    );
}
/// Original alias shared with the live overlay.
fn record() -> Rc<RefCell<maestro_web::Attachment>> {
    Rc::new(RefCell::new(maestro_web::Attachment {
        id: "same".into(),
        r#type: maestro_app::browser::utils::attachment_utils::AttachmentType::Document,
        file_name: "original.xlsx".into(),
        mime_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into(),
        size: bytes("two").len(),
        content: encoded("two"),
        extracted_text: Some("literal text".into()),
        preview: None,
    }))
}
/// Activate the center of an actual measured slot in canvas coordinates.
fn tab_position(index: usize) -> egui::Pos2 {
    let point = js(&format!(
        "(()=>{{const r=document.querySelectorAll('.maestro-excel-tab')[{index}].getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]}})()"
    ));
    let a = js_sys::Array::from(&point);
    egui::pos2(
        num_traits::cast(a.get(0).as_f64().unwrap()).unwrap(),
        num_traits::cast(a.get(1).as_f64().unwrap()).unwrap(),
    )
}
/// Activate a measured tab through the caller's actual input frame.
fn tab(view: &mut View, index: usize) {
    view.draw(false, click(tab_position(index)));
}
/// Native parser, table layout and safe markup agree with the qualified projection.
#[wasm_bindgen]
pub fn excel_native_tables_paint_formatted_text_merges_and_links() {
    let mut view = View::new("a.xlsx", &encoded("rich"));
    view.draw(false, vec![]);
    check(
        "document.querySelector('td b i').textContent==='Bold' && getComputedStyle(document.querySelector('td b')).fontWeight==='900' && getComputedStyle(document.querySelector('td span')).fontSize==='24px'",
    );
    view.element.set_content(encoded("merged"));
    view.draw(false, vec![]);
    check(
        "document.querySelector('td[rowspan=\"2\"][colspan=\"2\"]')!==null && document.querySelector('td').getBoundingClientRect().width>0",
    );
    view.element.set_content(encoded("literal"));
    view.draw(false, vec![]);
    check(
        "!document.querySelector('.maestro-excel img')&&document.querySelector('td').textContent.includes('<img src=x')",
    );
    view.element.set_content(encoded("quoted"));
    view.draw(false, vec![]);
    check(
        "document.querySelector(\".maestro-excel table\").id==='sheet-Quarter \"A\" & 雪'&&document.querySelector(\"td\").id==='sheet-Quarter \"A\" & 雪-A1'",
    );
    view.element.set_content(encoded("links"));
    view.draw(false, vec![]);
    check(
        "document.querySelectorAll('.maestro-excel a').length===2&&document.querySelector('.maestro-excel a').getAttribute('href')==='data:text/plain,hello'&&[...document.querySelectorAll('.maestro-excel a')].every(a=>!['javascript:','vbscript:'].includes(a.protocol))",
    );
    {
        use maestro_app::browser::ssf::{
            CellAddress, CellObject, CellValue, ExcelDataType, NumberFormat, Range, Ssf, WorkSheet,
            sheet_to_html,
        };
        let mut sheet = WorkSheet {
            cells: [(
                "A1".into(),
                CellObject {
                    t: Some(ExcelDataType::Number),
                    v: CellValue::Number(12.),
                    w: Some("12".into()),
                    h: None,
                    z: Some(NumberFormat::Code("0 \"quoted\" 雪".into())),
                    l: None,
                    xf: None,
                    f: None,
                    array_formula: None,
                },
            )]
            .into_iter()
            .collect(),
            range: Some(Range {
                s: CellAddress { r: 0, c: 0 },
                e: CellAddress { r: 0, c: 0 },
            }),
            merges: vec![],
        };
        let document = web_sys::window().unwrap().document().unwrap();
        let container = document.create_element("div").unwrap();
        container.set_inner_html(&sheet_to_html(&mut sheet, "literal", &Ssf::default()));
        assert_eq!(
            container
                .query_selector("td")
                .unwrap()
                .unwrap()
                .get_attribute("data-z")
                .as_deref(),
            Some("0 \"quoted\" 雪")
        );
    }
    let record = record();
    let mut overlay = AttachmentOverlay::open(record, None, view.host.clone());
    view.overlay(&mut overlay, vec![]);
    check("document.querySelector('.maestro-excel:not(.maestro-excel-artifact)')!==null");
}
/// Tab changes retain native sheet bodies and their independent scroll positions.
#[wasm_bindgen]
pub fn excel_sheet_tabs_switch_without_losing_sheet_scroll() {
    let mut view = View::new("a.xlsx", &encoded("scroll"));
    view.size = egui::vec2(240., 140.);
    view.draw(false, vec![]);
    let paint = view.draw(false, vec![]);
    let labels: Vec<_> = paint
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.job.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(labels, ["Zulu", "Alpha 雪"]);
    check(
        "document.querySelectorAll('.maestro-excel-tab').length===2 && getComputedStyle(document.querySelectorAll('.maestro-excel-sheet')[0]).display!=='none' && getComputedStyle(document.querySelectorAll('.maestro-excel-sheet')[1]).display==='none'",
    );
    js(
        "globalThis.first=document.querySelectorAll('.maestro-excel-sheet')[0];first.scrollTop=350;first.scrollLeft=150;globalThis.offsets=[first.scrollTop,first.scrollLeft];",
    );
    check(
        "offsets[0]>0&&offsets[1]>0&&first.querySelector('td').getBoundingClientRect().top>=first.getBoundingClientRect().top-1",
    );
    tab(&mut view, 1);
    check(
        "getComputedStyle(first).display==='none'&&getComputedStyle(document.querySelectorAll('.maestro-excel-sheet')[1]).display!=='none'",
    );
    js(
        "globalThis.second=document.querySelectorAll('.maestro-excel-sheet')[1];second.scrollTop=100;second.scrollLeft=50;globalThis.secondOffsets=[second.scrollTop,second.scrollLeft];",
    );
    tab(&mut view, 0);
    check(
        "first===document.querySelectorAll('.maestro-excel-sheet')[0]&&first.scrollTop===offsets[0]&&first.scrollLeft===offsets[1]",
    );
    for _ in 0..2 {
        view.draw(
            false,
            vec![
                egui::Event::PointerMoved(egui::pos2(900., 700.)),
                egui::Event::Key {
                    key: egui::Key::Tab,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        view.draw(
            false,
            vec![egui::Event::Key {
                key: egui::Key::Tab,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    view.draw(
        false,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    check(
        "getComputedStyle(second).display!=='none'&&second.scrollTop===secondOffsets[0]&&second.scrollLeft===secondOffsets[1]",
    );
    view.element.set_content(encoded("one"));
    view.draw(false, vec![]);
    check(
        "document.querySelectorAll('.maestro-excel-tab').length===0&&document.querySelectorAll('.maestro-excel table').length===1",
    );
}
/// Live brand changes reach actual cells and selectable localized errors.
#[wasm_bindgen]
pub fn excel_native_states_follow_live_brand_and_localization() {
    let mut view = View::new("a.xlsx", &encoded("two"));
    for pack in ["brand.json", "signal.json"] {
        view.pack = maestro_theme::load_brand_pack(pack, &support::brand::Pack).unwrap();
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            view.mode = mode;
            view.draw(false, vec![]);
            let p = view.pack.presentation(mode).unwrap();
            let color = egui::Color32::from_hex(p.colors["text"]).unwrap();
            let panel = egui::Color32::from_hex(p.colors["panel"]).unwrap();
            check(&format!(
                "getComputedStyle(document.querySelector('td')).color==='rgb({}, {}, {})'&&getComputedStyle(document.querySelector('td')).backgroundColor==='rgb({}, {}, {})'",
                color.r(),
                color.g(),
                color.b(),
                panel.r(),
                panel.g(),
                panel.b()
            ));
            check(
                "getComputedStyle(document.querySelector('td')).position==='sticky'&&getComputedStyle(document.querySelector('td')).fontWeight==='600'&&getComputedStyle(document.querySelector('td')).paddingLeft==='12px'",
            );
        }
    }
    view.element.set_content("hello!".into());
    view.draw(false, vec![]);
    check(
        "document.querySelector('.maestro-file-error h3').textContent==='Error loading spreadsheet'&&document.querySelector('.maestro-file-error p').textContent.length>0",
    );
    let translations = view.i18n.get_translations();
    for (key, value) in [
        ("Error loading spreadsheet", "Spreadsheet error localized"),
        (
            "Failed to load spreadsheet",
            "Spreadsheet fallback localized",
        ),
        ("Download", "Download localized"),
    ] {
        translations.borrow()["en"].borrow_mut().insert(
            key.into(),
            maestro_web::TranslationValue::Text(value.into()),
        );
    }
    view.i18n.set_translations(translations);
    view.element.set_content(encoded("zero"));
    view.draw(false, vec![]);
    check(
        "document.querySelector('.maestro-file-error h3').textContent==='Spreadsheet error localized'&&document.querySelector('.maestro-file-error p').textContent==='Spreadsheet fallback localized'&&getComputedStyle(document.querySelector('.maestro-file-error>div')).maxWidth==='672px'",
    );
    view.draw(true, vec![]);
    let header = view.draw(true, click(egui::pos2(18., 16.)));
    assert!(
        header
            .platform_output
            .events
            .iter()
            .any(|event| event.widget_info().label.as_deref() == Some("Download localized"))
    );
    view.element.set_content(encoded("one"));
    view.density = 1.25;
    js(
        "document.getElementById('canvas').style.width='640px';document.getElementById('canvas').style.height='360px';",
    );
    view.draw(false, vec![]);
    check(
        "(()=>{const r=document.querySelector('.maestro-excel');const b=r.getBoundingClientRect();return Math.abs(b.width/r.offsetWidth-1)<.02&&Math.abs(b.height/r.offsetHeight-.75)<.02})()",
    );
    for width in [0., 1., 180.] {
        view.size.x = width;
        view.draw(false, vec![]);
    }
}
/// Identical rewrites retain selection; transitions and drop release old roots.
#[wasm_bindgen]
pub fn excel_native_lifetimes_keep_selection_and_cancel_old_roots() {
    let mut view = View::new("a.xlsx", &encoded("two"));
    view.draw(false, vec![]);
    js(
        "globalThis.table=document.querySelector('.maestro-excel table');const r=document.createRange();r.selectNodeContents(table.querySelector('td'));getSelection().removeAllRanges();getSelection().addRange(r);globalThis.selected=getSelection().toString();",
    );
    view.element.set_content(encoded("two"));
    view.draw(false, vec![]);
    check(
        "table===document.querySelector('.maestro-excel table')&&getSelection().toString()===selected",
    );
    let record = record();
    let mut overlay = AttachmentOverlay::open(record.clone(), None, view.host.clone());
    view.overlay(&mut overlay, vec![]);
    view.overlay(&mut overlay, vec![]);
    js("globalThis.old=document.querySelector('.maestro-excel:not(.maestro-excel-artifact)');");
    view.toggle(&mut overlay, "Text");
    check("!old.isConnected");
    view.toggle(&mut overlay, "Spreadsheet");
    check("document.querySelector('.maestro-excel:not(.maestro-excel-artifact)')!==old");
    drop(overlay);
    check("!document.querySelector('.maestro-excel:not(.maestro-excel-artifact)')");
    drop(view);
    check("!document.querySelector('.maestro-excel')");
    let panel = panel_browser_support::Panel::new();
    panel.create("a.xlsx", &encoded("scroll"));
    panel.draw(click(tab_position(1)));
    js(
        "globalThis.panelRoot=document.querySelector('.maestro-excel');globalThis.panelSheet=panelRoot.querySelectorAll('.maestro-excel-sheet')[1];panelSheet.scrollTop=350;panelSheet.scrollLeft=150;globalThis.panelOffsets=[panelSheet.scrollTop,panelSheet.scrollLeft];globalThis.panelHeight=panelRoot.getBoundingClientRect().height;",
    );
    check("panelOffsets[0]>0&&panelOffsets[1]>0");
    panel.create("b.zip", "AAEC");
    check("panelRoot.isConnected&&getComputedStyle(panelRoot.parentElement).visibility==='hidden'");
    panel.caller.show_artifact("a.xlsx");
    panel.draw(vec![]);
    check(
        "panelRoot===document.querySelector('.maestro-excel')&&getComputedStyle(panelRoot.parentElement).visibility==='visible'&&panelRoot.getBoundingClientRect().height===panelHeight&&getComputedStyle(panelSheet).display!=='none'&&panelSheet.scrollTop===panelOffsets[0]&&panelSheet.scrollLeft===panelOffsets[1]",
    );
    drop(panel);
    check("!panelRoot.isConnected");
}
/// Native failures preserve controls, suppression policy and original downloads.
#[wasm_bindgen]
pub fn excel_native_host_failures_preserve_error_controls_and_download() {
    let view = View::new("a.xlsx", &encoded("two"));
    let record = record();
    let mut overlay = AttachmentOverlay::open(record.clone(), None, view.host.clone());
    *view.host.failure.borrow_mut() = Some("<literal> failure".into());
    view.overlay(&mut overlay, vec![]);
    view.overlay(&mut overlay, vec![]);
    check(
        "document.querySelector('.maestro-file-error h3').textContent==='Error loading file'&&document.querySelector('.maestro-file-error p').textContent==='<literal> failure'&&!document.querySelector('.maestro-file-error literal')",
    );
    record.borrow_mut().content = encoded("one");
    view.overlay(&mut overlay, vec![]);
    check("document.querySelector('.maestro-file-error p').textContent==='<literal> failure'");
    view.overlay(&mut overlay, click(egui::pos2(728., 24.)));
    assert!(Rc::ptr_eq(
        view.host.downloads.borrow().last().unwrap(),
        &record
    ));
    view.toggle(&mut overlay, "Text");
    view.toggle(&mut overlay, "Spreadsheet");
    check(
        "!document.querySelector('.maestro-file-error')&&document.querySelector('.maestro-excel table')!==null",
    );
    *view.host.failure.borrow_mut() = Some(String::new());
    view.overlay(&mut overlay, vec![]);
    view.overlay(&mut overlay, vec![]);
    check(
        "!document.querySelector('.maestro-file-error')&&document.querySelector('.maestro-excel table')!==null",
    );
    view.host.fail_download.set(true);
    assert!(matches!(
        view.try_overlay(&mut overlay, click(egui::pos2(728., 24.))),
        Err(maestro_web::browser::attachment_content::AttachmentUiError::Host(_))
    ));
    view.host.fail_download.set(false);
    view.overlay(&mut overlay, vec![]);
    check(
        "!document.querySelector('.maestro-file-error')&&document.querySelector('.maestro-excel table')!==null",
    );
}

/// Workbook links use the live accent foreground in either pack and mode.
#[wasm_bindgen]
pub fn excel_links_follow_live_accent() {
    let mut view = View::new("a.xlsx", &encoded("links"));
    for pack in ["brand.json", "signal.json"] {
        view.pack = maestro_theme::load_brand_pack(pack, &support::brand::Pack).unwrap();
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            view.mode = mode;
            view.draw(false, vec![]);
            let p = view.pack.presentation(mode).unwrap();
            let accent = egui::Color32::from_hex(p.colors["accent"]).unwrap();
            check(&format!(
                "document.querySelectorAll('.maestro-excel a').length===2&&[...document.querySelectorAll('.maestro-excel a')].every(a=>getComputedStyle(a).color==='rgb({}, {}, {})')",
                accent.r(),
                accent.g(),
                accent.b()
            ));
        }
    }
}

/// Keyboard traversal reveals clipped sheet tabs without admitting hidden pointer hits.
#[wasm_bindgen]
pub fn excel_keyboard_reveals_offscreen_sheet_tabs() {
    let mut view = View::new("a.xlsx", &encoded("scroll"));
    view.size = egui::vec2(45., 140.);
    view.draw(false, vec![]);
    view.draw(false, vec![]);
    check(
        "(()=>{const t=document.querySelectorAll('.maestro-excel-tab')[1].getBoundingClientRect();const r=document.querySelector('.maestro-excel').parentElement.getBoundingClientRect();return t.left>=r.right})()",
    );
    tab(&mut view, 1);
    check(
        "getComputedStyle(document.querySelectorAll('.maestro-excel-sheet')[0]).display!=='none'",
    );
    for _ in 0..2 {
        for pressed in [true, false] {
            view.draw(
                false,
                vec![
                    egui::Event::PointerMoved(egui::pos2(900., 700.)),
                    egui::Event::Key {
                        key: egui::Key::Tab,
                        physical_key: None,
                        pressed,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }
    view.draw(false, vec![]);
    check(
        "(()=>{const t=document.querySelectorAll('.maestro-excel-tab')[1].getBoundingClientRect();const r=document.querySelector('.maestro-excel').parentElement;const b=r.getBoundingClientRect();return r.scrollLeft>0&&t.left<b.right&&t.left>=b.left-1})()",
    );
    view.draw(
        false,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    check(
        "getComputedStyle(document.querySelectorAll('.maestro-excel-sheet')[1]).display!=='none'&&document.querySelectorAll('.maestro-excel-sheet')[1].querySelector('td').textContent==='1000'",
    );
}
