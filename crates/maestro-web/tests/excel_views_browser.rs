//! Actual browser observations of spreadsheet content, controls and ownership.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::panel_browser_support;
use crate::reading_browser_support;
use crate::support;
use reading_browser_support::{View, check, click, js, run};
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
        "mixed-first" => {
            include_bytes!("../../maestro-app/tests/fixtures/workbooks/mixed-bad-first.xlsx")
        }
        "mixed-last" => {
            include_bytes!("../../maestro-app/tests/fixtures/workbooks/mixed-bad-last.xlsx")
        }
        "mixed-literal" => {
            include_bytes!("../../maestro-app/tests/fixtures/workbooks/mixed-literal-error.xlsx")
        }
        "mixed-control" => {
            include_bytes!("../../maestro-app/tests/fixtures/workbooks/mixed-valid-control.xlsx")
        }
        "all-corrupt" => {
            include_bytes!("../../maestro-app/tests/fixtures/workbooks/all-corrupt-two.xlsx")
        }
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
pub(crate) fn encoded(name: &str) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes(name))
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
    literal_number_format();
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
    run(
        "globalThis.first=document.querySelectorAll('.maestro-excel-sheet')[0];first.scrollTop=350;first.scrollLeft=150;globalThis.offsets=[first.scrollTop,first.scrollLeft];",
    );
    check(
        "offsets[0]>0&&offsets[1]>0&&first.querySelector('td').getBoundingClientRect().top>=first.getBoundingClientRect().top-1",
    );
    tab(&mut view, 1);
    check(
        "getComputedStyle(first).display==='none'&&getComputedStyle(document.querySelectorAll('.maestro-excel-sheet')[1]).display!=='none'",
    );
    run(
        "globalThis.second=document.querySelectorAll('.maestro-excel-sheet')[1];second.scrollTop=100;second.scrollLeft=50;globalThis.secondOffsets=[second.scrollTop,second.scrollLeft];",
    );
    tab(&mut view, 0);
    check(
        "first===document.querySelectorAll('.maestro-excel-sheet')[0]&&first.scrollTop===offsets[0]&&first.scrollLeft===offsets[1]",
    );
    keyboard_second_tab(&mut view);
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
        view.pack = support::brand::pack(pack).unwrap();
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
    localized_error(&mut view);
    view.element.set_content(encoded("one"));
    view.density = 1.25;
    run(
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
    run(
        "globalThis.table=document.querySelector('.maestro-excel table');const r=document.createRange();r.selectNodeContents(table.querySelector('td'));getSelection().removeAllRanges();getSelection().addRange(r);globalThis.selected=getSelection().toString();",
    );
    view.element.set_content(encoded("two"));
    view.draw(false, vec![]);
    check(
        "table===document.querySelector('.maestro-excel table')&&getSelection().toString()===selected",
    );
    drop(view);
    check("!document.querySelector('.maestro-excel')");
    let panel = panel_browser_support::Panel::new();
    panel.create("a.xlsx", &encoded("scroll"));
    panel.draw(click(tab_position(1)));
    run(
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

/// Workbook links use the live accent foreground in either pack and mode.
#[wasm_bindgen]
pub fn excel_links_follow_live_accent() {
    let mut view = View::new("a.xlsx", &encoded("links"));
    for pack in ["brand.json", "signal.json"] {
        view.pack = support::brand::pack(pack).unwrap();
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
    let _ = js("globalThis.legacyFirst=document.querySelector('#sheet-First')");
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

/// The artifact consumer preserves failed and valid tabs from their first frame.
/// # Panics
/// Panics if a mounted body, ordered tab or input transition differs from the contract.
#[wasm_bindgen]
pub fn excel_mixed_workbooks_show_valid_and_failed_tabs() {
    for name in ["mixed-first", "mixed-last"] {
        let mut view = mixed_view(name);
        view.draw(false, vec![]);
        check(
            "document.querySelectorAll('.maestro-excel-tab').length===2&&document.querySelectorAll('.maestro-excel-sheet').length===2",
        );
        let (bad, good) = if name == "mixed-first" {
            (0, 1)
        } else {
            (1, 0)
        };
        let _ = js(&format!(
            "globalThis.bad=document.querySelectorAll('.maestro-excel-sheet')[{bad}];globalThis.good=document.querySelectorAll('.maestro-excel-sheet')[{good}];"
        ));
        check(
            "bad.querySelector('h3').textContent==='Error loading spreadsheet'&&bad.querySelector('p').textContent==='spreadsheet: xl/worksheets/sheet1.xml: shared string 3'&&!bad.querySelector('table')&&good.querySelector('td').textContent==='42'",
        );
        check(if bad == 0 {
            "getComputedStyle(bad).display!=='none'&&getComputedStyle(good).display==='none'"
        } else {
            "getComputedStyle(good).display!=='none'&&getComputedStyle(bad).display==='none'"
        });
        view.draw(false, vec![]);
        view.draw(false, click(tab_position(good)));
        check("getComputedStyle(good).display!=='none'&&getComputedStyle(bad).display==='none'");
        view.draw(false, click(tab_position(bad)));
        check("getComputedStyle(bad).display!=='none'&&getComputedStyle(good).display==='none'");
        mixed_keyboard(&mut view, good);
        check("bad.isConnected&&good.isConnected&&getComputedStyle(good).display!=='none'");
        drop(view);
        check("!bad.isConnected&&!good.isConnected");
    }
}

/// Traverse registered controls until the actual sheet endpoint owns keyboard focus.
fn mixed_keyboard(view: &mut View, index: usize) {
    let target = tab_position(index);
    let mut reached = false;
    for _ in view.context.interactive_rects_last_pass() {
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
        reached = view
            .context
            .memory(egui::Memory::focused)
            .and_then(|id| view.context.read_response(id))
            .is_some_and(|r| r.rect.contains(target));
        if reached {
            break;
        }
    }
    assert!(reached, "sheet tab must be keyboard reachable");
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
}

/// Open the artifact consumer with original workbook bytes.
fn mixed_view(name: &str) -> View {
    let mut view = View::new("a.xlsx", "");
    view.element.set_content(encoded(name));
    view
}
/// Failed-sheet diagnostics stay literal, localized and correctly painted in place.
/// # Errors
/// Returns an invalid controlled brand presentation or color.
/// # Panics
/// Panics if the actual error body, fallback, localization or computed style is wrong.
#[wasm_bindgen]
pub fn excel_sheet_errors_are_literal_localized_and_live() -> Result<(), JsValue> {
    let mut fallback = View::new("a.xlsx", &encoded("zero"));
    fallback.draw(false, vec![]);
    check(
        "document.querySelector('.maestro-file-error p').textContent==='Failed to load spreadsheet'",
    );
    drop(fallback);
    let mut view = mixed_view("mixed-literal");
    view.draw(false, vec![]);
    let _ = js(
        "globalThis.literalRoot=document.querySelector('.maestro-excel');globalThis.literalHeading=literalRoot.querySelector('h3');",
    );
    check(
        "literalRoot.querySelector('p').textContent==='spreadsheet: xl/worksheets/sheet1.xml: shared string <img src=x onerror=alert(1)>'&&!literalRoot.querySelector('img,[onerror]')&&literalRoot.querySelector('td').textContent==='42'",
    );
    let translations = view.i18n.get_translations();
    let _ = translations.borrow()["en"].borrow_mut().insert(
        "Error loading spreadsheet".into(),
        maestro_web::TranslationValue::Text("Spreadsheet error localized".into()),
    );
    view.i18n.set_translations(translations);
    for pack in ["brand.json", "signal.json"] {
        view.pack = support::brand::pack(pack).map_err(|e| JsValue::from_str(&e.to_string()))?;
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            view.mode = mode;
            view.draw(false, vec![]);
            check(
                "literalRoot===document.querySelector('.maestro-excel')&&literalHeading===literalRoot.querySelector('h3')&&literalHeading.textContent==='Spreadsheet error localized'",
            );
            mixed_error_style(&view)?;
        }
    }
    drop(view);
    check("!literalRoot.isConnected");
    Ok(())
}
/// Compare the actual panel's computed colors and composited text contrast.
fn mixed_error_style(view: &View) -> Result<(), JsValue> {
    let p = view
        .pack
        .presentation(view.mode)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    for (role, selector, property) in [
        ("error", ".maestro-file-error h3", "color"),
        ("error", ".maestro-file-error>div", "borderTopColor"),
        ("panel", ".maestro-file-error>div", "backgroundColor"),
    ] {
        let color = egui::Color32::from_hex(p.colors[role])
            .map_err(|e| JsValue::from_str(&format!("{e:?}")))?;
        check(&format!(
            "getComputedStyle(document.querySelector('{selector}')).{property}==='rgb({}, {}, {})'",
            color.r(),
            color.g(),
            color.b()
        ));
    }
    check(
        r"(()=>{
        const panel=document.querySelector('.maestro-file-error>div'), heading=panel.querySelector('h3'), detail=panel.querySelector('p');
        const rgb=s=>s.match(/[\d.]+/g).slice(0,3).map(Number);
        const luminance=c=>c.map(v=>v/255).map(v=>v<=.04045?v/12.92:((v+.055)/1.055)**2.4).reduce((s,v,i)=>s+v*[.2126,.7152,.0722][i],0);
        const contrast=(a,b)=>{const x=luminance(a),y=luminance(b);return (Math.max(x,y)+.05)/(Math.min(x,y)+.05)};
        const background=rgb(getComputedStyle(panel).backgroundColor), ink=rgb(getComputedStyle(heading).color), alpha=Number(getComputedStyle(detail).opacity), detailInk=rgb(getComputedStyle(detail).color).map((v,i)=>v*alpha+background[i]*(1-alpha));
        return contrast(ink,background)>=4.5&&contrast(detailInk,background)>=4.5&&contrast(rgb(getComputedStyle(panel).borderTopColor),background)>=3&&panel.getBoundingClientRect().width>0;
    })()",
    );
    Ok(())
}

/// Retained mixed bodies preserve selection, rewrite cleanly and release with their owners.
/// # Panics
/// Panics if a native root is replaced early, retained after teardown or loses selection.
#[wasm_bindgen]
pub fn excel_mixed_sheet_roots_keep_selection_and_release_on_rewrite() {
    let mut view = View::new("a.xlsx", &encoded("mixed-first"));
    view.draw(false, vec![]);
    view.draw(false, vec![]);
    tab(&mut view, 1);
    let _ = js(
        "globalThis.mixedRoot=document.querySelector('.maestro-excel');globalThis.badRoot=mixedRoot.querySelectorAll('.maestro-excel-sheet')[0];globalThis.goodRoot=mixedRoot.querySelectorAll('.maestro-excel-sheet')[1];const range=document.createRange();range.selectNodeContents(goodRoot.querySelector('td'));getSelection().removeAllRanges();getSelection().addRange(range);globalThis.mixedSelection=getSelection().toString();",
    );
    check("mixedSelection==='42'");
    view.element.set_content(encoded("mixed-first"));
    view.draw(false, vec![]);
    check(
        "mixedRoot===document.querySelector('.maestro-excel')&&goodRoot===mixedRoot.querySelectorAll('.maestro-excel-sheet')[1]&&getSelection().toString()===mixedSelection",
    );
    tab(&mut view, 0);
    check(
        "badRoot===mixedRoot.querySelectorAll('.maestro-excel-sheet')[0]&&getComputedStyle(badRoot).display!=='none'&&getComputedStyle(goodRoot).display==='none'",
    );
    tab(&mut view, 1);
    check(
        "goodRoot===mixedRoot.querySelectorAll('.maestro-excel-sheet')[1]&&getComputedStyle(goodRoot).display!=='none'",
    );
    view.element.set_content(encoded("mixed-control"));
    view.draw(false, vec![]);
    check(
        "!mixedRoot.isConnected&&document.querySelector('.maestro-excel')!==mixedRoot&&!document.querySelector('.maestro-file-error')&&document.querySelectorAll('.maestro-excel table').length===2&&document.querySelector('td').textContent==='one'",
    );
    let _ = js("globalThis.repairedRoot=document.querySelector('.maestro-excel');");
    view.element.set_content(encoded("all-corrupt"));
    view.draw(false, vec![]);
    check(
        "!repairedRoot.isConnected&&!document.querySelector('.maestro-excel')&&document.querySelector('.maestro-file-error h3').textContent==='Error loading spreadsheet'&&document.querySelector('.maestro-file-error p').textContent==='spreadsheet: xl/worksheets/sheet1.xml: shared string 3'",
    );
    drop(view);
    check("!document.querySelector('.maestro-file-error')");
    let panel = panel_browser_support::Panel::new();
    panel.create("mixed.xlsx", &encoded("mixed-first"));
    let _ = js(
        "globalThis.mixedPanelRoot=document.querySelector('.maestro-excel');globalThis.mixedPanelError=mixedPanelRoot.querySelector('.maestro-file-error');",
    );
    check(
        "mixedPanelError.querySelector('p').textContent==='spreadsheet: xl/worksheets/sheet1.xml: shared string 3'&&mixedPanelRoot.querySelector('td').textContent==='42'",
    );
    drop(panel);
    check("!mixedPanelRoot.isConnected&&!mixedPanelError.isConnected");
}

/// Keep quoted number-format attributes on the native table.
fn literal_number_format() {
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

/// Activate the second tab through the unchanged keyboard gesture.
fn keyboard_second_tab(view: &mut View) {
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
}

/// Observe reached errors and localized download controls before recovery.
fn localized_error(view: &mut View) {
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
}
