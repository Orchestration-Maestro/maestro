//! Message composition keeps reached localization, attachment and highlighting causes.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    contributions_support::view,
    failing_highlighter::{Cause, FailingEngine},
};
use maestro_web::{
    AbortedMessage, AssistantMessage, UserMessage,
    components::code_block::ReadingUiError,
    dialogs::attachment_overlay::AttachmentUiError,
    dom::{DomContent, DomView},
    tools::types::ToolUiError,
};
use std::{
    error::Error as _,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::wasm_bindgen;

/// Native row preparation returns the original typed leaf error and cause identity.
#[wasm_bindgen]
pub fn message_failures_keep_localization_attachment_and_reading_sources(
    import_module: &js_sys::Function,
) {
    let view = view();
    tool_failure(&view, import_module);
    let marker = Arc::new(());
    let view = DomView {
        highlighter: Rc::new(FailingEngine(marker.clone())),
        ..(*view).clone()
    };
    let dictionaries = view.i18n.get_translations();
    let borrow = dictionaries.borrow_mut();
    let error = AbortedMessage.mount(&view).err().unwrap();
    assert!(matches!(
        error,
        ToolUiError::I18n(maestro_web::I18nError::BorrowConflict)
    ));
    assert!(error.source().unwrap().is::<maestro_web::I18nError>());
    drop(borrow);
    attachment_failure(&view);
    let message = serde_json::from_value(serde_json::json!({
        "role":"assistant","content":[{"type":"text","text":"```js\nconst value = 1;\n```"}],
        "api":"openai-completions","provider":"openai","model":"controlled","timestamp":0,"stopReason":"stop",
        "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}
    })).unwrap();
    let assistant = AssistantMessage::<serde_json::Value> {
        message: Some(Arc::new(RwLock::new(message))),
        ..Default::default()
    };
    let error = assistant.mount(&view).err().unwrap();
    assert!(matches!(
        error,
        ToolUiError::Reading(ReadingUiError::Highlight(_))
    ));
    let cause = error
        .source()
        .unwrap()
        .source()
        .unwrap()
        .downcast_ref::<Cause>()
        .unwrap();
    assert!(Arc::ptr_eq(&cause.0, &marker));
    assert_eq!(error.to_string(), "original syntax cause");
}

/// A reached tile reads its original attachment rather than a detached copy.
fn attachment_failure(view: &DomView) {
    use maestro_app::{
        AgentMessage, NoCustomMessages, browser::components::messages::CustomAgentMessages,
    };
    let record = Arc::new(RwLock::new(maestro_web::Attachment {
        id: "original".into(),
        r#type: maestro_app::browser::utils::attachment_utils::AttachmentType::Document,
        file_name: "note.txt".into(),
        mime_type: "text/plain".into(),
        size: 3,
        content: "QUJD".into(),
        extracted_text: None,
        preview: None,
    }));
    let user = UserMessage::<NoCustomMessages> {
        message: Some(AgentMessage::Custom(Arc::new(RwLock::new(
            CustomAgentMessages::UserWithAttachments(maestro_web::UserMessageWithAttachments {
                content: maestro_app::UserContent::Text("native text".into()),
                attachments: Some(vec![record.clone()]),
                timestamp: 0.,
            }),
        )))),
    };
    let borrow = record.write().unwrap();
    let error = user.mount(view).err().unwrap();
    assert!(matches!(
        error,
        ToolUiError::Attachment(AttachmentUiError::BorrowConflict)
    ));
    assert!(error.source().unwrap().is::<AttachmentUiError>());
    drop(borrow);
}

/// Default tool mounting returns the selected syntax failure rather than publishing an empty listing.
fn tool_failure(view: &DomView, import_module: &js_sys::Function) {
    use maestro_web::tools::types::ToolParams;
    let highlighter = Rc::new(crate::text_image_support::Highlighter::default());
    highlighter.fail.set(true);
    let view = Rc::new(DomView {
        highlighter: highlighter.clone(),
        ..view.clone()
    });
    let _host = crate::contributions_support::host(&view, import_module);
    let params = serde_json::json!({"value": 1});
    let prepared = maestro_web::render_tool::<serde_json::Value, serde_json::Value>(
        "unregistered-operation-error",
        Some(ToolParams::json(&params)),
        None,
        false,
    )
    .unwrap();
    let error = prepared
        .content
        .mount(&view)
        .err()
        .expect("the reached syntax failure must propagate");
    assert!(matches!(
        error,
        ToolUiError::Reading(ReadingUiError::Highlight(_))
    ));
    let reading = error
        .source()
        .unwrap()
        .downcast_ref::<ReadingUiError>()
        .unwrap();
    let cause = reading
        .source()
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(cause.to_string(), "engine failure");
    assert_eq!(error.to_string(), cause.to_string());
    assert_eq!(
        &*highlighter.requests.borrow(),
        &[("{\n  \"value\": 1\n}".to_owned(), "json".to_owned())]
    );
}
