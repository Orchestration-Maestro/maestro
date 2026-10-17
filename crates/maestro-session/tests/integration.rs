//! Shared integration-test executable.
#![cfg(test)]

mod branch_summary;
#[path = "../../maestro-catalog/tests/support/subprocess.rs"]
mod subprocess;
use subprocess::child_process;
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
mod summary_generation;
mod summary_text;
/// Recorded corpus support shared by the test modules.
mod support {
    /// Branch scenarios.
    pub mod branch_summary;
    /// Compaction corpus consumer.
    pub mod compaction_cases;
    #[cfg(not(target_arch = "wasm32"))]
    pub mod discovery;
    /// Summary operation consumer.
    pub mod generation;
    #[cfg(not(target_arch = "wasm32"))]
    pub mod native;
    /// Disposable directory.
    pub mod scratch;
    /// Context corpus consumer.
    pub mod session_context;
    pub mod session_files;
    /// Memory-history corpus consumer.
    pub mod session_history;
    pub mod session_tree;
    /// Consuming request controls.
    pub mod summary_options;
    /// Full value comparison.
    pub mod value;
}
