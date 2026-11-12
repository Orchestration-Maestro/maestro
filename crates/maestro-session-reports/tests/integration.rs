//! Shared integration-test executable.
#![cfg(test)]

mod analysis_cost;
#[path = "support/analysis_fixtures.rs"]
mod analysis_fixtures;
mod analysis_shared;
mod analysis_tools;
mod analysis_transcripts;
mod analysis_usage;
/// Test-owned runtimes the report cases run on.
mod support {
    mod runtime;
    pub use runtime::run_io;
}
