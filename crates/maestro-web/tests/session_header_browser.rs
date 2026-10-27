//! Browser-executed assertions through the public native header boundary.
#![cfg(all(test, target_arch = "wasm32"))]
use base64::Engine as _;
use maestro_theme::BrandMode;
use maestro_web::browser::attachment_content::BrowserAttachmentHost;
use maestro_web::export_html::template::{
    HeaderState, HeaderView, SessionDocument, load_document, localize_header_date, render_header,
};
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, prelude::*};

use crate::support;
use support::brand::pack;
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
            pack: pack("brand.json").unwrap(),
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
pub async fn header_trailing_spacing_precedes_following_widget() {
    let mut header = Header::new();
    let following = header.draw(egui::vec2(800., 600.));
    let bottom = panel().get_bounding_client_rect().bottom();
    let presentation = header.pack.presentation(BrandMode::Dark).unwrap();
    assert!(
        (f64::from(following) - bottom - presentation.spacing["lg"]).abs() < 0.1,
        "following widget at {following}, panel bottom {bottom}; trailing spacing must be allocated exactly once"
    );
    support::reading_paint::observe("neighbors-header").await;
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
        let expected = localized(&data["header"]["timestamp"], expected);
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
    populated_replays_every_expectation(&mut header, &rows);
}

/// Mount the populated recording and compare its title, help, labels, date and metadata.
fn populated_replays_every_expectation(header: &mut Header, rows: &serde_json::Value) {
    let populated = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "populated")
        .unwrap();
    let data = &populated["input"]["data"];
    header.document = load_document(
        &base64::engine::general_purpose::STANDARD.encode(data.to_string()),
        "",
        None,
    )
    .unwrap();
    header.draw(egui::vec2(800., 600.));
    let expected = &populated["output"];
    for (selector, text) in [
        ("h1", &expected["title"]),
        (".help-hint", &expected["help"]),
    ] {
        assert_eq!(
            panel()
                .query_selector(selector)
                .unwrap()
                .unwrap()
                .text_content()
                .unwrap(),
            text.as_str().unwrap()
        );
    }
    session_header_support::assert_button_labels(&header.shapes, expected);
    let actual = js(&panel(), "return JSON.stringify([...value.querySelectorAll('.info-item')].map(e=>[e.querySelector('.info-label').textContent,e.querySelector('.info-value').textContent]))").as_string().unwrap();
    let actual: serde_json::Value = serde_json::from_str(&actual).unwrap();
    let mut info = expected["info"].clone();
    info[0][1] = localized(&data["header"]["timestamp"], &expected["info"][0][1]).into();
    assert_eq!(actual, info);
    assert!(panel().query_selector("m").unwrap().is_none());
}

/// Expect unknown for a falsy timestamp, the recorded en-US UTC text, else the browser's text.
fn localized(timestamp: &serde_json::Value, recorded: &serde_json::Value) -> String {
    if !maestro_app::is_truthy(timestamp) {
        "unknown".into()
    } else if js(&JsValue::NULL, "return Intl.DateTimeFormat().resolvedOptions().timeZone==='UTC' && navigator.language==='en-US'").as_bool().unwrap() {
        recorded.as_str().unwrap().to_owned()
    } else {
        js(&js_sys::JSON::parse(&timestamp.to_string()).unwrap(), "return new Date(value).toLocaleString()").as_string().unwrap()
    }
}

#[path = "session_header_browser_support/mod.rs"]
mod session_header_browser_support;
use crate::session_header_support;
use maestro_web::browser::attachment_content::{AttachmentHost as _, ReadingPhase};
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
        for kind in [
            "slot0",
            "slot1",
            "slot2",
            "readback",
            "readback-getter",
            "publish",
        ] {
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
    if kind.starts_with("publish") {
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
    let result = if kind.starts_with("publish") {
        slots.place(ReadingPhase::Publish, egui::vec2(300., 40.), &sizes)
    } else {
        slots.layout().map(|_| ())
    };
    let evidence = js(&hook, "return value.stop()");
    let error = result
        .expect_err(&format!("native {kind} fault must be reached"))
        .to_string();
    assert!(
        error.contains(if kind == "cleanup" {
            "cleanup marker"
        } else {
            "operation marker"
        }),
        "original failure must win for {kind}"
    );
    assert!(js(&evidence, "return [0,1,2,3,4].every(i=>value.restored.includes(i)) && value.offsets.includes('scrollLeft') && value.offsets.includes('scrollTop')").as_bool().unwrap());
    if replacement {
        unchanged(&old);
        assert!(!wrapper.is_connected());
    } else if !cleanup {
        unchanged(&old);
    } else if !kind.contains("axis") {
        assert!(js(&old, "return value.styles.slice(1).every((s,i)=>[...value.slots,value.wrapper,value.frame][i+1].getAttribute('style')===s) && value.wrapper.scrollLeft===value.left && value.wrapper.scrollTop===value.top").as_bool().unwrap());
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
    clamped_slots();
    let mut header = Header::new();
    clipped_header_controls(&mut header);
    header.draw(egui::vec2(300., 40.));
    let before = native_controls();
    js(&panel(), "value.parentElement.parentElement.scrollTop=90.4");
    header.draw(egui::vec2(300., 40.));
    let after = native_controls();
    assert_ne!(before, after);
    let size = egui::vec2(300., 600.);
    header.draw(size);
    click_header(&mut header, native_controls()[0].center(), size);
    assert_eq!(
        header.actions,
        [maestro_web::export_html::template::HeaderAction::ToggleThinking]
    );
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
        let pack = pack("brand.json").unwrap();
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

        Rc::clone(&host.admissions.borrow()[0])
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
        web_sys::console::error_1(&info.to_string().into());
    }));
    let slots = Slots::new();
    let document = web_sys::window().unwrap().document().unwrap();
    let canvas = document.get_element_by_id("canvas").unwrap();
    let restore = js(
        &canvas,
        "const frame=document.createElement('iframe');document.body.append(frame);const rect=new frame.contentWindow.DOMRect(20,30,1000,450);value.getBoundingClientRect=()=>rect;return ()=>{delete value.getBoundingClientRect;frame.remove()}",
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

/// One viewport retains native offsets and selection across all placement phases.
/// # Panics
/// Panics if a second element scrolls or retained browser state changes.
#[wasm_bindgen]
pub fn reading_frame_repaint_preserves_single_scroller_and_native_state() {
    let slots = Slots::new();
    let sizes = Slots::sizes([(40., 20.), (70., 30.), (100., 40.)]);
    slots.publish(egui::vec2(300., 40.), &sizes);
    js(
        &slots.root(),
        "const v=value.parentElement,f=v.parentElement;const r=document.createRange();r.selectNodeContents(value.querySelector('h1'));getSelection().removeAllRanges();getSelection().addRange(r);for(const e of [f,v,value,document.scrollingElement]){e.scrollLeft=17.6;e.scrollTop=25.6}",
    );
    assert!(js(&slots.root(), "const v=value.parentElement;return v.scrollLeft>0&&v.scrollTop>0&&[v.parentElement,value,document.scrollingElement].every(e=>e.scrollLeft===0&&e.scrollTop===0)").as_bool().unwrap());
    let saved = snapshot(&slots.root());
    slots.publish(egui::vec2(300., 40.), &sizes);
    unchanged(&saved);
    slots.publish(egui::vec2(300., 600.), &sizes);
    assert_eq!(
        slots
            .wrapper()
            .get_bounding_client_rect()
            .height()
            .to_bits(),
        slots.frame().get_bounding_client_rect().height().to_bits()
    );
    let unbounded = slots.publish(egui::vec2(260., f32::INFINITY), &sizes);
    assert!(unbounded.height > 40.);
    slots.publish(egui::vec2(300., 0.), &sizes);
    assert_eq!(
        slots
            .wrapper()
            .get_bounding_client_rect()
            .height()
            .to_bits(),
        0_f64.to_bits()
    );
    let root = slots.root();
    let frame = slots.frame();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = std::sync::Arc::clone(&calls);
    slots.context.set_request_repaint_callback(move |_| {
        observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    js(&slots.wrapper(), "value.dispatchEvent(new Event('scroll'))");
    assert!(calls.load(std::sync::atomic::Ordering::SeqCst) > 0);
    slots.host.remove(slots.id);
    calls.store(0, std::sync::atomic::Ordering::SeqCst);
    js(
        &root,
        "value.parentElement.dispatchEvent(new Event('scroll'))",
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(!frame.is_connected());
}

/// Both placement nodes participate in rollback for retained and pending roots.
/// # Panics
/// Panics if a native fault loses the published owner or skips restoration.
#[wasm_bindgen]
pub fn reading_frame_transitions_restore_both_nodes_after_failure() {
    reading_candidate_setup_failures_keep_the_published_root();
    for route in [Route::Retained, Route::Replacement] {
        for node in ["frame", "viewport", "slot0", "slot1", "slot2"] {
            slot_failure_moments(route, node);
        }
        slot_failure(route, "readback-axis", true);
    }
    for style in [None, Some(""), Some("opacity:0.7")] {
        let slots = Slots::new();
        slots.publish(egui::vec2(300., 40.), &[]);
        for element in [slots.wrapper(), slots.frame()] {
            if let Some(style) = style {
                element.set_attribute("style", style).unwrap();
            } else {
                element.remove_attribute("style").unwrap();
            }
        }
        let saved = snapshot(&slots.root());
        slots
            .place(ReadingPhase::Measure, egui::vec2(300., 40.), &[])
            .unwrap();
        slots.layout().unwrap();
        unchanged(&saved);
    }
}

/// Paint and native hit testing respect the viewport, ancestor and control holes.
/// # Panics
/// Panics when captured sentinel pixels or native hit targets escape their clips.
#[wasm_bindgen]
pub async fn reading_viewport_keeps_content_and_occluders_clipped() {
    let mut slots = Slots::new();
    slots.origin = egui::pos2(100., 100.);
    slots.density = js(&JsValue::NULL, "return devicePixelRatio")
        .as_f64()
        .unwrap()
        .to_f32()
        .unwrap();
    let bounds = egui::Rect::from_min_size(slots.origin, egui::vec2(300., 80.));
    slots.holes = vec![
        egui::Rect::from_min_max(egui::pos2(200., 125.), egui::pos2(240., 150.)),
        egui::Rect::from_min_max(egui::pos2(90., 170.), egui::pos2(120., 200.)),
        egui::Rect::from_min_max(egui::pos2(600., 400.), egui::pos2(650., 450.)),
    ];
    for (index, (clip, size, origin)) in [
        (egui::Rect::EVERYTHING, bounds.size(), bounds.min),
        (
            egui::Rect::from_min_max(egui::pos2(70., 70.), egui::pos2(500., 300.)),
            bounds.size(),
            bounds.min,
        ),
        (bounds, bounds.size(), bounds.min),
        (
            egui::Rect::from_min_max(egui::pos2(150., 110.), egui::pos2(500., 300.)),
            bounds.size(),
            bounds.min,
        ),
        (egui::Rect::NOTHING, bounds.size(), bounds.min),
        (
            egui::Rect::from_min_max(egui::pos2(700., 400.), egui::pos2(800., 500.)),
            bounds.size(),
            bounds.min,
        ),
        (egui::Rect::EVERYTHING, egui::vec2(0., 80.), bounds.min),
        (egui::Rect::EVERYTHING, egui::vec2(300., 0.), bounds.min),
        (
            egui::Rect::EVERYTHING,
            bounds.size(),
            egui::pos2(-20.5, 80.25),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        slots.clip = clip;
        slots.origin = origin;
        slots.publish(size, &[]);
        js(
            &slots.root(),
            "value.style.cssText='width:700px;height:500px;background:rgb(255,0,0)';if(!value.querySelector('.sentinel')){const e=document.createElement('div');e.className='sentinel';e.style.cssText='position:absolute;left:305px;top:10px;width:15px;height:20px;background:rgb(0,255,0)';value.append(e)}value.parentElement.parentElement.style.boxShadow='0 0 0 90px rgb(255,0,255)'",
        );
        support::reading_paint::observe(&format!("viewport-{index}")).await;
    }
}

/// Factory and native setup failures discard only the unpublished candidate.
fn reading_candidate_setup_failures_keep_the_published_root() {
    let mut slots = Slots::new();
    slots.publish(egui::vec2(300., 40.), &[]);
    let old = snapshot(&slots.root());
    slots.content = Rc::new(maestro_web::browser::attachment_content::ReadingContent {
        mount: Rc::new(|_| {
            Err(
                maestro_web::browser::attachment_content::AttachmentHostError::Host(
                    "factory marker".into(),
                ),
            )
        }),
    });
    assert!(
        slots
            .place(ReadingPhase::Measure, egui::vec2(300., 40.), &[])
            .unwrap_err()
            .to_string()
            .contains("factory marker")
    );
    unchanged(&old);
    slots.content = Slots::factory(Rc::clone(&slots.root));
    let stop = js(
        &JsValue::NULL,
        "const set=Element.prototype.setAttribute;Element.prototype.setAttribute=function(k,v){set.call(this,k,v);if(this.className==='maestro-reading-frame'&&k==='style'&&v.includes('visibility:hidden'))throw Error('setup marker')};return ()=>Element.prototype.setAttribute=set",
    );
    let result = slots.place(ReadingPhase::Measure, egui::vec2(300., 40.), &[]);
    stop.dyn_into::<js_sys::Function>()
        .unwrap()
        .call0(&JsValue::NULL)
        .unwrap();
    assert!(result.unwrap_err().to_string().contains("setup marker"));
    unchanged(&old);
    slots.publish(egui::vec2(300., 40.), &[]);
    assert!(!js(&old, "return value.root.isConnected").as_bool().unwrap());
}

/// Exercise every before/after and publication boundary for one retained node.
fn slot_failure_moments(route: Route, node: &str) {
    for moment in ["before", "after"] {
        for phase in ["", "publish-"] {
            let kind = format!("{phase}{node}-{moment}");
            slot_failure(route, &kind, false);
            slot_failure(route, &kind, true);
        }
    }
}

/// Compare every bounded slot with native placement under unequal axis scales.
fn clamped_slots() {
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
}

/// Compare native header slots and their clipped painted controls at every height.
fn clipped_header_controls(header: &mut Header) {
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
                click_header(header, slot.center(), egui::vec2(300., height));
                assert!(header.actions.is_empty());
            }
        }
        click_header(header, egui::pos2(290., 590.), egui::vec2(300., height));
        assert!(header.actions.is_empty());
    }
}

/// The native attachment layer intercepts real pointer input above every offline header control.
pub(crate) async fn attachment_layer_intercepts_header_controls(
    view: &Rc<maestro_web::dom::DomView>,
) {
    let canvas = view.document.create_element("canvas").unwrap();
    for (name, value) in [
        ("id", "canvas"),
        ("width", "800"),
        ("height", "600"),
        ("style", "width:800px;height:600px;margin-top:120px"),
    ] {
        canvas.set_attribute(name, value).unwrap();
    }
    view.document.body().unwrap().append_child(&canvas).unwrap();
    let header = Rc::new(std::cell::RefCell::new(Header::new()));
    header.borrow_mut().draw(egui::vec2(800., 600.));
    let listener = header_pointer_listener(&header, &canvas);
    canvas
        .add_event_listener_with_callback("click", listener.as_ref().unchecked_ref())
        .unwrap();
    for index in 0..3 {
        header_overlay_pointer(view, &header, &canvas, index).await;
    }
    canvas
        .remove_event_listener_with_callback("click", listener.as_ref().unchecked_ref())
        .unwrap();
    canvas.remove();
}

/// Compare one uncovered control with the same real pointer click intercepted by the attachment layer.
async fn header_overlay_pointer(
    view: &Rc<maestro_web::dom::DomView>,
    header: &Rc<std::cell::RefCell<Header>>,
    canvas: &web_sys::Element,
    index: u32,
) {
    use crate::contributions_support::select;
    use maestro_web::dom::DomContent as _;
    let slot = select(&panel(), &format!(".control-slot:nth-child({})", index + 1));
    let bounds = slot.get_bounding_client_rect();
    let (x, y) = (
        bounds.x() + bounds.width() / 2.,
        bounds.y() + bounds.height() / 2.,
    );
    assert!(
        view.document
            .element_from_point(x.to_f32().unwrap(), y.to_f32().unwrap())
            .unwrap()
            .is_same_node(Some(canvas))
    );
    let point = serde_json::json!([x, y, true]).to_string();
    crate::contributions_support::driver("driverPointer", &point).await;
    assert_eq!(
        header.borrow().actions.len(),
        1,
        "uncovered control {index}"
    );
    header.borrow_mut().actions.clear();
    let overlay =
        maestro_web::AttachmentOverlay::open(view, crate::attachment_support::attachment(), None)
            .unwrap();
    let root = overlay.mount(view).unwrap();
    let target = view
        .document
        .element_from_point(x.to_f32().unwrap(), y.to_f32().unwrap())
        .unwrap();
    assert!(root.node.contains(Some(&target)));
    crate::contributions_support::driver("driverPointer", &point).await;
    assert!(
        header.borrow().actions.is_empty(),
        "covered control {index}"
    );
    assert!(
        root.node.is_connected(),
        "clicks inside the body do not close it"
    );
    select(&root.node, "[data-close]")
        .unchecked_ref::<web_sys::HtmlElement>()
        .click();
    assert!(!root.node.is_connected());
    crate::contributions_support::driver("driverPointer", &point).await;
    assert_eq!(header.borrow().actions.len(), 1, "restored control {index}");
    header.borrow_mut().actions.clear();
}

/// Forward an admitted native click through the same controlled header caller as its canvas witnesses.
fn header_pointer_listener(
    header: &Rc<std::cell::RefCell<Header>>,
    canvas: &web_sys::Element,
) -> wasm_bindgen::closure::Closure<dyn Fn(web_sys::Event)> {
    let header = Rc::clone(header);
    let canvas = canvas.clone();
    wasm_bindgen::closure::Closure::new(move |event: web_sys::Event| {
        let rect = canvas.get_bounding_client_rect();
        let coordinate = |name: &str| {
            js_sys::Reflect::get(&event, &name.into())
                .unwrap()
                .as_f64()
                .unwrap()
        };
        let x = (coordinate("clientX") - rect.x()) * 800. / rect.width();
        let y = (coordinate("clientY") - rect.y()) * 600. / rect.height();
        click_header(
            &mut header.borrow_mut(),
            egui::pos2(x.to_f32().unwrap(), y.to_f32().unwrap()),
            egui::vec2(800., 600.),
        );
    })
}
