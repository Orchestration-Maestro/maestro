//! Compiled screen detection manifests with explicit cache ownership.
use crate::agents::{
    Agent, AgentDetection, AgentState, AgentVisibility, agent_label, parse_agent_label,
};
use crate::manifest_update::ManifestVersion;
use regex::Regex;
use serde::Deserialize;
use std::path::{Path, PathBuf};
mod bundled;
mod cache;
mod evaluation;
mod records;
mod regions;
mod validation;
pub use cache::ManifestCache;
pub use evaluation::{agent_state_label, explain_to_json_value};
use evaluation::{evaluate_loaded_manifest, fallback_explain};
pub use records::*;
use regions::{TOP_NON_EMPTY_LINES_ENGINE_VERSION, region, region_count, top_region_count};
pub use validation::{ParsedRemoteManifest, parse_manifest, parse_remote_manifest_for_agent};
use validation::{compile_manifest, manifest_matches_agent};
/// Largest admitted number of authored rules.
const MAX_RULES_PER_MANIFEST: usize = 128;
/// Largest admitted gate nesting depth, with root depth zero.
const MAX_GATE_DEPTH: usize = 8;
/// Largest admitted aggregate number of gates.
const MAX_TOTAL_GATES: usize = 512;
/// Largest admitted number of direct matchers in one gate.
const MAX_MATCHERS_PER_GATE: usize = 32;
/// Largest admitted aggregate number of matchers.
const MAX_TOTAL_MATCHERS: usize = 1024;
/// Largest admitted Unicode-scalar length of one matcher.
const MAX_MATCHER_CHARS: usize = 512;
#[cfg(test)]
mod tests;
/// Returns the default full-screen region selector.
fn default_region() -> String {
    "whole_recent".to_owned()
}
