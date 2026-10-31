//! Shared integration-test executable.
#![cfg(test)]
mod application_message_encoding;

mod application_message_records;
mod branch_summary;
mod compaction_context;
mod compaction_preparation;
mod context_projection;
mod converter_binding;
mod cwd_diagnostics;
mod message_projection;
mod session_appends;
mod session_conversions;
mod session_discovery;
mod session_extraction;
mod session_files;
mod session_history;
mod session_tree;
#[path = "../../maestro-catalog/tests/support/subprocess.rs"]
mod subprocess;
mod summary_generation;
mod summary_text;
mod summary_usage;
mod tool_argument_roots;
/// Recorded corpus support shared by the test modules.
mod support {
    /// Branch scenarios.
    pub mod branch_summary;
    /// Compaction corpus consumer.
    pub mod compaction_cases;
    #[cfg(not(target_arch = "wasm32"))]
    pub mod discovery;
    /// Summary operation consumer.
    #[cfg(not(target_arch = "wasm32"))]
    pub mod generation;
    #[cfg(not(target_arch = "wasm32"))]
    pub mod native;
    /// Scripted summary answers with usage.
    #[cfg(not(target_arch = "wasm32"))]
    pub mod scripted_summary;
    /// Context corpus consumer.
    pub mod session_context;
    pub mod session_files;
    /// Memory-history corpus consumer.
    pub mod session_history;
    #[cfg(not(target_arch = "wasm32"))]
    pub mod session_tree;
    /// Consuming request controls.
    #[cfg(not(target_arch = "wasm32"))]
    pub mod summary_options;
    /// Full value comparison.
    pub mod value;
}
