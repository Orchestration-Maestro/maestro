//! Browser-executed assertions through the public native header boundary.
#![cfg(all(test, target_arch = "wasm32"))]
use base64::Engine as _;
use maestro_theme::{BrandMode, ThemeInfo, ThemeOperations};
use maestro_web::browser::attachment_content::BrowserAttachmentHost;
use maestro_web::export_html::template::{
    HeaderState, HeaderView, SessionDocument, load_document, localize_header_date, render_header,
};
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, prelude::*};

/// Supply only the checked-in brand pack, with no browser filesystem access.
struct Pack;
impl ThemeOperations for Pack {
    fn read_to_string(&self, _: &str) -> std::io::Result<String> {
        Ok(include_str!("../../../assets/brand/brand.json").into())
    }
    fn environment(&self, _: &str) -> Option<String> {
        unreachable!()
    }
    fn exists(&self, _: &str) -> bool {
        unreachable!()
    }
    fn read_dir(&self, _: &str) -> std::io::Result<Vec<String>> {
        unreachable!()
    }
    fn sort_by_name(&self, _: &mut [ThemeInfo]) -> std::io::Result<()> {
        unreachable!()
    }
}
/// Real native owner and canvas caller retained across repaints.
struct Header {
    state: HeaderState,
    pack: maestro_theme::BrandPack,
    document: SessionDocument,
    context: egui::Context,
    shapes: Vec<egui::epaint::ClippedShape>,
    actions: Vec<maestro_web::export_html::template::HeaderAction>,
    events: Vec<egui::Event>,
}
impl Header {
    /// Bind to the browser runner's supplied canvas.
    fn new() -> Self {
        std::panic::set_hook(Box::new(|info| {
            web_sys::console::error_1(&info.to_string().into());
        }));
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas: web_sys::HtmlCanvasElement = document
            .get_element_by_id("canvas")
            .unwrap()
            .dyn_into()
            .unwrap();
        Self {
            state: HeaderState::new(Rc::new(BrowserAttachmentHost::new(canvas).unwrap())),
            pack: maestro_theme::load_brand_pack("brand.json", &Pack).unwrap(),
            document: load_document(
                &base64::engine::general_purpose::STANDARD
                    .encode(r#"{"entries":[],"header":{"id":"roomy"}}"#),
                "",
                None,
            )
            .unwrap(),
            context: egui::Context::default(),
            shapes: vec![],
            actions: vec![],
            events: vec![],
        }
    }
    /// Draw with no caller spacing, returning the actual following widget top.
    fn draw(&mut self, size: egui::Vec2) -> f32 {
        let presentation = self.pack.presentation(BrandMode::Dark).unwrap();
        let mut following = 0.;
        let mut output = self.context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                events: std::mem::take(&mut self.events),
                ..Default::default()
            },
            |ui| {
                ui.set_max_size(size);
                ui.spacing_mut().item_spacing.y = 0.;
                self.actions = render_header(
                    ui,
                    &self.document,
                    &mut self.state,
                    HeaderView {
                        presentation: &presentation,
                        localize_date: &localize_header_date,
                    },
                )
                .unwrap();
                following = ui.allocate_space(egui::vec2(10., 10.)).1.top();
            },
        );
        self.shapes = std::mem::take(&mut output.shapes);
        output.textures_delta.clear();
        following
    }
}
/// Locate the live painted panel, not a reconstructed DOM sample.
fn panel() -> web_sys::Element {
    web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .query_selector(".maestro-session-header")
        .unwrap()
        .unwrap()
}
/// A roomy native header allocates its transparent tail before the next widget.
/// # Panics
/// Panics when the observed browser behavior differs from the required result.
#[wasm_bindgen]
pub fn header_trailing_spacing_precedes_following_widget() {
    let mut header = Header::new();
    let following = header.draw(egui::vec2(800., 600.));
    let bottom = panel().get_bounding_client_rect().bottom();
    let presentation = header.pack.presentation(BrandMode::Dark).unwrap();
    assert!(
        (f64::from(following) - bottom - presentation.spacing["lg"]).abs() < 0.1,
        "following widget at {following}, panel bottom {bottom}; trailing spacing must be allocated exactly once"
    );
}
/// Fractional native offsets on both axes survive the repaint's measurement.
/// # Panics
/// Panics when the observed browser behavior differs from the required result.
#[wasm_bindgen]
pub fn header_fractional_scroll_survives_repaint() {
    let mut header = Header::new();
    let size = egui::vec2(300., 40.);
    header.document = load_document(
        &base64::engine::general_purpose::STANDARD.encode(format!(
            r#"{{"entries":[],"header":{{"id":"{}"}}}}"#,
            "wide".repeat(200)
        )),
        "",
        None,
    )
    .unwrap();
    header.draw(size);
    let wrapper = panel().parent_element().unwrap().parent_element().unwrap();
    let read = |key: &str| {
        js_sys::Reflect::get(&wrapper, &key.into())
            .unwrap()
            .as_f64()
            .unwrap()
    };
    js_sys::Reflect::set(&wrapper, &"scrollLeft".into(), &17.6.into()).unwrap();
    js_sys::Reflect::set(&wrapper, &"scrollTop".into(), &25.6.into()).unwrap();
    let before = (read("scrollLeft"), read("scrollTop"));
    assert!(
        before.0.fract() != 0. && before.1.fract() != 0.,
        "browser must supply fractional offsets: {before:?}"
    );
    header.context.request_repaint();
    header.draw(size);
    let after = (read("scrollLeft"), read("scrollTop"));
    assert_eq!(
        after, before,
        "repaint must preserve both fractional native offsets"
    );
}
/// The public loader's absent header reaches the literal native fallback.
/// # Panics
/// Panics when the observed browser behavior differs from the required result.
#[wasm_bindgen]
pub fn header_absent_from_loaded_document_shows_unknown() {
    let mut header = Header::new();
    header.document = load_document(
        &base64::engine::general_purpose::STANDARD.encode(r#"{"entries":[]}"#),
        "",
        None,
    )
    .unwrap();
    header.draw(egui::vec2(800., 600.));
    assert_eq!(
        panel()
            .query_selector("h1")
            .unwrap()
            .unwrap()
            .text_content()
            .unwrap(),
        "Session: unknown"
    );
}

/// Native locale and literal text are consumed by the actual mounted factory.
/// # Panics
/// Panics when the observed browser behavior differs from the required result.
#[wasm_bindgen]
pub fn header_native_dates_use_browser_locale() {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/session_header.json")).unwrap();
    let mut header = Header::new();
    let mut count = 0;
    for row in rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["name"].as_str().unwrap().starts_with("date-"))
    {
        let input = &row["input"];
        let data = input.get("data").cloned().unwrap_or_else(
            || serde_json::json!({"entries":[],"header":{"timestamp":input["timestamp"]}}),
        );
        header.document = load_document(
            &base64::engine::general_purpose::STANDARD.encode(data.to_string()),
            "",
            None,
        )
        .unwrap();
        header.draw(egui::vec2(800., 600.));
        let expected = if row["output"].is_string() {
            &row["output"]
        } else {
            &row["output"]["info"][0][1]
        };
        let expected = if !maestro_app::is_truthy(&data["header"]["timestamp"]) {
            "unknown".into()
        } else if js(&JsValue::NULL, "return Intl.DateTimeFormat().resolvedOptions().timeZone==='UTC' && navigator.language==='en-US'").as_bool().unwrap() {
            expected.as_str().unwrap().to_owned()
        } else {
            js(&js_sys::JSON::parse(&data["header"]["timestamp"].to_string()).unwrap(), "return new Date(value).toLocaleString()").as_string().unwrap()
        };
        assert_eq!(
            panel()
                .query_selector(".info-value")
                .unwrap()
                .unwrap()
                .text_content()
                .unwrap(),
            expected
        );
        count += 1;
    }
    assert_eq!(count, 13);
    let populated = &rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "populated")
        .unwrap()["input"]["data"];
    header.document = load_document(
        &base64::engine::general_purpose::STANDARD.encode(populated.to_string()),
        "",
        None,
    )
    .unwrap();
    header.draw(egui::vec2(800., 600.));
    assert_eq!(
        panel()
            .query_selector("h1")
            .unwrap()
            .unwrap()
            .text_content()
            .unwrap(),
        "Session: session <雪> & \"quoted\""
    );
    let actual = js(&panel(), "return JSON.stringify([...value.querySelectorAll('.info-item')].map(e=>[e.querySelector('.info-label').textContent,e.querySelector('.info-value').textContent]))").as_string().unwrap();
    let actual: serde_json::Value = serde_json::from_str(&actual).unwrap();
    let expected = &rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "populated")
        .unwrap()["output"]["info"];
    assert_eq!(actual.as_array().unwrap().len(), 6);
    for i in 1..6 {
        assert_eq!(actual[i], expected[i]);
    }
    assert!(panel().query_selector("m").unwrap().is_none());
}

mod session_header_browser_support;
mod session_header_support;
use maestro_web::browser::attachment_content::ReadingPhase;
use num_traits::ToPrimitive as _;
use session_header_browser_support::{Slots, js, snapshot, unchanged};

/// Readback keeps unclipped natural flow while unequal slots wrap natively.
/// # Panics
/// Panics when the observed browser behavior differs from the required result.
#[wasm_bindgen]
pub fn reading_readback_keeps_natural_height_and_slot_space() {
    owned_factory_survives_document();
    let slots = Slots::new();
    let small = Slots::sizes([(40., 20.), (70., 30.), (100., 40.)]);
    let large = Slots::sizes([(140., 50.), (170., 60.), (200., 70.)]);
    let first = slots.publish(egui::vec2(300., 40.), &small);
    let next = slots.publish(egui::vec2(300., 40.), &large);
    assert!(first.height > 40. && next.height > first.height);
    for ((id, rect), (expected, size)) in next.controls.iter().zip(&large) {
        assert_eq!(id, expected);
        assert!((rect.width() - size.x).abs() < 0.01 && (rect.height() - size.y).abs() < 0.01);
    }
    let mut header = Header::new();
    header.draw(egui::vec2(300., 40.));
    let root = panel();
    assert!(
        (js(
            &root,
            "return value.querySelectorAll('.control-slot').length"
        )
        .as_f64()
        .unwrap()
            - 3.)
            .abs()
            < f64::EPSILON
    );
    assert!(!root.text_content().unwrap().contains("Toggle thinking"));
    assert!(root.get_bounding_client_rect().height() > 40.);
}

/// Bounded slot measurements restore fractional offsets, attributes and nodes.
/// # Panics
/// Panics when the observed browser behavior differs from the required result.
#[wasm_bindgen]
pub fn reading_slot_sizing_preserves_fractional_scroll_and_selection() {
    let slots = Slots::new();
    let initial = Slots::sizes([(40., 20.), (70., 30.), (100., 40.)]);
    slots.publish(egui::vec2(300., 40.), &initial);
    js(
        &slots.root(),
        "const w=value.parentElement; w.scrollLeft=17.6; w.scrollTop=25.6; const s=value.querySelectorAll('.test-slot'); s[0].removeAttribute('style'); s[1].setAttribute('style',''); s[2].setAttribute('style','opacity:1'); const r=document.createRange(); r.setStart(value.querySelector('h1').firstChild,0); r.setEnd(value.querySelector('h1').firstChild,10); const selection=window.getSelection(); selection.removeAllRanges(); selection.addRange(r);",
    );
    let saved = snapshot(&slots.root());
    assert!(
        js(&saved, "return value.left % 1 !== 0 && value.top % 1 !== 0")
            .as_bool()
            .unwrap()
    );
    let sizes = Slots::sizes([(110., 23.), (150., 37.), (190., 51.)]);
    slots
        .place(ReadingPhase::Measure, egui::vec2(300., 40.), &sizes)
        .unwrap();
    unchanged(&saved);
    let layout = slots.layout().unwrap();
    unchanged(&saved);
    for ((_, rect), (_, size)) in layout.controls.iter().zip(&sizes) {
        assert!((rect.width() - size.x).abs() < 0.01 && (rect.height() - size.y).abs() < 0.01);
    }
    slots.publish(egui::vec2(300., 600.), &sizes);
    assert_eq!(
        js(&slots.root(), "return window.getSelection().toString()")
            .as_string()
            .unwrap(),
        "selectable"
    );
    assert!(js(&saved, "return window.getSelection().anchorNode === value.anchor && window.getSelection().focusNode === value.focus").as_bool().unwrap());
}

/// Failures below the adapter preserve retained roots and attempt all cleanup.
/// # Panics
/// Panics when the observed browser behavior differs from the required result.
#[wasm_bindgen]
pub fn reading_slot_failures_restore_native_state() {
    for route in [Route::Retained, Route::Replacement] {
        for kind in ["slot0", "slot1", "slot2", "readback", "publish"] {
            for cleanup in [false, true] {
                slot_failure(route, kind, cleanup);
            }
        }
        slot_failure(route, "cleanup", true);
    }
}
/// Native writer route being faulted.
#[derive(Clone, Copy)]
enum Route {
    /// Update the currently published native root.
    Retained,
    /// Stage a new candidate without replacing the published root.
    Replacement,
}
/// Exercise one fault below the shared native transaction.
fn slot_failure(route: Route, kind: &str, cleanup: bool) {
    let replacement = matches!(route, Route::Replacement);
    let mut slots = Slots::new();
    let initial = Slots::sizes([(40., 20.), (70., 30.), (100., 40.)]);
    slots.publish(egui::vec2(300., 40.), &initial);
    js(
        &slots.root(),
        "const w=value.parentElement;w.scrollLeft=17.6;w.scrollTop=25.6;const r=document.createRange();r.setStart(value.querySelector('h1').firstChild,0);r.setEnd(value.querySelector('h1').firstChild,10);const s=window.getSelection();s.removeAllRanges();s.addRange(r)",
    );
    let old = snapshot(&slots.root());
    if replacement {
        slots.content = Slots::factory(Rc::clone(&slots.root));
    }
    let sizes = Slots::sizes([(110., 23.), (150., 37.), (190., 51.)]);
    slots
        .place(ReadingPhase::Measure, egui::vec2(300., 40.), &sizes)
        .unwrap();
    if kind == "publish" {
        slots.layout().unwrap();
    }
    let root = slots.root();
    let wrapper = slots.wrapper();
    let hook = js(
        &root,
        &format!(
            "const kind={kind:?}, cleanup={cleanup};\n{}",
            include_str!("session_header_browser_support/faults.js")
        ),
    );
    let result = if kind == "publish" {
        slots.place(ReadingPhase::Publish, egui::vec2(300., 40.), &sizes)
    } else {
        slots.layout().map(|_| ())
    };
    let evidence = js(&hook, "return value.stop()");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains(if kind == "cleanup" {
            "cleanup marker"
        } else {
            "operation marker"
        }),
        "original failure must win for {kind}"
    );
    assert!(js(&evidence, "return [0,1,2,3].every(i=>value.restored.includes(i)) && value.offsets.includes('scrollLeft') && value.offsets.includes('scrollTop')").as_bool().unwrap());
    if replacement {
        unchanged(&old);
        assert!(!wrapper.is_connected());
    } else if !cleanup {
        unchanged(&old);
    } else {
        assert!(js(&old, "return value.styles.slice(1).every((s,i)=>[...value.slots,value.wrapper][i+1].getAttribute('style')===s) && value.wrapper.scrollLeft===value.left && value.wrapper.scrollTop===value.top").as_bool().unwrap());
    }
}

/// Current bounded slots agree with paint clips under unequal axis scales.
/// # Panics
/// Panics when the observed browser behavior differs from the required result.
#[wasm_bindgen]
pub fn header_clamped_viewports_keep_controls_in_current_slots() {
    js(
        &web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id("canvas")
            .unwrap(),
        "value.style.width='1000px'; value.style.height='450px'",
    );
    let slots = Slots::new();
    for sizes in [
        Slots::sizes([(40., 20.), (70., 30.), (100., 40.)]),
        Slots::sizes([(140., 50.), (170., 60.), (200., 70.)]),
    ] {
        for height in [0., 1., 40., 600.] {
            let layout = slots.publish(egui::vec2(300., height), &sizes);
            let wrapper = slots.wrapper().get_bounding_client_rect();
            for ((id, expected), (_, element)) in layout
                .controls
                .iter()
                .zip(&slots.root.borrow().as_ref().unwrap().slots)
            {
                assert!(sizes.iter().any(|(slot, _)| slot == id));
                let actual = element.get_bounding_client_rect();
                let local = egui::Rect::from_min_size(
                    egui::pos2(
                        ((actual.x() - wrapper.x()) / 1.25).to_f32().unwrap(),
                        ((actual.y() - wrapper.y()) / 0.75).to_f32().unwrap(),
                    ),
                    egui::vec2(
                        (actual.width() / 1.25).to_f32().unwrap(),
                        (actual.height() / 0.75).to_f32().unwrap(),
                    ),
                );
                assert!(
                    (expected.min - local.min).length() < 0.1
                        && (expected.size() - local.size()).length() < 0.1
                );
            }
        }
    }
    drop(slots);
    let mut header = Header::new();
    for height in [0., 1., 40., 600.] {
        header.draw(egui::vec2(300., height));
        let slots = native_controls();
        let wrapper = panel()
            .parent_element()
            .unwrap()
            .parent_element()
            .unwrap()
            .get_bounding_client_rect();
        let viewport = egui::Rect::from_min_size(
            egui::pos2(
                (wrapper.x() / 1.25).to_f32().unwrap(),
                (wrapper.y() / 0.75).to_f32().unwrap(),
            ),
            egui::vec2(
                (wrapper.width() / 1.25).to_f32().unwrap(),
                (wrapper.height() / 0.75).to_f32().unwrap(),
            ),
        );
        for (slot, label) in slots
            .iter()
            .zip(["Toggle thinking", "Toggle tools", "↓ JSONL"])
        {
            let visible = slot.intersect(viewport);
            let text = header.shapes.iter().find(
                |shape| matches!(&shape.shape, egui::Shape::Text(t) if t.galley.text()==label),
            );
            assert_eq!(text.is_some(), visible.is_positive());
            if let Some(text) = text {
                assert!(
                    (text.clip_rect.min - visible.min).length() < 0.1
                        && (text.clip_rect.max - visible.max).length() < 0.1
                );
            }
            if !visible.is_positive() {
                click_header(&mut header, slot.center(), egui::vec2(300., height));
                assert!(header.actions.is_empty());
            }
        }
        click_header(
            &mut header,
            egui::pos2(290., 590.),
            egui::vec2(300., height),
        );
        assert!(header.actions.is_empty());
    }
    header.draw(egui::vec2(300., 40.));
    let before = native_controls();
    js(&panel(), "value.parentElement.parentElement.scrollTop=90.4");
    header.draw(egui::vec2(300., 40.));
    let after = native_controls();
    assert_ne!(before, after);
}
/// Observe current native control rectangles in canvas coordinates.
fn native_controls() -> Vec<egui::Rect> {
    let values = js(&panel(), "return JSON.stringify([...value.querySelectorAll('.control-slot')].map(e=>{const r=e.getBoundingClientRect();return [r.x/1.25,r.y/0.75,r.width/1.25,r.height/0.75]}))").as_string().unwrap();
    let rows: Vec<[f32; 4]> = serde_json::from_str(&values).unwrap();
    rows.into_iter()
        .map(|r| egui::Rect::from_min_size(egui::pos2(r[0], r[1]), egui::vec2(r[2], r[3])))
        .collect()
}
/// Send a complete pointer gesture through the actual canvas caller.
fn click_header(header: &mut Header, pos: egui::Pos2, size: egui::Vec2) {
    for pressed in [true, false] {
        header.events = vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        header.draw(size);
    }
}

/// Mount the actual retained factory after every source operand has been dropped.
fn owned_factory_survives_document() {
    let factory = {
        let host = Rc::new(session_header_support::Host::default());
        host.height.set(120.);
        let mut state = HeaderState::new(host.clone());
        let pack = maestro_theme::load_brand_pack("brand.json", &Pack).unwrap();
        let presentation = pack.presentation(BrandMode::Dark).unwrap();
        let mut doc = session_header_support::document(&serde_json::json!("owned <雪>"));
        doc.global_stats.models.push("literal <model>".into());
        session_header_support::draw(
            &mut state,
            &doc,
            &presentation,
            egui::Layout::top_down(egui::Align::Min),
            egui::vec2(300., 300.),
        )
        .0
        .unwrap();
        drop(doc);
        let retained = Rc::clone(&host.admissions.borrow()[0]);
        retained
    };
    let document = web_sys::window().unwrap().document().unwrap();
    let root = (factory.mount)(&document).unwrap();
    assert_eq!(root.slots.len(), 3);
    let text = root.element.text_content().unwrap();
    assert!(text.contains("Session: owned <雪>") && text.contains("literal <model>"));
    assert!(root.element.query_selector("model").unwrap().is_none());
}

/// Geometry returned by another same-origin realm reaches actual placement.
/// # Panics
/// Panics if the native rectangle cannot be converted or placement differs.
#[wasm_bindgen]
pub fn reading_geometry_accepts_second_realm_rectangle() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&info.to_string().into())
    }));
    let slots = Slots::new();
    let document = web_sys::window().unwrap().document().unwrap();
    let canvas = document.get_element_by_id("canvas").unwrap();
    let restore = js(
        &canvas,
        "const frame=document.createElement('iframe');document.body.append(frame);const original=value.getBoundingClientRect;const rect=new frame.contentWindow.DOMRect(20,30,1000,450);value.getBoundingClientRect=()=>rect;return ()=>{delete value.getBoundingClientRect;frame.remove()}",
    );
    let sizes = Slots::sizes([(40., 20.), (70., 30.), (100., 40.)]);
    let result = slots.place(ReadingPhase::Measure, egui::vec2(300., 40.), &sizes);
    if result.is_ok() {
        slots.layout().unwrap();
        slots
            .place(ReadingPhase::Publish, egui::vec2(300., 40.), &sizes)
            .unwrap();
        let rect = slots.wrapper().get_bounding_client_rect();
        assert!((rect.left() - 35.).abs() < 0.01);
        assert!((rect.top() - 43.5).abs() < 0.01);
        assert!((rect.width() - 375.).abs() < 0.01);
        assert!((rect.height() - 30.).abs() < 0.01);
    }
    restore
        .dyn_into::<js_sys::Function>()
        .unwrap()
        .call0(&JsValue::NULL)
        .unwrap();
    result.unwrap();
}
