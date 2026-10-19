//! Retained transcript message rows.
mod aborted_message;
mod assistant_message;
mod tool_message;
mod tool_message_debug_view;
mod user_message;
pub use aborted_message::AbortedMessage;
pub use assistant_message::{AssistantMessage, AssistantMessageProps};
pub use tool_message::{ToolMessage, ToolMessageProps};
pub use tool_message_debug_view::ToolMessageDebugView;
pub use user_message::{UserMessage, UserMessageInput};

use crate::{
    components::code_block::CodeBlockView,
    tools::types::{ToolRenderView, ToolUiError},
};

/// Project message presentation into the existing reading widgets.
pub(super) fn reading_view(
    view: ToolRenderView<'_>,
) -> CodeBlockView<'_, crate::TranslationFunction> {
    CodeBlockView {
        i18n: view.i18n,
        presentation: view.presentation,
        highlighter: view.highlighter,
        modal: view.modal,
    }
}

/// Resolve the selected presentation's canvas color.
fn color(view: ToolRenderView<'_>, role: &str) -> Result<egui::Color32, ToolUiError> {
    crate::browser::attachment_content::color(view.presentation, role)
        .map_err(|error| ToolUiError::Presentation(error.to_string()))
}
