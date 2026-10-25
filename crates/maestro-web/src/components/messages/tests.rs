//! Owned user operands retain their original attachment handles.
#![cfg(test)]
use super::dom::UserMessage;
use maestro_app::{AgentMessage, browser::components::messages::CustomAgentMessages};
use std::sync::{Arc, RwLock};

#[test]
fn user_owned_operand_keeps_content_and_attachment_handles() {
    let attachment = Arc::new(RwLock::new(crate::Attachment {
        id: "original".into(),
        r#type: maestro_app::browser::utils::attachment_utils::AttachmentType::Document,
        file_name: "note.txt".into(),
        mime_type: "text/plain".into(),
        size: 1,
        content: "YQ==".into(),
        extracted_text: None,
        preview: None,
    }));
    let message = AgentMessage::Custom(Arc::new(RwLock::new(
        CustomAgentMessages::UserWithAttachments(
            maestro_app::browser::components::messages::UserMessageWithAttachments {
                content: maestro_app::UserContent::Text("  kept text  ".into()),
                timestamp: 0.,
                attachments: Some(vec![Arc::clone(&attachment), Arc::clone(&attachment)]),
            },
        ),
    )));
    let mut row = UserMessage::<maestro_app::NoCustomMessages>::default();
    assert!(row.operand().is_err());
    row.message = Some(message);
    let operand = row.operand().unwrap();
    drop(row);
    assert_eq!(operand.text, "  kept text  ");
    assert_eq!(operand.attachments.len(), 2);
    assert!(
        operand
            .attachments
            .iter()
            .all(|handle| Arc::ptr_eq(handle, &attachment))
    );
    attachment.write().unwrap().file_name = "edited.txt".into();
    assert_eq!(
        operand.attachments[1].read().unwrap().file_name,
        "edited.txt"
    );
}
