//! Assembly of the first message from stdin, file text and authored prompts.

use super::args::Args;
use maestro_app::ImageContent;

/// The inputs combined into the first message.
pub struct InitialMessageInput<'a> {
    /// Parsed arguments whose first prompt is consumed.
    pub parsed: &'a mut Args,
    /// Prepared text of the authored file arguments.
    pub file_text: Option<&'a str>,
    /// Prepared image attachments in their authored order.
    pub file_images: Vec<ImageContent>,
    /// Content read from standard input, or absence when none was supplied.
    pub stdin_content: Option<&'a str>,
}

/// The first message and its attachments, each absent when nothing contributed.
pub struct InitialMessageResult {
    /// Concatenated text of every supplied part.
    pub initial_message: Option<String>,
    /// Nonempty image attachments.
    pub initial_images: Option<Vec<ImageContent>>,
}

/// Combine the supplied parts and consume exactly the first queued prompt.
#[must_use]
pub fn build_initial_message(input: InitialMessageInput<'_>) -> InitialMessageResult {
    let first = input.parsed.messages.pop_front();
    let mut parts: Vec<&str> = Vec::with_capacity(3);
    if let Some(stdin) = input.stdin_content {
        parts.push(stdin);
    }
    if let Some(text) = input.file_text.filter(|text| !text.is_empty()) {
        parts.push(text);
    }
    if let Some(first) = first.as_deref() {
        parts.push(first);
    }
    InitialMessageResult {
        initial_message: (!parts.is_empty()).then(|| parts.concat()),
        initial_images: (!input.file_images.is_empty()).then_some(input.file_images),
    }
}
