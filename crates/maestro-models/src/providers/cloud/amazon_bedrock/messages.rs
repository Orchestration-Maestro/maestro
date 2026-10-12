//! Conversion of conversation text into the service's turn and system blocks.

use super::request::{Message, TextBlock};
use crate::{Context, Message as Turn, UserBlock, UserContent};

/// The user turns of the conversation as text-block messages, in order.
pub(super) fn user_messages(context: &Context) -> Vec<Message> {
    context
        .messages
        .iter()
        .filter_map(|turn| match turn {
            Turn::User(user) => Some(Message {
                role: Some("user".to_owned()),
                content: Some(match &user.content {
                    UserContent::Text(text) => vec![block(text)],
                    UserContent::Blocks(blocks) => blocks.iter().filter_map(text_block).collect(),
                }),
            }),
            _ => None,
        })
        .collect()
}

/// The system prompt as one text block, or none when it is absent or empty.
pub(super) fn system_blocks(context: &Context) -> Option<Vec<TextBlock>> {
    context
        .system_prompt
        .as_deref()
        .filter(|prompt| !prompt.is_empty())
        .map(|prompt| vec![block(prompt)])
}

/// A text block holding `text` as written.
fn block(text: &str) -> TextBlock {
    TextBlock {
        text: Some(text.to_owned()),
    }
}

/// A text block for a text member of user content; other members have no text.
fn text_block(member: &UserBlock) -> Option<TextBlock> {
    match member {
        UserBlock::Text(text) => Some(block(&text.text)),
        UserBlock::Image(_) => None,
    }
}
