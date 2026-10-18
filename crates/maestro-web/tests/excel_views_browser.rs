//! Actual browser observations of spreadsheet content, controls and ownership.
#![cfg(all(test, target_arch = "wasm32"))]
mod panel_browser_support;
#[path = "artifact_support/presentation.rs"]
mod presentation;
mod reading_browser_support;
pub mod support;
use maestro_web::{
    ArtifactElement, AttachmentOverlay, tools::artifacts::artifact_element::ArtifactView,
};
use reading_browser_support::{View, click, js};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
/// Qualified original modern workbook bytes.
fn bytes(name: &str) -> &'static [u8] {
    match name {
        "legacy-links" => {
            include_bytes!("../../maestro-app/tests/fixtures/workbooks/metadata-link-external.xls")
        }
        "legacy-rich" => include_bytes!(
            "../../maestro-app/tests/fixtures/workbooks/metadata-rich-shared-and-inline.xls"
        ),
        "legacy-shared" => include_bytes!(
            "../../maestro-app/tests/fixtures/workbooks/metadata-shared-sheet-local.xls"
        ),
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
/// An actual measured slot in canvas coordinates.
fn tab_rect(index: usize) -> egui::Rect {
    let rect = js(&format!(
        "(()=>{{const r=document.querySelectorAll('.maestro-excel-tab')[{index}].getBoundingClientRect();return [r.left,r.top,r.right,r.bottom]}})()"
    ));
    let a = js_sys::Array::from(&rect);
    let at = |i| num_traits::cast(a.get(i).as_f64().unwrap()).unwrap();
    egui::Rect::from_min_max(egui::pos2(at(0), at(1)), egui::pos2(at(2), at(3)))
}
/// Activate the center of an actual measured slot in canvas coordinates.
fn tab_position(index: usize) -> egui::Pos2 {
    tab_rect(index).center()
}
/// One unmodified key transition.
fn key(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
/// Each painted tab label's colour and the colour of the underline across its slot.
fn tab_paint(paint: &egui::FullOutput) -> Vec<(egui::Color32, Option<egui::Color32>)> {
    let lines: Vec<_> = paint
        .shapes
        .iter()
        .filter_map(|s| match s.shape {
            egui::Shape::LineSegment { points, stroke } => Some((points[0].x, stroke.color)),
            _ => None,
        })
        .collect();
    paint
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.fallback_color),
            _ => None,
        })
        .enumerate()
        .map(|(index, ink)| {
            let left = tab_rect(index).left();
            let line = lines.iter().find(|(x, _)| (x - left).abs() < 0.5);
            (ink, line.map(|(_, color)| *color))
        })
        .collect()
}
/// Both packs in both modes.
fn brands() -> impl Iterator<Item = (&'static str, maestro_theme::BrandMode)> {
    ["brand.json", "signal.json"].into_iter().flat_map(|pack| {
        [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ]
        .map(|mode| (pack, mode))
    })
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
                key(egui::Key::Tab, true),
            ],
        );
        view.draw(false, vec![key(egui::Key::Tab, false)]);
    }
    view.draw(false, vec![key(egui::Key::Enter, true)]);
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
    for (pack, mode) in brands() {
        view.pack = maestro_theme::load_brand_pack(pack, &support::brand::Pack).unwrap();
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
    // The surface follows the slot exactly; a narrower slot scrolls the table instead of squeezing it.
    for width in [0., 1., 180.] {
        view.size.x = width;
        view.draw(false, vec![]);
        check(&format!(
            "(()=>{{const r=document.querySelector('.maestro-excel'),p=r.parentElement,t=r.querySelector('table'),w={width},h={};return r.offsetWidth===w&&p.offsetWidth===w&&r.offsetHeight===h&&p.offsetHeight===h&&(t.offsetWidth>w?p.scrollWidth===t.offsetWidth:t.offsetWidth===w&&p.scrollWidth===w)}})()",
            view.size.y
        ));
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
    let shown = "(()=>{const s=document.querySelectorAll('.maestro-excel-sheet');return getComputedStyle(s[0]).display==='none'&&getComputedStyle(s[1]).display!=='none'})()";
    tab(&mut view, 1);
    check(shown);
    view.element.set_content(encoded("two"));
    view.draw(false, vec![]);
    check(shown);
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
    for (pack, mode) in brands() {
        view.pack = maestro_theme::load_brand_pack(pack, &support::brand::Pack).unwrap();
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
                    key(egui::Key::Tab, pressed),
                ],
            );
        }
    }
    view.draw(false, vec![]);
    check(
        "(()=>{const t=document.querySelectorAll('.maestro-excel-tab')[1].getBoundingClientRect();const r=document.querySelector('.maestro-excel').parentElement;const b=r.getBoundingClientRect();return r.scrollLeft>0&&t.left<b.right&&t.left>=b.left-1})()",
    );
    view.draw(false, vec![key(egui::Key::Enter, true)]);
    check(
        "getComputedStyle(document.querySelectorAll('.maestro-excel-sheet')[1]).display!=='none'&&document.querySelectorAll('.maestro-excel-sheet')[1].querySelector('td').textContent==='1000'",
    );
}

pub mod highlighter_support;

/// Retained binary cells reach mounted tables through the existing reader owner.
#[wasm_bindgen]
pub fn excel_legacy_metadata_reaches_retained_tables() {
    let mut view = View::new("legacy.xls", &encoded("legacy-links"));
    view.draw(false, vec![]);
    check(
        "document.querySelector('.maestro-excel td').textContent==='1234.50'&&document.querySelector('.maestro-excel td a').getAttribute('href')==='https://example.test/a?x=1&y=\"q\"'",
    );
    view.element.set_content(encoded("legacy-rich"));
    view.draw(false, vec![]);
    check(
        "document.querySelector('.maestro-excel td').textContent==='a<雪>b'&&!document.querySelector('.maestro-excel td b, .maestro-excel td span, .maestro-excel td img')",
    );
    view.element.set_content(encoded("legacy-shared"));
    view.draw(false, vec![]);
    check(
        "[...document.querySelectorAll('#sheet-First td')].map(c=>c.textContent).join('|')==='|||10||20||30'",
    );
    js("globalThis.legacyFirst=document.querySelector('#sheet-First')");
    tab(&mut view, 1);
    view.draw(false, vec![]);
    check(
        "[...document.querySelectorAll('#sheet-Second td')].map(c=>c.textContent).join('|')==='|||40||50'",
    );
    check(
        "getComputedStyle(document.querySelector('#sheet-Second').parentElement).display!=='none'",
    );
    tab(&mut view, 0);
    view.draw(false, vec![]);
    check(
        "[...document.querySelectorAll('#sheet-First td')].map(c=>c.textContent).join('|')==='|||10||20||30'",
    );
    check(
        "legacyFirst===document.querySelector('#sheet-First')&&getComputedStyle(legacyFirst.parentElement).display!=='none'",
    );
}

/// Tab labels and underlines follow selection, hover and focus in either pack and mode.
/// # Panics
/// Panics when a painted label or underline differs from its brand role.
#[wasm_bindgen]
pub fn excel_tab_paint_follows_selection_hover_and_focus() {
    let away = egui::Event::PointerMoved(egui::pos2(900., 700.));
    for (pack, mode) in brands() {
        let mut view = View::new("a.xlsx", &encoded("scroll"));
        view.pack = maestro_theme::load_brand_pack(pack, &support::brand::Pack).unwrap();
        view.mode = mode;
        let p = view.pack.presentation(mode).unwrap();
        let role = |name: &str| egui::Color32::from_hex(p.colors[name]).unwrap();
        let [accent, muted, text, border] = ["accent", "muted", "text", "border"].map(role);
        view.draw(false, vec![]);
        let rest = view.draw(false, vec![away.clone()]);
        assert_eq!(tab_paint(&rest), [(accent, Some(accent)), (muted, None)]);
        view.draw(false, vec![egui::Event::PointerMoved(tab_position(1))]);
        let hover = view.draw(false, vec![]);
        assert_eq!(
            tab_paint(&hover),
            [(accent, Some(accent)), (text, Some(border))]
        );
        for _ in 0..2 {
            view.draw(false, vec![away.clone(), key(egui::Key::Tab, true)]);
            view.draw(false, vec![key(egui::Key::Tab, false)]);
        }
        let focus = view.draw(false, vec![]);
        assert_eq!(
            tab_paint(&focus),
            [(accent, Some(accent)), (text, Some(border))]
        );
    }
}

/// Striped rows, cell borders and each owner's tab strip follow the live brand.
/// # Panics
/// Panics when a computed row, border or strip colour differs from its brand role.
#[wasm_bindgen]
pub fn excel_tables_and_strips_follow_live_brand() {
    let mut view = View::new("a.xlsx", &encoded("scroll"));
    let mut overlay = AttachmentOverlay::open(record(), None, view.host.clone());
    for (pack, mode) in brands() {
        view.pack = maestro_theme::load_brand_pack(pack, &support::brand::Pack).unwrap();
        view.mode = mode;
        let p = view.pack.presentation(mode).unwrap();
        let [panel, background, border] =
            ["panel", "background", "border"].map(|r| p.colors[r].to_owned());
        view.draw(false, vec![]);
        view.overlay(&mut overlay, vec![]);
        view.overlay(&mut overlay, vec![]);
        check(&format!(
            "(()=>{{const p=document.createElement('i');document.body.append(p);const css=v=>{{p.style.background=v;return getComputedStyle(p).backgroundColor}};const stripe=css('color-mix(in srgb, {panel} 30%, transparent)'),panel=css('{panel}'),background=css('{background}');p.style.border='1px solid {border}';const l=getComputedStyle(p),edge=[l.borderTopWidth,l.borderTopStyle,l.borderTopColor].join();p.remove();const bg=e=>getComputedStyle(e).backgroundColor;const a=document.querySelector('.maestro-excel-artifact'),o=document.querySelector('.maestro-excel:not(.maestro-excel-artifact)');const rows=a.querySelectorAll('.maestro-excel-sheet tbody tr'),c=getComputedStyle(rows[1].querySelector('td'));return bg(rows[1])===stripe&&bg(rows[2])==='rgba(0, 0, 0, 0)'&&['Top','Right','Bottom','Left'].every(s=>[c['border'+s+'Width'],c['border'+s+'Style'],c['border'+s+'Color']].join()===edge)&&bg(a.querySelector('.maestro-excel-tabs'))===background&&bg(o.querySelector('.maestro-excel-tabs'))===panel}})()"
        ));
    }
}

/// Primary tab gestures act only when pressed and released on one visible, enabled, topmost slot.
/// # Panics
/// Panics when a rejected gesture changes the sheet or the admitted one does not.
#[wasm_bindgen]
pub fn excel_tab_gestures_need_one_visible_enabled_topmost_slot() {
    let mut view = View::new("a.xlsx", &encoded("scroll"));
    view.draw(false, vec![]);
    view.draw(false, vec![]);
    let (first, second) = (tab_rect(0), tab_rect(1));
    let shown = |index: usize| {
        check(&format!(
            "getComputedStyle(document.querySelectorAll('.maestro-excel-sheet')[{index}]).display!=='none'"
        ));
    };
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    view.draw(
        false,
        vec![
            egui::Event::PointerMoved(first.center()),
            button(first.center(), true),
        ],
    );
    view.draw(
        false,
        vec![
            egui::Event::PointerMoved(second.center()),
            button(second.center(), false),
        ],
    );
    shown(0);
    view.element.set_visibility(false, None).unwrap();
    view.draw(false, click(second.center()));
    view.element.set_visibility(true, None).unwrap();
    view.draw(false, vec![]);
    shown(0);
    view.frame(click(second.center()), |ui, element, v| {
        ui.disable();
        element.render(ui, v).unwrap();
    });
    shown(0);
    let clip = egui::Rect::from_min_max(
        egui::Pos2::ZERO,
        egui::pos2(second.left() + 4., view.size.y),
    );
    let clipped = |ui: &mut egui::Ui, element: &mut dyn ArtifactElement, v: ArtifactView<'_>| {
        ui.set_clip_rect(clip);
        element.render(ui, v).unwrap();
    };
    view.frame(
        click(egui::pos2(second.left() + 8., second.center().y)),
        clipped,
    );
    shown(0);
    let covered = |ui: &mut egui::Ui, element: &mut dyn ArtifactElement, v: ArtifactView<'_>| {
        element.render(ui, v).unwrap();
        egui::Area::new(egui::Id::new("cover"))
            .order(egui::Order::Foreground)
            .fixed_pos(second.min)
            .show(ui.ctx(), |ui| {
                ui.allocate_exact_size(second.size(), egui::Sense::click())
            });
    };
    // A new area is hit-tested only after its first, invisible sizing frame.
    view.frame(vec![], covered);
    view.frame(vec![], covered);
    view.frame(click(second.center()), covered);
    shown(0);
    // The cover's layer stays hit-testable until one frame without it.
    view.frame(vec![], clipped);
    view.frame(
        click(egui::pos2(second.left() + 2., second.center().y)),
        clipped,
    );
    shown(1);
}
