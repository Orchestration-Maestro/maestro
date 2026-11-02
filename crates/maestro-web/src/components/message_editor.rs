//! Native message composer: text, attachments, reasoning level, model choice and send or abort.
use crate::SharedAttachment;
use maestro_app::{Model, ThinkingLevel};
use std::rc::Rc;

/// File chooser hint; dropped and pasted files are not filtered by it.
pub const ACCEPTED_TYPES: &str = "image/*,application/pdf,.docx,.pptx,.xlsx,.xls,.txt,.md,.json,.xml,.html,.css,.js,.ts,.jsx,.tsx,.yml,.yaml";
/// Default attachment count limit.
pub const MAX_FILES: usize = 10;
/// Default per-file size limit in bytes (20 MiB).
pub const MAX_FILE_SIZE: usize = 20 * 1024 * 1024;
/// The reasoning levels the editor offers, in menu order.
pub const THINKING_LEVELS: [ThinkingLevel; 5] = [
    ThinkingLevel::Off,
    ThinkingLevel::Minimal,
    ThinkingLevel::Low,
    ThinkingLevel::Medium,
    ThinkingLevel::High,
];

/// Callback with the edited text.
pub type TextCallback = Rc<dyn Fn(String)>;
/// Callback with the text and the attachments to send.
pub type SendCallback = Rc<dyn Fn(String, Vec<SharedAttachment>)>;
/// Callback with the chosen reasoning level.
pub type ThinkingCallback = Rc<dyn Fn(ThinkingLevel)>;
/// Callback with the complete attachment list after a change.
pub type FilesCallback = Rc<dyn Fn(Vec<SharedAttachment>)>;

/// The editor's optional controls, all shown by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorControls {
    /// Show the attachment control.
    pub attachments: bool,
    /// Show the model control while a model is set.
    pub model: bool,
    /// Show the level menu for a reasoning model.
    pub thinking: bool,
}
impl Default for EditorControls {
    fn default() -> Self {
        Self {
            attachments: true,
            model: true,
            thinking: true,
        }
    }
}
/// Editor operands; the callbacks in effect are the most recently supplied ones.
///
/// The text and the attachments belong to the editor once it exists: callers read and replace
/// them through the editor's accessors, so an update never discards an edit.
#[derive(Clone)]
pub struct MessageEditorProps {
    /// Whether a response is streaming: Abort replaces Send and Escape aborts.
    pub is_streaming: bool,
    /// The model named on the model control; a reasoning model also shows the level menu.
    pub current_model: Option<Model>,
    /// The selected reasoning level.
    pub thinking_level: ThinkingLevel,
    /// The optional controls to show.
    pub controls: EditorControls,
    /// Every text edit.
    pub on_input: Option<TextCallback>,
    /// Send with the untrimmed text and the current attachments.
    pub on_send: Option<SendCallback>,
    /// Abort the streaming response.
    pub on_abort: Option<Rc<dyn Fn()>>,
    /// Open model selection; called on the frame after the text field takes focus.
    pub on_model_select: Option<Rc<dyn Fn()>>,
    /// A new reasoning level.
    pub on_thinking_change: Option<ThinkingCallback>,
    /// The attachment list after an addition or a removal.
    pub on_files_change: Option<FilesCallback>,
    /// Most attachments the editor holds.
    pub max_files: usize,
    /// Largest accepted file in bytes.
    pub max_file_size: usize,
    /// File chooser hint.
    pub accepted_types: String,
}
impl Default for MessageEditorProps {
    fn default() -> Self {
        Self {
            is_streaming: false,
            current_model: None,
            thinking_level: ThinkingLevel::Off,
            controls: EditorControls::default(),
            on_input: None,
            on_send: None,
            on_abort: None,
            on_model_select: None,
            on_thinking_change: None,
            on_files_change: None,
            max_files: MAX_FILES,
            max_file_size: MAX_FILE_SIZE,
            accepted_types: ACCEPTED_TYPES.into(),
        }
    }
}

/// Whether a draft can be sent: some non-whitespace text or an attachment, and no file loading.
#[must_use]
pub fn can_send(value: &str, attachments: usize, processing: bool) -> bool {
    !processing && (attachments > 0 || !value.chars().all(maestro_app::is_whitespace_scalar))
}
/// The size limit in whole mebibytes, rounded half up as the alert reports it.
#[must_use]
pub fn limit_megabytes(max_file_size: usize) -> usize {
    const MIB: usize = 1024 * 1024;
    max_file_size / MIB + usize::from(max_file_size % MIB >= MIB / 2)
}
/// The level's stored spelling, also its menu value.
#[must_use]
pub fn level_value(level: &ThinkingLevel) -> &'static str {
    match level {
        ThinkingLevel::Off => "off",
        ThinkingLevel::Minimal => "minimal",
        ThinkingLevel::Low => "low",
        ThinkingLevel::Medium => "medium",
        ThinkingLevel::High => "high",
        ThinkingLevel::Xhigh => "xhigh",
    }
}

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
mod controls;
#[cfg(target_arch = "wasm32")]
mod files;
#[cfg(target_arch = "wasm32")]
pub use browser::MessageEditor;

#[cfg(test)]
mod tests;
