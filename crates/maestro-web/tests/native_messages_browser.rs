//! User rows mounted directly in the page with original application operands.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{body, view};
use maestro_app::{
    AgentMessage, NoCustomMessages, browser::components::messages::CustomAgentMessages,
};
use maestro_web::{components::messages::UserMessage, dom::DomContent as _};
use wasm_bindgen::{JsCast as _, prelude::wasm_bindgen};
use web_sys::{Element, HtmlElement};

mod assistant;
mod rows;
mod transcript;

/// Composed rows paint through brand roles in both packs and modes at narrow and wide widths,
/// keep offscreen text findable and keep native controls usable by keyboard and after a
/// collapse clamps the scroll position.
#[wasm_bindgen]
pub async fn transcript_parts_dom_use_brand_roles_layout_and_native_focus(
    import_module: &js_sys::Function,
) {
    let view = view();
    let _elements = crate::contributions_support::host(&view, import_module);
    let page = crate::contributions_support::page_module(import_module).await;
    transcript::paint(&view);
    transcript::keyboard_and_scroll(&view, &page).await;
}

/// Assistant admission, order and original tool/result operands through native children.
#[wasm_bindgen]
pub fn assistant_dom_preserves_order_visibility_and_emitted_positions(
    import_module: &js_sys::Function,
) {
    let view = view();
    let _elements = crate::contributions_support::host(&view, import_module);
    assistant::visibility(&view, &rows::fixture());
}

/// Usage and localized outcomes remain independent of streaming and use the current action.
#[wasm_bindgen]
pub fn assistant_dom_usage_outcomes_current_action_and_localization(
    import_module: &js_sys::Function,
) {
    let view = view();
    let _elements = crate::contributions_support::host(&view, import_module);
    assistant::outcomes(&view, &rows::fixture());
}

/// Tool rows pass original arguments and result envelopes to the selected renderer, let an abort
/// replace the result and end streaming, and card only default content.
#[wasm_bindgen]
pub fn tool_dom_preserves_original_operands_abort_and_card_choice(
    import_module: &js_sys::Function,
) {
    let view = view();
    let _elements = crate::contributions_support::host(&view, import_module);
    let fixture = rows::fixture();
    rows::recorded_combinations(&view, &fixture);
    rows::selection_and_card(&view, &fixture);
}

/// Debug listings replay every recorded locale, details, value and output case literally.
#[wasm_bindgen]
pub fn debug_dom_keeps_strict_pretty_missing_null_and_literal_code(
    import_module: &js_sys::Function,
) {
    let view = view();
    let _elements = crate::contributions_support::host(&view, import_module);
    let fixture = rows::fixture();
    rows::debug_cases(&view, &fixture);
    rows::pretty_and_output_cases(&view, &fixture);
}

/// Every delivered row keeps its containing root through updates and reconnection, keeps its last
/// published content when a candidate fails and releases its children when dropped.
#[wasm_bindgen]
pub async fn message_dom_reconnect_update_and_drop_keep_independent_roots(
    import_module: &js_sys::Function,
) {
    let view = view();
    let _elements = crate::contributions_support::host(&view, import_module);
    let page = crate::contributions_support::page_module(import_module).await;
    let fixture = rows::fixture();
    rows::tool_row_lifecycle(&view, &fixture, &page).await;
    rows::debug_row_lifecycle(&view);
    rows::abort_row(&view, &fixture);
    user_row_lifecycle(&view);
    assistant::lifecycle(&view, &fixture);
}

/// A user row keeps its root and last text when its required message goes missing.
fn user_row_lifecycle(view: &maestro_web::dom::DomView) {
    let mut row = UserMessage::<NoCustomMessages> {
        message: Some(
            serde_json::from_value(
                serde_json::json!({"role":"user", "content":"kept text", "timestamp":0}),
            )
            .unwrap(),
        ),
    };
    let root = row.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let node: &Element = root.node.unchecked_ref();
    node.remove();
    body().append_child(&root.node).unwrap();
    row.update(&root, view).unwrap();
    assert_eq!(node.text_content().unwrap().trim(), "kept text");
    row.message = None;
    assert!(row.update(&root, view).is_err());
    assert_eq!(node.text_content().unwrap().trim(), "kept text");
    let other = UserMessage::<NoCustomMessages>::default();
    assert!(other.mount(view).is_err());
    node.remove();
}

/// Source-derived user selection with an open original input record.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Application input, including ordinary-user extra fields.
    input: serde_json::Value,
    /// Selected first source text.
    text: String,
    /// Ordered original attachment names.
    attachments: Vec<String>,
}
/// First text selection and shared attachment identity through the real overlay consumer.
#[wasm_bindgen]
pub fn user_dom_selects_first_text_and_preserves_attachment_handles() {
    let view = view();
    let mut row = UserMessage::<NoCustomMessages>::default();
    assert!(row.mount(&view).is_err());
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/native_user_rows.json")).unwrap();
    assert_eq!(cases.len(), 19);
    for case in cases {
        user_case(&view, &mut row, case);
    }
    optional_attachments(&view, &mut row);
    // Indentation is source content, not whitespace removed before the Markdown consumer.
    row.message = Some(
        serde_json::from_value(
            serde_json::json!({"role":"user", "content":"    literal indentation", "timestamp":0}),
        )
        .unwrap(),
    );
    let root = row.mount(&view).unwrap();
    let node: Element = root.node.unchecked_into();
    assert_eq!(
        node.query_selector("code")
            .unwrap()
            .unwrap()
            .text_content()
            .unwrap(),
        "literal indentation\n"
    );
}

/// Observe one source selection and every original attachment at its consuming overlay.
fn user_case(view: &maestro_web::dom::DomView, row: &mut UserMessage, case: Case) {
    row.message = Some(serde_json::from_value(case.input).unwrap());
    let attachments = match row.message.as_ref().unwrap() {
        AgentMessage::Custom(message) => match &*message.read().unwrap() {
            CustomAgentMessages::UserWithAttachments(message) => {
                message.attachments.clone().unwrap_or_default()
            }
            _ => unreachable!(),
        },
        _ => vec![],
    };
    let root = row.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let node: Element = root.node.unchecked_into();
    let markdown = node.query_selector(".maestro-markdown").unwrap().unwrap();
    assert_eq!(markdown.text_content().unwrap().trim(), case.text);
    let tiles = node.query_selector_all("[data-open]").unwrap();
    assert_eq!(tiles.length() as usize, case.attachments.len());
    for (index, (attachment, expected)) in attachments.iter().zip(case.attachments).enumerate() {
        let target: HtmlElement = tiles
            .item(u32::try_from(index).unwrap())
            .unwrap()
            .unchecked_into();
        assert_eq!(target.title(), expected);
        attachment.write().unwrap().file_name = format!("current-{index}.txt");
        target.click();
        let overlay = view
            .document
            .query_selector("attachment-overlay")
            .unwrap()
            .unwrap();
        assert_eq!(
            overlay
                .query_selector(".maestro-attachment-name")
                .unwrap()
                .unwrap()
                .text_content()
                .unwrap(),
            format!("current-{index}.txt")
        );
        overlay
            .query_selector("[data-close]")
            .unwrap()
            .unwrap()
            .unchecked_into::<HtmlElement>()
            .click();
    }
    node.remove();
}

/// Missing operands and repeated attachment entries remain distinct from an ordinary user row.
fn optional_attachments(view: &maestro_web::dom::DomView, row: &mut UserMessage) {
    let attachment = serde_json::json!({
        "id": "repeated", "type": "document", "fileName": "notes.txt",
        "content": "", "mimeType": "text/plain", "size": 0
    });
    for attachments in [None, Some(vec![attachment.clone(), attachment])] {
        let names = if attachments.is_some() {
            vec!["notes.txt".into(); 2]
        } else {
            vec![]
        };
        let mut input = serde_json::json!({"role": "user-with-attachments", "content": "first", "timestamp": 0});
        if let Some(attachments) = attachments {
            input["attachments"] = attachments.into();
        }
        user_case(
            view,
            row,
            Case {
                input,
                text: "first".into(),
                attachments: names,
            },
        );
    }
}
