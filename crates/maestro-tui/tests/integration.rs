//! Shared integration-test executable.
#![cfg(test)]

#[path = "binding_support/mod.rs"]
mod binding_support;
#[path = "fixtures/completion_rig.rs"]
mod completion_rig;
#[path = "fixtures/editor_support/host.rs"]
mod editor_host;
#[path = "fixtures/editor_support/mod.rs"]
mod editor_support;
#[path = "image_support/mod.rs"]
mod image_support;
#[path = "fixtures/input_support/mod.rs"]
mod input_support;
#[path = "terminal_support/mod.rs"]
mod terminal_support;
mod fixtures {
    pub mod attachments;
    pub mod cluster_cases;
    pub mod completion;
    pub mod completion_native;
    pub mod editors;
    pub mod futures;
    pub mod native_attachments;
    pub mod pending_attachments;
    pub mod providers;
    pub mod slice_cases;
    pub mod terminal;
    pub mod terminals;
    pub mod truncated_cases;
    pub mod truncation_cases;
    pub mod widget_probes;
    pub mod widgets;
}
mod support {
    pub mod key_cases;
}
mod ansi_columns;
mod ansi_truncation;
mod ansi_wrapping;
mod attachment_completion;
mod binding_registry;
mod binding_rules;
mod command_completion;
mod components;
mod editing_history;
mod editor_callbacks;
mod editor_completion;
mod editor_completion_lifecycle;
mod editor_drawing;
mod editor_editing;
mod editor_history;
mod editor_kill;
mod editor_markers;
mod editor_navigation;
mod editor_paste;
mod editor_wrapping;
mod fuzzy_ranking;
mod image_components;
mod input_editing;
mod input_viewport;
mod key_decoding;
mod list_interactions;
pub mod list_support;
mod native_attachment_completion;
mod native_completion;
mod path_completion;
mod selection_lists;
mod settings_panel;
mod stdin_sequences;
mod terminal_images;
mod text_boundaries;
mod text_width;
mod widget_rendering;
mod widget_state;
