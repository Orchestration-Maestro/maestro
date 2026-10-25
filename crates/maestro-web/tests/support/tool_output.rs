//! Assertions for text-only rendering fixtures that cannot open attachments.
#![cfg(test)]
use maestro_web::tools::types::{ToolContentOutput, ToolUiError};

/// Observe that a fixture has no independently opened effects before its outcome.
pub trait TextOnlyOutput {
    /// Assert the fixture's empty overlay contract without hiding a drawing failure.
    /// # Errors
    /// Returns the original drawing failure unchanged.
    fn assert_no_overlays(self) -> Result<egui::Response, ToolUiError>;
}
impl TextOnlyOutput for ToolContentOutput {
    fn assert_no_overlays(self) -> Result<egui::Response, ToolUiError> {
        assert!(
            self.overlays.is_empty(),
            "text-only fixture opened an attachment"
        );
        self.response
    }
}
