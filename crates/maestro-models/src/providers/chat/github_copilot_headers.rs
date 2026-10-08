//! Request headers the Copilot gateway expects.

use indexmap::IndexMap;

use crate::{Message, UserBlock, UserContent};

/// Classify a request as user-initiated or as an agent follow-up to the last message.
#[must_use]
pub fn infer_copilot_initiator(messages: &[Message]) -> &'static str {
    match messages.last() {
        Some(Message::Assistant(_) | Message::ToolResult(_)) => "agent",
        Some(Message::User(_)) | None => "user",
    }
}

/// Report whether any user or tool-result message carries an image block.
#[must_use]
pub fn has_copilot_vision_input(messages: &[Message]) -> bool {
    let has_image = |blocks: &[UserBlock]| blocks.iter().any(|b| matches!(b, UserBlock::Image(_)));
    messages.iter().any(|message| match message {
        Message::User(user) => match &user.content {
            UserContent::Blocks(blocks) => has_image(blocks),
            UserContent::Text(_) => false,
        },
        Message::ToolResult(result) => has_image(&result.content),
        Message::Assistant(_) => false,
    })
}

/// Build the initiator and intent headers, adding the vision marker when requested.
#[must_use]
pub fn build_copilot_dynamic_headers(
    messages: &[Message],
    has_images: bool,
) -> IndexMap<String, String> {
    let mut headers = IndexMap::from([
        (
            "X-Initiator".to_owned(),
            infer_copilot_initiator(messages).to_owned(),
        ),
        ("Openai-Intent".to_owned(), "conversation-edits".to_owned()),
    ]);
    if has_images {
        headers.insert("Copilot-Vision-Request".to_owned(), "true".to_owned());
    }
    headers
}
