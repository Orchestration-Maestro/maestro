//! Composed transcript rows painted with brand roles and driven by native keyboard and scrolling.
use crate::contributions_support::{body, call, computed, container, driver, rgb, text};
use maestro_app::{NoCustomMessages, ToolResultMessage};
use maestro_theme::{BrandMode, BrandPresentation};
use maestro_web::{
    AbortedMessage, AssistantMessage, ToolMessageDebugView, UserMessage,
    components::messages::ToolResults,
    dom::{DomContent, DomRoot, DomView},
};
use serde_json::json;
use std::{collections::HashMap, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::{Element, HtmlElement};

/// One original result envelope for `id`.
fn result(id: &str, name: &str, output: &str) -> Rc<ToolResultMessage> {
    Rc::new(
        serde_json::from_value(json!({
            "role":"toolResult", "toolCallId":id, "toolName":name, "isError":false, "timestamp":0,
            "content":[{"type":"text","text":output}],
        }))
        .unwrap(),
    )
}
/// Every row kind of the visible-state inventory, mounted in order under `parent`.
fn rows(parent: &Element, view: &DomView) -> Vec<DomRoot> {
    let user = UserMessage::<NoCustomMessages> {
        message: Some(
            serde_json::from_value(json!({"role":"user","content":"user words","timestamp":0}))
                .unwrap(),
        ),
    };
    let results = HashMap::from([
        ("b".to_owned(), result("b", "bash", "listing words")),
        ("j".to_owned(), result("j", "javascript_repl", "2")),
    ]);
    let assistant = AssistantMessage {
        message: Some(super::assistant::message(
            json!([
                {"type":"thinking","thinking":"reasoning words"},
                {"type":"text","text":"assistant words"},
                {"type":"toolCall","id":"b","name":"bash","arguments":{"command":"ls"}},
                {"type":"toolCall","id":"j","name":"javascript_repl","arguments":{"code":"1 + 1"}},
            ]),
            "error",
        )),
        tool_results_by_id: Some(ToolResults::Native(Rc::new(results))),
        ..AssistantMessage::default()
    };
    if let Some(message) = &assistant.message {
        message.write().unwrap().error_message = Some("failed words".into());
    }
    let debug = ToolMessageDebugView {
        call_args: Some(json!({"command":"ls"})),
        result: Some(result("b", "bash", "debug words")),
        has_result: true,
    };
    let contents: [&dyn DomContent; 4] = [&user, &assistant, &debug, &AbortedMessage];
    contents
        .into_iter()
        .map(|content| {
            let root = content.mount(view).unwrap();
            parent.append_child(&root.node).unwrap();
            root
        })
        .collect()
}
/// The first element under `root` matching `selector`.
fn one(root: &Element, selector: &str) -> Element {
    root.query_selector(selector).unwrap().unwrap()
}
/// Both packs and modes, at narrow and wide widths, paint every part through its brand role.
pub(super) fn paint(view: &DomView) {
    for pack in ["brand.json", "signal.json"] {
        let pack = crate::brand::pack(pack).unwrap();
        for mode in [BrandMode::Dark, BrandMode::Light] {
            for (width, zoom) in [(280, "1"), (800, "1.5")] {
                paint_size(view, &pack.presentation(mode).unwrap(), width, zoom);
            }
        }
    }
}
/// Measure painted text and native controls at one width.
fn paint_size(view: &DomView, presentation: &BrandPresentation<'_>, width: u32, zoom: &str) {
    let parent = container();
    let style = parent.unchecked_ref::<HtmlElement>().style();
    for (key, value) in presentation.css_properties() {
        style.set_property(&key, &value).unwrap();
    }
    style.set_property("width", &format!("{width}px")).unwrap();
    style.set_property("zoom", zoom).unwrap();
    let _rows = rows(&parent, view);
    let color = |role: &str| rgb(presentation.colors[role]);
    let card = one(&parent, ".maestro-tool-message-card");
    assert_eq!(computed(&card, "border-top-color"), color("border"));
    assert_eq!(computed(&card, "background-color"), color("panel"));
    assert_eq!(
        computed(&one(&parent, ".maestro-tool-caption"), "color"),
        color("muted")
    );
    assert_eq!(
        computed(&one(&parent, ".maestro-thinking-header"), "color"),
        color("muted")
    );
    assert_eq!(
        computed(&one(&parent, ".maestro-message-error"), "color"),
        color("error")
    );
    assert_eq!(
        computed(&one(&parent, ".maestro-message-abort"), "color"),
        color("error")
    );
    let right = parent.get_bounding_client_rect().right();
    for selector in [
        ".maestro-user-bubble",
        ".maestro-tool-message-card",
        ".maestro-message-abort",
    ] {
        let part = one(&parent, selector).get_bounding_client_rect();
        assert!(part.height() > 0.0, "{selector} paints");
        assert!(
            part.right() <= right + 0.5,
            "{selector} stays within {width}px"
        );
    }
    for words in [
        "user words",
        "assistant words",
        "listing words",
        "debug words",
        "failed words",
    ] {
        assert!(text(&parent).contains(words), "{words}");
    }
    parent.remove();
}
/// Tab and Shift-Tab walk the same native controls, offscreen text stays findable, and a control
/// revealed when a collapse clamps the scroll position is usable at once.
pub(super) async fn keyboard_and_scroll(view: &DomView, page: &JsValue) {
    let before = button("before");
    let parent = container();
    let style = parent.unchecked_ref::<HtmlElement>().style();
    style.set_property("height", "240px").unwrap();
    style.set_property("overflow", "auto").unwrap();
    let after = button("after");
    let _rows = rows(&parent, view);
    let document = body().owner_document().unwrap();
    before.focus().unwrap();
    let mut forward = Vec::new();
    loop {
        driver("driverPress", "Tab").await;
        let active = document.active_element().unwrap();
        if active.is_same_node(Some(&after)) {
            break;
        }
        assert!(
            parent.contains(Some(&active)),
            "Tab stays in the transcript"
        );
        assert!(
            forward
                .iter()
                .all(|seen: &Element| !seen.is_same_node(Some(&active)))
        );
        forward.push(active);
    }
    assert!(!forward.is_empty(), "the transcript has native controls");
    for expected in forward.iter().rev() {
        driver("driverPress", "Shift+Tab").await;
        assert!(
            document
                .active_element()
                .unwrap()
                .is_same_node(Some(expected))
        );
    }
    driver("driverPress", "Shift+Tab").await;
    assert!(
        document
            .active_element()
            .unwrap()
            .is_same_node(Some(&before))
    );
    clamped_controls(&parent, page).await;
    let element: &HtmlElement = parent.unchecked_ref();
    element.set_scroll_top(element.scroll_height());
    let found = call(page, "find", &js_sys::Array::of1(&"user words".into())).await;
    assert_eq!(found.as_bool(), Some(true), "offscreen text stays findable");
    for element in [
        before.unchecked_into::<Element>(),
        parent,
        after.unchecked_into(),
    ] {
        element.remove();
    }
}
/// Expand the execution card, scroll to its end, collapse it by keyboard, and click the control
/// the clamped scroll position reveals.
async fn clamped_controls(parent: &Element, page: &JsValue) {
    let scroller: &HtmlElement = parent.unchecked_ref();
    let headers = parent.query_selector_all(".maestro-tool-toggle").unwrap();
    let header: HtmlElement = headers.item(headers.length() - 1).unwrap().unchecked_into();
    header.click();
    let disclosure = header.next_element_sibling().unwrap();
    call(page, "finish", &js_sys::Array::of1(&disclosure)).await;
    scroller.set_scroll_top(scroller.scroll_height());
    let bottom = scroller.scroll_top();
    crate::contributions_support::disclosure(&header)
        .unwrap()
        .focus()
        .unwrap();
    driver("driverPress", "Enter").await;
    call(page, "finish", &js_sys::Array::of1(&disclosure)).await;
    let limit = scroller.scroll_height() - scroller.client_height();
    assert!(
        scroller.scroll_top() <= limit,
        "the collapse clamps the scroll position"
    );
    assert!(scroller.scroll_top() < bottom || bottom <= limit);
    let thinking: HtmlElement = one(parent, ".maestro-thinking-header").unchecked_into();
    thinking.scroll_into_view();
    let rect = thinking.get_bounding_client_rect();
    let point = format!(
        "[{}, {}, true]",
        rect.x() + rect.width() / 2.0,
        rect.y() + rect.height() / 2.0
    );
    driver("driverPointer", &point).await;
    assert_eq!(
        thinking.get_attribute("aria-expanded").as_deref(),
        Some("true")
    );
}
/// A focusable page control outside the transcript.
fn button(label: &str) -> HtmlElement {
    let button: HtmlElement = body()
        .owner_document()
        .unwrap()
        .create_element("button")
        .unwrap()
        .unchecked_into();
    button.set_text_content(Some(label));
    body().append_child(&button).unwrap();
    button
}
