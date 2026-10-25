//! Artifact-console bodies retain literal ordered rows and independent controls.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{container, host, page_module, text, view};
use crate::native_artifact_console_support::{cases, factory, localized, native};
use maestro_web::{
    browser::sandbox_value::BrowserSandboxValue,
    dom::{DomContent as _, DomRoot},
    tools::artifacts::console::{Console, LogEntry, LogType},
};
use wasm_bindgen::{JsCast as _, JsValue, prelude::wasm_bindgen};
use web_sys::{Element, HtmlElement};

/// Retain a supplied native value as an ordinary console line.
fn log(value: impl Into<JsValue>) -> LogEntry {
    LogEntry {
        r#type: LogType::Log,
        text: BrowserSandboxValue(value.into()),
    }
}
/// A descendant required by the mounted console contract.
fn part(root: &DomRoot, selector: &str) -> Element {
    root.node
        .unchecked_ref::<Element>()
        .query_selector(selector)
        .unwrap()
        .unwrap()
}
/// Activate a native console control.
fn click(root: &DomRoot, selector: &str) {
    part(root, selector).unchecked_ref::<HtmlElement>().click();
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
        text(&part(&root, ".maestro-artifact-summary")),
        "console (0)"
    );
    assert!(part(&root, ".maestro-artifact-logs").has_attribute("hidden"));
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
        text(&part(&root, ".maestro-artifact-summary")),
        "console (1 error)"
    );
    click(&root, ".maestro-artifact-summary");
    let rows = part(&root, ".maestro-artifact-logs");
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
            text(&part(&root, ".maestro-artifact-summary")),
            case.summary
        );
    }
}
/// Native operands paint literally, finite shared iterables repeat and cycles terminate.
#[wasm_bindgen]
pub async fn artifact_console_dom_preserves_raw_value_domains(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let factory = factory(&import_module).await;
    let parent = container();
    let console = Console::new();
    let root = console.mount(&view).unwrap();
    parent.append_child(&root.node).unwrap();
    click(&root, ".maestro-artifact-summary");
    for case in cases().values {
        let operand = js_sys::JSON::parse(&case.operand.to_string()).unwrap();
        let operand = native(
            &factory,
            "operand",
            &js_sys::Array::of2(&case.kind.clone().into(), &operand),
        );
        if case.kind == "negative-zero" {
            assert!(js_sys::Object::is(&operand, &JsValue::from_f64(-0.0)));
        }
        let expected = if case.kind == "date" {
            format!(
                "[log] {}",
                native(&factory, "nativeString", &js_sys::Array::of1(&operand))
                    .as_string()
                    .unwrap()
            )
        } else {
            case.paint
        };
        *console.logs.borrow_mut() = vec![log(operand), log("later usable line")];
        console.update(&root, &view).unwrap();
        let rows = part(&root, ".maestro-artifact-logs");
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
    }
    parent.remove();
}
