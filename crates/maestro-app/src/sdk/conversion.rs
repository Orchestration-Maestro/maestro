//! The message conversion a session's agent uses, with images withheld when blocked.
use maestro_agent::AgentMessage;
use maestro_models::{Message, TextContent, UserBlock, UserContent};
use maestro_session::messages::{CustomAgentMessages, convert_to_llm};

/// The text that stands in for a withheld image.
const IMAGE_PLACEHOLDER: &str = "Image reading is disabled.";

/// Converts `messages` for the model; when `block_images` is set, every image of a user or
/// tool-result message becomes the placeholder text and a run of placeholders becomes one.
#[must_use]
pub fn convert_to_llm_with_block_images<C: maestro_agent::CustomAgentMessages>(
    messages: &[AgentMessage<CustomAgentMessages<serde_json::Value, C>>],
    block_images: bool,
) -> Vec<Message> {
    let converted = convert_to_llm(messages);
    if !block_images {
        return converted;
    }
    converted
        .into_iter()
        .map(|message| match message {
            Message::User(mut user) => {
                if let UserContent::Blocks(blocks) = &mut user.content {
                    withhold_images(blocks);
                }
                Message::User(user)
            }
            Message::ToolResult(mut result) => {
                withhold_images(&mut result.content);
                Message::ToolResult(result)
            }
            other @ Message::Assistant(_) => other,
        })
        .collect()
}

/// Replaces each image with the placeholder, then collapses consecutive placeholders.
fn withhold_images(blocks: &mut Vec<UserBlock>) {
    if !blocks
        .iter()
        .any(|block| matches!(block, UserBlock::Image(_)))
    {
        return;
    }
    for block in blocks.iter_mut() {
        if matches!(block, UserBlock::Image(_)) {
            *block = UserBlock::Text(TextContent::new(IMAGE_PLACEHOLDER.to_owned()));
        }
    }
    blocks.dedup_by(|block, previous| is_placeholder(block) && is_placeholder(previous));
}

/// Whether `block` is the placeholder text.
fn is_placeholder(block: &UserBlock) -> bool {
    matches!(block, UserBlock::Text(text) if text.text == IMAGE_PLACEHOLDER)
}
