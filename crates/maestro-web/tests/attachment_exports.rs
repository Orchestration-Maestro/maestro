//! Browser root reaches application-owned attachment policy.
#![cfg(test)]
use maestro_app::browser::components::records::{ArtifactAction, CustomAgentMessages};
use maestro_app::browser::utils::attachments::{AttachmentSource, AttachmentType};
use maestro_web::{
    ArtifactMessage, Attachment, UserMessageWithAttachments, convert_attachments,
    default_convert_to_llm, is_artifact_message, is_user_message_with_attachments, load_attachment,
};
use std::sync::{Arc, RwLock};

#[test]
fn web_attachment_exports_use_application_records() {
    let attachment: Attachment = maestro_app::browser::utils::attachments::Attachment {
        id: "id".into(),
        r#type: AttachmentType::Image,
        file_name: "name".into(),
        mime_type: "image/png".into(),
        size: 0,
        content: String::new(),
        extracted_text: None,
        preview: Some(String::new()),
    };
    let attachment = Arc::new(RwLock::new(attachment));
    let mut user = UserMessageWithAttachments {
        content: maestro_models::UserContent::Text("hello".into()),
        timestamp: 1.0,
        attachments: Some(vec![Arc::clone(&attachment)]),
    };
    check_shared_attachment(&mut user, &attachment);
    let artifact = ArtifactMessage {
        action: ArtifactAction::Create,
        filename: "made.txt".into(),
        content: None,
        title: None,
        timestamp: String::new(),
    };
    let history: Vec<maestro_agent::AgentMessage<CustomAgentMessages>> = vec![
        maestro_agent::AgentMessage::Custom(Arc::new(RwLock::new(
            CustomAgentMessages::UserWithAttachments(user),
        ))),
        maestro_agent::AgentMessage::Custom(Arc::new(RwLock::new(CustomAgentMessages::Artifact(
            artifact,
        )))),
    ];
    assert!(is_user_message_with_attachments(&history[0]));
    assert!(is_artifact_message(&history[1]));
    let text: maestro_models::TextContent = maestro_models::TextContent::new("hello".into());
    let image: maestro_models::ImageContent = maestro_models::ImageContent {
        data: String::new(),
        mime_type: "image/png".into(),
    };
    let blocks: Vec<maestro_models::UserBlock> = vec![
        maestro_models::UserBlock::Text(text),
        maestro_models::UserBlock::Image(image.clone()),
    ];
    let converted: Vec<maestro_models::Message> = default_convert_to_llm(&history);
    let [maestro_models::Message::User(user)] = converted.as_slice() else {
        panic!("expected one user message");
    };
    assert_eq!(user.content, maestro_models::UserContent::Blocks(blocks));
    assert_eq!(user.timestamp.to_bits(), 1.0_f64.to_bits());
    assert_eq!(
        convert_attachments(&[attachment]),
        vec![maestro_models::UserBlock::Image(image)]
    );
    assert!(convert_attachments(&[]).is_empty());
    let _: maestro_models::Fetch = maestro_models::default_fetch();
    let error: maestro_models::FetchError = maestro_models::FetchError::Aborted;
    assert_eq!(error.to_string(), "Request was aborted.");
}

/// Application admission and the tile retain the same original attachment owner.
fn check_shared_attachment(
    user: &mut UserMessageWithAttachments,
    attachment: &maestro_web::SharedAttachment,
) {
    let retained = Arc::clone(&user.attachments.take().unwrap()[0]);
    let tile = maestro_web::AttachmentTile::from(retained);
    assert!(Arc::ptr_eq(&tile.attachment, attachment));
    drop(tile);
    user.attachments = Some(vec![Arc::clone(attachment)]);
}

#[test]
fn web_local_attachment_loading_does_not_fetch() {
    let fetch: maestro_models::Fetch = Arc::new(|_| panic!("local input fetched"));
    let mut future = std::pin::pin!(load_attachment(
        AttachmentSource::Buffer(b"hi"),
        Some("note.txt"),
        &fetch
    ));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    let std::task::Poll::Ready(Ok(loaded)) =
        std::future::Future::poll(future.as_mut(), &mut context)
    else {
        panic!("local load did not complete");
    };
    assert_eq!(loaded.extracted_text.as_deref(), Some("hi"));
}

#[test]
fn web_pdf_loading_uses_application_owner() {
    let bytes = include_bytes!("../../maestro-app/tests/fixtures/pdf/synthetic-wide.pdf");
    let fetch: maestro_models::Fetch = Arc::new(|_| panic!("local input fetched"));
    let mut future = std::pin::pin!(load_attachment(
        AttachmentSource::Buffer(bytes),
        Some("page.pdf"),
        &fetch
    ));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    let std::task::Poll::Ready(Ok(loaded)) =
        std::future::Future::poll(future.as_mut(), &mut context)
    else {
        panic!("local load did not complete");
    };
    assert_eq!(
        loaded.extracted_text.as_deref(),
        Some("<pdf filename=\"page.pdf\">\n<page number=\"1\">\nPage A\n</page>\n</pdf>")
    );
    assert!(loaded.preview.is_some_and(|preview| !preview.is_empty()));
}
