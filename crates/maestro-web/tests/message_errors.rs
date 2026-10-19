//! Message composition preserves reached operation failures and their original causes.
#![cfg(test)]
use crate::support::tool_output::TextOnlyOutput as _;
use crate::{attachment_support, effects_support};

use maestro_web::{
    AbortedMessage, AssistantMessage, AssistantMessageProps, UserMessage, UserMessageInput,
    browser::attachment_content::{AttachmentHost, AttachmentHostError},
    components::code_block::ReadingUiError,
    dialogs::attachment_overlay::AttachmentUiError,
    tools::types::{ToolRenderView, ToolUiError},
};
use std::{error::Error, rc::Rc, sync::Arc};

use crate::failing_highlighter::{Cause, FailingEngine};

#[test]
fn message_failures_keep_localization_attachment_and_reading_sources() {
    let mut frame = attachment_support::Frame::default();
    let effects = Rc::new(effects_support::Host::default());
    let host: Rc<dyn AttachmentHost> = effects.clone();
    let marker = Arc::new(());
    let engine = FailingEngine(marker.clone());
    let mut aborted = AbortedMessage::new(host.clone());
    let dictionaries = frame.i18n.get_translations();
    let borrow = dictionaries.borrow_mut();
    let error = render(&mut frame, &host, &engine, |ui, view| {
        aborted.render(ui, view)
    })
    .unwrap_err();
    assert!(matches!(
        error,
        ToolUiError::I18n(maestro_web::I18nError::BorrowConflict)
    ));
    assert!(error.source().unwrap().is::<maestro_web::I18nError>());
    drop(borrow);
    *effects.failure.borrow_mut() = Some(AttachmentHostError::Host("native failure".into()));
    let error = render(&mut frame, &host, &engine, |ui, view| {
        aborted.render(ui, view)
    })
    .unwrap_err();
    assert!(matches!(error, ToolUiError::Host(_)));
    assert_eq!(error.source().unwrap().to_string(), "native failure");
    attachment_failure(&mut frame, &host, &engine);
    let mut assistant = AssistantMessage::new(host.clone());
    let message = serde_json::from_value(serde_json::json!({
        "role":"assistant","content":[{"type":"text","text":"```js\nconst value = 1;\n```"}],
        "api":"openai-completions","provider":"openai","model":"controlled","timestamp":0,"stopReason":"stop",
        "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}
    })).unwrap();
    let error = render(&mut frame, &host, &engine, |ui, view| {
        assistant.render::<serde_json::Value>(ui, &message, AssistantMessageProps::default(), view)
    })
    .assert_no_overlays()
    .unwrap_err();
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

/// Reach the tile's own original-record admission failure through a user row.
fn attachment_failure(
    frame: &mut attachment_support::Frame,
    host: &Rc<dyn AttachmentHost>,
    engine: &FailingEngine,
) {
    let attachments = [attachment_support::attachment()];
    let borrow = attachments[0].borrow_mut();
    let content = maestro_app::UserContent::Text("native text".into());
    let mut user = UserMessage::new(host.clone());
    let error = render(frame, host, engine, |ui, view| {
        user.render(
            ui,
            UserMessageInput::WithAttachments {
                content: &content,
                attachments: Some(&attachments),
            },
            view,
        )
    })
    .assert_no_overlays()
    .err()
    .unwrap();
    assert!(matches!(
        error,
        ToolUiError::Attachment(AttachmentUiError::BorrowConflict)
    ));
    assert!(error.source().unwrap().is::<AttachmentUiError>());
    drop(borrow);
}

/// Borrow the shared native frame without retaining the caller's operation operands.
fn render<R>(
    frame: &mut attachment_support::Frame,
    host: &Rc<dyn AttachmentHost>,
    engine: &FailingEngine,
    draw: impl FnOnce(&mut egui::Ui, ToolRenderView<'_>) -> R,
) -> R {
    frame.run(vec![], |ctx, i18n, presentation| {
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("message-errors"),
            egui::UiBuilder::new().max_rect(ctx.content_rect()),
        );
        draw(
            &mut ui,
            ToolRenderView {
                host,
                i18n,
                presentation,
                highlighter: engine,
                modal: None,
            },
        )
    })
}
