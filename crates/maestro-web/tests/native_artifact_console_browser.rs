//! Artifact-console bodies retain literal ordered rows and independent controls.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{
    Clipboard, Recorded, call, computed, container, driver, frames, host, page_module, published,
    rgb, select, selection, text, until, view,
};
use crate::native_artifact_console_support::{cases, factory, localized, native};
use maestro_web::{
    browser::sandbox_value::BrowserSandboxValue,
    dom::{DomContent as _, DomRoot, DomView},
    tools::artifacts::console::{Console, LogEntry, LogType},
};
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, JsValue, prelude::wasm_bindgen};
use web_sys::{Element, HtmlElement};

/// Retain a supplied native value as an ordinary console line.
fn log(value: impl Into<JsValue>) -> LogEntry {
    LogEntry {
        r#type: LogType::Log,
        text: BrowserSandboxValue(value.into()),
    }
}
/// Activate a native console control.
fn click(root: &DomRoot, selector: &str) {
    select(&root.node, selector)
        .unchecked_ref::<HtmlElement>()
        .click();
}
/// Summary localization, literal positional rows and independent initialization.
#[wasm_bindgen]
pub async fn artifact_console_dom_summaries_and_literal_lines(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let _page = page_module(&import_module).await;
    let console = Console::new();
    let neighbour = Console::default();
    let root = console.mount(&view).unwrap();
    let parent = container();
    parent.append_child(&root.node).unwrap();
    assert_eq!(
        text(&select(&root.node, ".maestro-artifact-summary")),
        "console (0)"
    );
    assert!(select(&root.node, ".maestro-artifact-logs").has_attribute("hidden"));
    console.logs.borrow_mut().extend([
        log("literal <&> 雪"),
        LogEntry {
            r#type: LogType::Error,
            text: BrowserSandboxValue("bad".into()),
        },
        log("literal <&> 雪"),
    ]);
    console.update(&root, &view).unwrap();
    assert!(neighbour.logs.borrow().is_empty());
    assert_eq!(
        text(&select(&root.node, ".maestro-artifact-summary")),
        "console (1 error)"
    );
    click(&root, ".maestro-artifact-summary");
    let rows = select(&root.node, ".maestro-artifact-logs");
    assert_eq!(rows.child_nodes().length(), 3);
    assert_eq!(
        text(rows.first_child().as_ref().unwrap()),
        "[log] literal <&> 雪"
    );
    assert_eq!(rows.query_selector_all("literal").unwrap().length(), 0);
    let first = rows.first_child().unwrap();
    console.update(&root, &view).unwrap();
    assert!(first.is_same_node(rows.first_child().as_ref()));
    console.logs.borrow_mut().truncate(1);
    console.update(&root, &view).unwrap();
    assert_eq!(rows.child_nodes().length(), 1);
    assert!(first.is_same_node(rows.first_child().as_ref()));
    parent.remove();
    assert_summaries(&view);
}

/// All localized count branches recorded from the source component.
fn assert_summaries(view: &maestro_web::dom::DomView) {
    for case in cases().summaries {
        let console = Console::new();
        console
            .logs
            .borrow_mut()
            .extend(case.logs.into_iter().map(|line| LogEntry {
                r#type: match line.r#type.as_str() {
                    "log" => LogType::Log,
                    "error" => LogType::Error,
                    _ => panic!("unknown line kind"),
                },
                text: BrowserSandboxValue(line.text.into()),
            }));
        let root = console.mount(&localized(view, &case.language)).unwrap();
        assert_eq!(
            text(&select(&root.node, ".maestro-artifact-summary")),
            case.summary
        );
    }
}
/// Native operands paint literally, finite shared iterables repeat and cycles terminate.
#[wasm_bindgen]
pub async fn artifact_console_dom_preserves_raw_value_domains(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let page = page_module(&import_module).await;
    let clipboard = Recorded::install(Clipboard::Accept);
    let factory = factory(&import_module).await;
    let parent = container();
    let console = Console::new();
    let root = console.mount(&view).unwrap();
    parent.append_child(&root.node).unwrap();
    click(&root, ".maestro-artifact-summary");
    assert_eq!(
        select(&root.node, "copy-button")
            .get_attribute("title")
            .as_deref(),
        Some("Copy logs")
    );
    for case in cases().values {
        let (operand, expected, copied) = prepare(&factory, &case);
        *console.logs.borrow_mut() = vec![log(operand), log("later usable line")];
        console.update(&root, &view).unwrap();
        let rows = select(&root.node, ".maestro-artifact-logs");
        let row = rows.first_element_child().unwrap();
        assert_eq!(
            row.unchecked_ref::<HtmlElement>().inner_text(),
            expected,
            "{}",
            case.kind
        );
        assert_eq!(
            text(rows.last_child().as_ref().unwrap()),
            "[log] later usable line"
        );
        assert_eq!(
            rows.query_selector_all("[data-artifact-oracle-forged]")
                .unwrap()
                .length(),
            0
        );
        let copy = root
            .node
            .unchecked_ref::<Element>()
            .query_selector("copy-button button")
            .unwrap();
        assert!(
            copy.is_some(),
            "expanded console must expose its copy consumer"
        );
        let before = clipboard.texts().len();
        copy.unwrap().unchecked_ref::<HtmlElement>().click();
        frames(&page, 1).await;
        assert_eq!(
            &clipboard.texts()[before..],
            &[format!("{copied}\n[log] later usable line")]
        );
    }
    assert_error_operands(&console, &root, &view, &page, &clipboard).await;
    growth(&console, &root, &view, &page, &factory).await;
    parent.remove();
}

/// Construct a native operand and its separately recorded paint/copy observations.
fn prepare(
    factory: &JsValue,
    case: &crate::native_artifact_console_support::Value,
) -> (JsValue, String, String) {
    let operand = js_sys::JSON::parse(&case.operand.to_string()).unwrap();
    let operand = native(
        factory,
        "operand",
        &js_sys::Array::of2(&JsValue::from_str(&case.kind), &operand),
    );
    if case.kind == "negative-zero" {
        assert!(js_sys::Object::is(&operand, &JsValue::from_f64(-0.0)));
    }
    if case.kind == "shared-array" {
        assert_ne!(
            native(factory, "deduplicated", &js_sys::Array::of1(&operand))
                .as_string()
                .unwrap(),
            "abab"
        );
    }
    let paint = if case.kind == "date" {
        format!(
            "[log] {}",
            native(factory, "nativeString", &js_sys::Array::of1(&operand))
                .as_string()
                .unwrap()
        )
    } else {
        case.paint.clone().unwrap()
    };
    let copied = if case.kind == "date" {
        paint.clone()
    } else {
        case.copy.clone().unwrap()
    };
    (operand, paint, copied)
}

/// Mount one independent console into its own connected container.
fn place(view: &DomView, console: &Console) -> (Element, DomRoot) {
    let parent = container();
    let root = console.mount(view).unwrap();
    parent.append_child(&root.node).unwrap();
    (parent, root)
}
/// Expandable controls use current state, native forms, following and shared copy feedback.
#[wasm_bindgen]
pub async fn artifact_console_dom_expansion_copy_and_follow(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let page = page_module(&import_module).await;
    following_and_geometry(&view);
    current_controller_and_copy(&view, &page).await;
    forms_and_keyboard(&view).await;
    painted_roles(&view, &page).await;
    release_and_sibling(&view, &page).await;
}
/// An overflowing viewport follows every enabled update and scales in root-relative units.
fn following_and_geometry(view: &DomView) {
    let console = Console::new();
    let (parent, root) = place(view, &console);
    console
        .logs
        .borrow_mut()
        .extend((0..80).map(|i| log(format!("line {i}"))));
    console.update(&root, view).unwrap();
    assert_eq!(
        select(&root.node, ".maestro-artifact-logs")
            .child_nodes()
            .length(),
        0
    );
    click(&root, ".maestro-artifact-summary");
    let scroll = select(&root.node, ".maestro-artifact-logs");
    assert_disclosure(&root, true);
    let html = scroll.unchecked_ref::<HtmlElement>();
    viewport_sizes(&console, &root, view, &scroll);
    click(&root, ".maestro-artifact-follow");
    assert_eq!(
        select(&root.node, ".maestro-artifact-follow")
            .get_attribute("title")
            .as_deref(),
        Some("Autoscroll disabled")
    );
    html.set_scroll_top(17);
    let manual = html.scroll_top();
    assert!(manual > 0 && manual < html.scroll_height() - html.client_height());
    console.logs.borrow_mut().push(log("arrival"));
    console.update(&root, view).unwrap();
    assert_eq!(html.scroll_top(), manual);
    console.update(&root, view).unwrap();
    assert_eq!(html.scroll_top(), manual);
    click(&root, ".maestro-artifact-follow");
    html.set_scroll_top(0);
    console.update(&root, view).unwrap();
    assert!(html.scroll_top() + html.client_height() >= html.scroll_height() - 1);
    console.logs.borrow_mut().truncate(1);
    console.update(&root, view).unwrap();
    assert!(scroll.get_bounding_client_rect().height() < 192.0);
    click(&root, ".maestro-artifact-summary");
    assert!(scroll.has_attribute("hidden"));
    assert!(
        root.node
            .unchecked_ref::<Element>()
            .query_selector("copy-button")
            .unwrap()
            .is_none()
    );
    assert_disclosure(&root, false);
    parent.remove();
}
/// A later controller replaces collection, localization and actions without replacing the summary.
async fn current_controller_and_copy(view: &DomView, page: &JsValue) {
    let first = Console::new();
    let (parent, root) = place(view, &first);
    click(&root, ".maestro-artifact-summary");
    let summary = select(&root.node, ".maestro-artifact-summary");
    let mut later = Console::new();
    let german = localized(view, "de");
    later.update(&root, &german).unwrap();
    assert_eq!(
        summary.get_attribute("aria-expanded").as_deref(),
        Some("false")
    );
    later.logs = Rc::new(std::cell::RefCell::new(vec![
        LogEntry {
            r#type: LogType::Error,
            text: BrowserSandboxValue("new <&> 雪".into()),
        },
        log("tail"),
    ]));
    later.update(&root, &german).unwrap();
    click(&root, ".maestro-artifact-summary");
    assert!(summary.is_same_node(Some(&select(&root.node, ".maestro-artifact-summary"))));
    assert_eq!(text(&summary), "Konsole (1 error)");
    assert_eq!(
        select(&root.node, ".maestro-artifact-follow")
            .get_attribute("title")
            .as_deref(),
        Some("Automatisches Scrollen aktiviert")
    );
    click(&root, ".maestro-artifact-follow");
    assert_eq!(
        select(&root.node, ".maestro-artifact-follow")
            .get_attribute("title")
            .as_deref(),
        Some("Automatisches Scrollen deaktiviert")
    );
    let rows = select(&root.node, ".maestro-artifact-logs");
    call(
        page,
        "select",
        &js_sys::Array::of1(&rows.first_child().unwrap()),
    )
    .await;
    let selected = selection(page, &rows).await;
    summary.unchecked_ref::<HtmlElement>().focus().unwrap();
    later.update(&root, &german).unwrap();
    assert_eq!(selection(page, &rows).await, selected);
    assert_eq!(view.document.active_element(), Some(summary));
    assert_copy_feedback(page, &root, "[error] new <&> 雪\n[log] tail").await;
    later.logs.borrow_mut().clear();
    later.update(&root, &german).unwrap();
    let clipboard = Recorded::install(Clipboard::Accept);
    click(&root, "copy-button button");
    frames(page, 1).await;
    assert_eq!(clipboard.texts(), [""]);
    parent.remove();
}
/// The actual copy consumer uses icon-only feedback for the shared two-second interval.
async fn assert_copy_feedback(page: &JsValue, root: &DomRoot, expected: &str) {
    let clipboard = Recorded::install(Clipboard::Accept);
    let copy = select(&root.node, "copy-button");
    assert_eq!(
        copy.get_attribute("title").as_deref(),
        Some("Logs kopieren")
    );
    let started = js_sys::Date::now();
    click(root, "copy-button button");
    until(page, &copy, || {
        copy.query_selector("path[d='M20 6 9 17l-5-5']")
            .unwrap()
            .is_some()
    })
    .await;
    assert_eq!(clipboard.texts(), [expected]);
    assert!(text(&copy).is_empty());
    until(page, &copy, || {
        copy.query_selector("rect").unwrap().is_some()
    })
    .await;
    assert!(
        js_sys::Date::now() - started >= 2000.0,
        "artifact feedback must not use the shorter tool-output policy"
    );
}
/// Summary and following retain submit semantics while copy and keyboard traversal stay native.
async fn forms_and_keyboard(view: &DomView) {
    let form = view.document.create_element("form").unwrap();
    let parent = container();
    parent.append_child(&form).unwrap();
    let submitting = crate::native_artifact_controls_browser::Submitting::new(&form);
    let console = Console::new();
    let root = console.mount(view).unwrap();
    form.append_child(&root.node).unwrap();
    let summary = select(&root.node, ".maestro-artifact-summary");
    summary.unchecked_ref::<HtmlElement>().focus().unwrap();
    driver("driverPress", "Space").await;
    assert_eq!(
        summary.get_attribute("aria-expanded").as_deref(),
        Some("true")
    );
    driver("driverPress", "Tab").await;
    let follow = select(&root.node, ".maestro-artifact-follow");
    assert_eq!(view.document.active_element(), Some(follow.clone()));
    driver("driverPress", "Enter").await;
    assert_eq!(
        follow.get_attribute("aria-pressed").as_deref(),
        Some("false")
    );
    assert_eq!(submitting.count(), 2);
    driver("driverPress", "Tab").await;
    assert_eq!(
        view.document.active_element(),
        Some(select(&root.node, "copy-button button"))
    );
    let _clipboard = Recorded::install(Clipboard::Accept);
    driver("driverPress", "Enter").await;
    assert_eq!(submitting.count(), 2);
    driver("driverPress", "Shift+Tab").await;
    assert_eq!(view.document.active_element(), Some(follow));
    parent.remove();
}
/// Both modes paint semantic row roles and finished hover/focus controls.
async fn painted_roles(view: &DomView, page: &JsValue) {
    for mode in [
        maestro_theme::BrandMode::Dark,
        maestro_theme::BrandMode::Light,
    ] {
        let console = Console::new();
        console.logs.borrow_mut().extend([
            log("ordinary"),
            LogEntry {
                r#type: LogType::Error,
                text: BrowserSandboxValue("failed".into()),
            },
        ]);
        let (parent, root) = place(view, &console);
        let presentation = view.pack.presentation(mode).unwrap();
        for (key, value) in presentation.css_properties() {
            parent
                .unchecked_ref::<HtmlElement>()
                .style()
                .set_property(&key, &value)
                .unwrap();
        }
        parent
            .unchecked_ref::<HtmlElement>()
            .style()
            .set_property("background-color", presentation.colors["background"])
            .unwrap();
        click(&root, ".maestro-artifact-summary");
        assert_console_roles(&root, view, mode, page).await;
        parent.remove();
    }
}
/// Releasing a root drops its controller aliases and retires pending copy work without a sibling effect.
async fn release_and_sibling(view: &DomView, page: &JsValue) {
    let console = Console::new();
    console.logs.borrow_mut().push(log("retired"));
    let weak = Rc::downgrade(&console.logs);
    let (parent, root) = place(view, &console);
    click(&root, ".maestro-artifact-summary");
    let copy = select(&root.node, "copy-button");
    let deferred = Recorded::install(Clipboard::Defer);
    click(&root, "copy-button button");
    frames(page, 1).await;
    assert_eq!(deferred.texts(), ["[log] retired"]);
    let sibling = Console::new();
    let sibling_root = sibling.mount(view).unwrap();
    parent.append_child(&sibling_root.node).unwrap();
    drop(console);
    drop(root);
    assert!(weak.upgrade().is_none());
    assert!(!copy.is_connected());
    deferred.settle();
    frames(page, 1).await;
    assert!(
        copy.query_selector("rect").unwrap().is_some(),
        "retired copy completion must not publish feedback"
    );
    click(&sibling_root, ".maestro-artifact-summary");
    assert_eq!(
        select(&sibling_root.node, ".maestro-artifact-summary")
            .get_attribute("aria-expanded")
            .as_deref(),
        Some("true")
    );
    parent.remove();
    parent.append_child(&sibling_root.node).unwrap();
    crate::contributions_support::document()
        .body()
        .unwrap()
        .append_child(&parent)
        .unwrap();
    click(&sibling_root, ".maestro-artifact-summary");
    assert_eq!(
        select(&sibling_root.node, ".maestro-artifact-summary")
            .get_attribute("aria-expanded")
            .as_deref(),
        Some("false")
    );
    parent.remove();
}

/// Both line labels preserve scalar and iterable operands through paint and copying.
async fn assert_error_operands(
    console: &Console,
    root: &DomRoot,
    view: &DomView,
    page: &JsValue,
    clipboard: &Recorded,
) {
    for (operand, paint, copied) in [
        (
            JsValue::from_str("<error> 雪"),
            "[error] <error> 雪",
            "[error] <error> 雪",
        ),
        (
            js_sys::JSON::parse("[\"a\",null,\"b\"]").unwrap(),
            "[error] ab",
            "[error] a,,b",
        ),
    ] {
        *console.logs.borrow_mut() = vec![LogEntry {
            r#type: LogType::Error,
            text: BrowserSandboxValue(operand),
        }];
        console.update(root, view).unwrap();
        assert_eq!(
            select(&root.node, ".maestro-artifact-logs")
                .unchecked_ref::<HtmlElement>()
                .inner_text(),
            paint
        );
        let before = clipboard.texts().len();
        click(root, "copy-button button");
        frames(page, 1).await;
        assert_eq!(&clipboard.texts()[before..], &[copied]);
    }
}
/// Native producer work remains linear while test-side rescan and global-dedup controls fail the bound/result.
async fn growth(
    console: &Console,
    root: &DomRoot,
    view: &DomView,
    page: &JsValue,
    factory: &JsValue,
) {
    let clipboard = Recorded::install(Clipboard::Accept);
    for case in cases().costs {
        let probe = native(factory, "probe", &js_sys::Array::of1(&case.count.into()));
        let scalars =
            js_sys::Array::from(&js_sys::Reflect::get(&probe, &"scalars".into()).unwrap());
        *console.logs.borrow_mut() = scalars
            .iter()
            .enumerate()
            .map(|(i, value)| LogEntry {
                r#type: if i % 3 == 0 {
                    LogType::Error
                } else {
                    LogType::Log
                },
                text: BrowserSandboxValue(value),
            })
            .collect();
        console.update(root, view).unwrap();
        let before = clipboard.texts().len();
        click(root, "copy-button button");
        frames(page, 1).await;
        assert_eq!(&clipboard.texts()[before..], &[case.formatted]);
        let stats = js_sys::Array::from(&native(factory, "stats", &js_sys::Array::of1(&probe)));
        assert!(stats.get(1).as_f64().unwrap() <= f64::from(2 * case.count));
        let iterable = js_sys::Reflect::get(&probe, &"iterable".into()).unwrap();
        *console.logs.borrow_mut() = vec![log(iterable)];
        console.update(root, view).unwrap();
        assert_eq!(
            select(&root.node, ".maestro-artifact-logs")
                .unchecked_ref::<HtmlElement>()
                .inner_text(),
            format!("[log] {}", "x".repeat(case.count as usize))
        );
        let stats = js_sys::Array::from(&native(factory, "stats", &js_sys::Array::of1(&probe)));
        assert!(stats.get(0).as_f64().unwrap() <= f64::from(2 * case.count));
        assert!(
            native(factory, "rescan", &js_sys::Array::of1(&case.count.into()))
                .as_f64()
                .unwrap()
                > f64::from(2 * case.count)
        );
    }
    append_work(view, factory);
    arrival_bytes(view, page).await;
    reentrant_history(view, &clipboard);
}

/// Native row paint and finished control transitions use the selected mode's roles.
async fn assert_console_roles(
    root: &DomRoot,
    view: &DomView,
    mode: maestro_theme::BrandMode,
    page: &JsValue,
) {
    let presentation = view.pack.presentation(mode).unwrap();
    let summary = select(&root.node, ".maestro-artifact-summary");
    driver("driverPointer", "[0,0,false]").await;
    call(page, "finish", &js_sys::Array::of1(&summary)).await;
    assert_eq!(
        computed(&summary, "color"),
        rgb(presentation.colors["muted"])
    );
    assert_eq!(
        computed(&select(&root.node, ".maestro-artifact-log"), "color"),
        rgb(presentation.colors["muted"])
    );
    assert_eq!(
        computed(&select(&root.node, ".maestro-artifact-error"), "color"),
        rgb(presentation.colors["error"])
    );
    assert_eq!(
        computed(&select(&root.node, ".maestro-artifact-logs"), "overflow-y"),
        "auto"
    );
    assert_eq!(
        computed(&select(&root.node, ".maestro-artifact-logs"), "font-family"),
        presentation.css_properties()["--maestro-type-mono-font"]
    );

    console_interaction_roles(root, view, mode, page).await;
}
/// Real pointer hits and keyboard focus paint the current control state.
async fn console_interaction_roles(
    root: &DomRoot,
    view: &DomView,
    mode: maestro_theme::BrandMode,
    page: &JsValue,
) {
    let presentation = view.pack.presentation(mode).unwrap();
    let summary = select(&root.node, ".maestro-artifact-summary");
    let rect = summary.get_bounding_client_rect();
    driver(
        "driverPointer",
        &format!(
            "[{},{},false]",
            rect.x() + rect.width() / 2.0,
            rect.y() + rect.height() / 2.0
        ),
    )
    .await;
    call(page, "finish", &js_sys::Array::of1(&summary)).await;
    assert_eq!(
        computed(&summary, "color"),
        rgb(presentation.colors["text"])
    );
    summary.unchecked_ref::<HtmlElement>().focus().unwrap();
    driver("driverPress", "Tab").await;
    let follow = select(&root.node, ".maestro-artifact-follow");
    assert_eq!(
        computed(&follow, "background-color"),
        rgb(presentation.colors["accent"])
    );
    assert_eq!(
        computed(&follow, "color"),
        rgb(presentation.colors["on-accent"])
    );
    assert_eq!(computed(&follow, "outline-style"), "solid");
    assert_eq!(
        computed(&follow, "outline-color"),
        rgb(presentation.colors["accent"])
    );
    driver(
        "driverPointer",
        &format!(
            "[{},{},true]",
            rect.x() + rect.width() / 2.0,
            rect.y() + rect.height() / 2.0
        ),
    )
    .await;
    assert_eq!(
        summary.get_attribute("aria-expanded").as_deref(),
        Some("false")
    );
}

/// The real scroll viewport scales with the document root's text size.
fn viewport_sizes(console: &Console, root: &DomRoot, view: &DomView, scroll: &Element) {
    let html = scroll.unchecked_ref::<HtmlElement>();
    let document_root = view.document.document_element().unwrap();
    let style = document_root.unchecked_ref::<HtmlElement>().style();
    let old = style.get_property_value("font-size").unwrap();
    for (size, height) in [("16px", 192.0), ("32px", 384.0)] {
        style.set_property("font-size", size).unwrap();
        console.update(root, view).unwrap();
        assert!((scroll.get_bounding_client_rect().height() - height).abs() < 1.0);
        assert!(html.scroll_top() + html.client_height() >= html.scroll_height() - 1);
    }
    style.set_property("font-size", &old).unwrap();
}

/// Glyph geometry and auxiliary-control admission track the actual disclosure state.
fn assert_disclosure(root: &DomRoot, expanded: bool) {
    assert_eq!(
        select(&root.node, ".maestro-artifact-summary path")
            .get_attribute("d")
            .as_deref(),
        Some(if expanded {
            "m6 9 6 6 6-6"
        } else {
            "m9 18 6-6-6-6"
        })
    );
    assert_eq!(
        select(&root.node, ".maestro-artifact-summary")
            .get_attribute("aria-expanded")
            .as_deref(),
        Some(if expanded { "true" } else { "false" })
    );
    if expanded {
        assert_eq!(
            select(&root.node, ".maestro-artifact-follow")
                .get_attribute("title")
                .as_deref(),
            Some("Autoscroll enabled")
        );
    }
}

/// Appending converts each new native value at most once, for its row, and never an old one;
/// copying converts only when activated.
fn append_work(view: &DomView, factory: &wasm_bindgen::JsValue) {
    let mut bounded = true;
    for expanded in [false, true] {
        for count in [16, 32, 64] {
            let console = Console::new();
            let (parent, root) = place(view, &console);
            if expanded {
                click(&root, ".maestro-artifact-summary");
            }
            let probe = js_sys::Reflect::get(factory, &"probe".into())
                .unwrap()
                .unchecked_into::<js_sys::Function>()
                .call1(&wasm_bindgen::JsValue::UNDEFINED, &count.into())
                .unwrap();
            let scalars = js_sys::Reflect::get(&probe, &"scalars".into())
                .unwrap()
                .unchecked_into::<js_sys::Array>();
            for value in scalars.iter() {
                console.logs.borrow_mut().push(log(value));
                console.append(&root, view).unwrap();
            }
            let stats = js_sys::Reflect::get(factory, &"stats".into())
                .unwrap()
                .unchecked_into::<js_sys::Function>()
                .call1(&wasm_bindgen::JsValue::UNDEFINED, &probe)
                .unwrap()
                .unchecked_into::<js_sys::Array>();
            let reads = stats.get(1).as_f64().unwrap();
            bounded &= reads <= if expanded { f64::from(count) } else { 0.0 };
            parent.remove();
        }
    }
    assert!(bounded, "append revisited accumulated history");
}

/// One arrival publishes the same bytes however long the history, while a deliberately wrong
/// producer republishing the whole history on each arrival grows with it.
async fn arrival_bytes(view: &DomView, page: &JsValue) {
    let mut arrival = Vec::new();
    let mut control = Vec::new();
    for count in [16, 32, 64] {
        let console = Console::new();
        let (parent, root) = place(view, &console);
        click(&root, ".maestro-artifact-summary");
        for _ in 0..count {
            console.logs.borrow_mut().push(log("x"));
            console.append(&root, view).unwrap();
        }
        let take = published(page, &root.node, "").await;
        console.logs.borrow_mut().push(log("x"));
        console.append(&root, view).unwrap();
        arrival.push(take());
        control.push(history_copy(page, count).await);
        parent.remove();
    }
    web_sys::console::log_1(
        &format!("MEASURE console arrival bytes {arrival:?}, history copy {control:?}").into(),
    );
    assert!(
        arrival[1] <= arrival[0] && arrival[2] <= arrival[0],
        "one arrival republished the history: {arrival:?}"
    );
    assert!(
        control[0] < control[1] && control[1] < control[2],
        "the meter missed a history copy: {control:?}"
    );
}
/// Bytes the next arrival publishes after `count` lines when every arrival rewrites one
/// attribute with the whole history.
async fn history_copy(page: &JsValue, count: usize) -> f64 {
    let copy = container();
    let history = vec!["[log] x"; count].join("\n");
    copy.set_attribute("text", &history).unwrap();
    let take = published(page, &copy, "").await;
    copy.set_attribute("text", &format!("{history}\n[log] x"))
        .unwrap();
    let bytes = take();
    copy.remove();
    bytes
}
/// Native coercion while rendering or copying can append to the live history without keeping
/// a conflicting borrow; copying converts the history it finds when activated.
fn reentrant_history(view: &DomView, clipboard: &Recorded) {
    for append in [false, true] {
        let console = Console::new();
        let (parent, root) = place(view, &console);
        click(&root, ".maestro-artifact-summary");
        let copying = Rc::new(std::cell::Cell::new(false));
        let (value, _conversion) = reentrant_value(&console.logs, &copying);
        console.logs.borrow_mut().push(log(value));
        if append {
            console.append(&root, view).unwrap();
        } else {
            console.update(&root, view).unwrap();
        }
        assert_eq!(console.logs.borrow().len(), 2);
        console.update(&root, view).unwrap();
        assert_eq!(
            select(&root.node, ".maestro-artifact-logs")
                .child_nodes()
                .length(),
            2
        );
        copying.set(true);
        click(&root, "copy-button button");
        assert_eq!(
            clipboard.texts().last().map(String::as_str),
            Some("[log] value\n[log] during\n[log] copying")
        );
        console.logs.borrow_mut().clear();
        parent.remove();
    }
}
/// A value whose conversion appends `during` once, then `copying` once while `copying` is set.
fn reentrant_value(
    logs: &Rc<std::cell::RefCell<Vec<LogEntry>>>,
    copying: &Rc<std::cell::Cell<bool>>,
) -> (JsValue, wasm_bindgen::closure::Closure<dyn Fn() -> String>) {
    let (logs, copying) = (Rc::clone(logs), Rc::clone(copying));
    let once = std::cell::Cell::new(false);
    let conversion = wasm_bindgen::closure::Closure::<dyn Fn() -> String>::new(move || {
        if !once.replace(true) {
            logs.borrow_mut().push(log("during"));
        }
        if copying.replace(false) {
            logs.borrow_mut().push(log("copying"));
        }
        "value".to_owned()
    });
    let value = js_sys::Object::new();
    js_sys::Reflect::set(&value, &"toString".into(), conversion.as_ref()).unwrap();
    (value.into(), conversion)
}
