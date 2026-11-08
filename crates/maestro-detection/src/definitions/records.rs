//! Manifest records and compiled matcher ownership.
use super::{
    Agent, AgentState, AgentVisibility, Deserialize, ManifestVersion, PathBuf, Regex,
    default_region,
};
/// Explain identifier for a known agent without matching live evidence.
pub const DEFAULT_KNOWN_AGENT_IDLE_FALLBACK: &str = "default_known_agent_idle_fallback";

/// Borrowed screen, terminal title and progress evidence for one detection call.
/// Use empty title and progress strings when those observations are unavailable.
#[derive(Debug, Clone, Copy)]
pub struct DetectionInput<'a> {
    /// Recent screen snapshot for live region selection.
    pub screen: &'a str,
    /// Terminal title evidence, separate from screen content.
    pub osc_title: &'a str,
    /// Terminal progress evidence, separate from screen content.
    pub osc_progress: &'a str,
}
/// Selected state, evidence and source metadata for one detection call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionExplain {
    /// Canonical catalogue identity, or the supplied unknown label in an explain result.
    pub agent: Option<String>,
    /// Recognized state, independent of confidence flags.
    pub state: AgentState,
    /// Active document provenance.
    pub source: Option<ManifestSource>,
    /// Highest-priority matching rule, with first-in-document tie breaking.
    pub matched_rule: Option<MatchedRule>,
    /// Whether screen detection itself was bypassed.
    pub screen_detection_skipped: bool,
    /// Independent live-state evidence flags.
    pub visible: AgentVisibility,
    /// Whether this screen must not change the retained pane state.
    pub skip_state_update: bool,
    /// Identifier of the rule suppressing state updates.
    pub skipped_update_reason: Option<String>,
    /// Reason returned when no live rule selects a state.
    pub fallback_reason: Option<String>,
    /// Rule outcomes in document order, including nonmatching rules.
    pub evaluated_rules: Vec<EvaluatedRule>,
    /// Ignored-source validation or loading diagnostic.
    pub warning: Option<String>,
    /// Version of the active document, when declared.
    pub manifest_version: Option<String>,
    /// Version of the cached download, including one shadowed by another source.
    pub cached_remote_version: Option<String>,
    /// Whether a local document exists alongside a valid cached download.
    pub local_override_shadowing_remote: bool,
    /// Most recent per-agent catalog outcome.
    pub remote_update_status: Option<String>,
    /// Most recent per-agent catalog failure detail.
    pub remote_update_error: Option<String>,
}
/// Provenance of an active detection document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestSource {
    /// Embedded default document.
    Bundled,
    /// Downloaded document and its declared version.
    Remote {
        /// Published document location.
        path: PathBuf,
        /// Declared version spelling.
        version: String,
    },
    /// Caller-supplied local document location.
    Override(PathBuf),
}

impl ManifestSource {
    /// Returns the source description used in explain output.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Bundled => "bundled".to_string(),
            Self::Remote { path, .. } => format!("remote:{}", path.display()),
            Self::Override(path) => path.display().to_string(),
        }
    }
    /// Returns the source category independent of its path.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::Remote { .. } => "remote",
            Self::Override(_) => "local override",
        }
    }
}
/// Active source and downloaded-version metadata for one catalogue entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentManifestSummary {
    /// Canonical catalogue identity.
    pub agent: Agent,
    /// Source selected after applying override and version precedence.
    pub active_source: ManifestSource,
    /// Version of the selected document, when present.
    pub active_version: Option<String>,
    /// Version of the cached download, including one shadowed by another source.
    pub cached_remote_version: Option<String>,
    /// Whether a local document exists alongside a valid cached download.
    pub local_override_shadowing_remote: bool,
    /// Ignored-source validation or loading diagnostic.
    pub warning: Option<String>,
}
/// Identity and state of the selected matching rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedRule {
    /// Authored document or rule identifier.
    pub id: String,
    /// Rule precedence, with larger values winning.
    pub priority: i32,
    /// Authored selector naming the screen or OSC evidence region.
    pub region: String,
    /// Recognized state, independent of confidence flags.
    pub state: AgentState,
}
/// One rule and its observed match result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedRule {
    /// Authored document or rule identifier.
    pub id: String,
    /// Rule precedence, with larger values winning.
    pub priority: i32,
    /// Authored selector naming the screen or OSC evidence region.
    pub region: String,
    /// Matcher definitions and the actual selected region preview.
    pub evidence: RuleEvidence,
    /// Recognized state, independent of confidence flags.
    pub state: AgentState,
    /// Whether every required gate condition matched.
    pub matched: bool,
}
/// Authored matcher metadata and a bounded preview of its input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleEvidence {
    /// Case-insensitive substring requirements.
    pub contains: Vec<String>,
    /// Whole-region regular expression requirements.
    pub regex: Vec<String>,
    /// Regular expressions matched independently against individual lines.
    pub line_regex: Vec<String>,
    /// Number of top-level conjunctive gates.
    pub all_count: usize,
    /// Number of top-level alternative gates.
    pub any_count: usize,
    /// Number of top-level exclusion gates.
    pub not_count: usize,
    /// Byte length of the untruncated selected region.
    pub region_bytes: usize,
    /// Region preview text returned by the explain operation.
    pub region_preview: String,
}
/// Validated document with compiled matchers and source-selection metadata.
#[derive(Debug, Clone)]
pub(super) struct LoadedManifest {
    /// Validated authored document retained for explain metadata.
    pub(super) manifest: AgentManifest,
    /// Matchers compiled once at cache creation or reload.
    pub(super) compiled_rules: Vec<CompiledRule>,
    /// Active document provenance.
    pub(super) source: ManifestSource,
    /// Ignored-source validation or loading diagnostic.
    pub(super) warning: Option<String>,
    /// Version of the cached download, including one shadowed by another source.
    pub(super) cached_remote_version: Option<String>,
    /// Whether a local document exists alongside a valid cached download.
    pub(super) local_override_shadowing_remote: bool,
}
/// Authored detection document; [`crate::definitions::parse_manifest`] validates its rules.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct AgentManifest {
    /// Authored document or rule identifier.
    pub(super) id: String,
    /// Validated declared document version.
    pub(super) version: Option<ManifestVersion>,
    /// Minimum declared engine required by a downloaded document.
    pub(super) min_engine_version: Option<u32>,
    /// Authored publication timestamp retained during parsing.
    #[serde(rename = "updated_at")]
    pub(super) _updated_at: Option<String>,
    /// Additional catalogue labels accepted by this document.
    #[serde(default)]
    pub(super) aliases: Vec<String>,
    /// Rules in authored order, preserving equal-priority precedence.
    #[serde(default)]
    pub(super) rules: Vec<ManifestRule>,
}
/// One prioritized region selector with match gates and a state result.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestRule {
    /// Authored document or rule identifier.
    pub(super) id: String,
    /// Recognized state, independent of confidence flags.
    pub(super) state: Option<ManifestState>,
    /// Rule precedence, with larger values winning.
    #[serde(default)]
    pub(super) priority: i32,
    /// Authored selector naming the screen or OSC evidence region.
    #[serde(default = "default_region")]
    pub(super) region: String,
    /// Independent live-state evidence flags.
    #[serde(flatten)]
    pub(super) visible: AgentVisibility,
    /// Whether this screen must not change the retained pane state.
    #[serde(default)]
    pub(super) skip_state_update: bool,
    /// Nested gates that must all match.
    #[serde(default)]
    pub(super) all: Vec<ManifestGate>,
    /// Nested gates of which at least one must match when nonempty.
    #[serde(default)]
    pub(super) any: Vec<ManifestGate>,
    /// Nested gates whose matches reject the enclosing gate.
    #[serde(default, rename = "not")]
    pub(super) not_gate: Vec<ManifestGate>,
    /// Case-insensitive substring requirements.
    #[serde(default)]
    pub(super) contains: Vec<String>,
    /// Whole-region regular expression requirements.
    #[serde(default)]
    pub(super) regex: Vec<String>,
    /// Regular expressions matched independently against individual lines.
    #[serde(default)]
    pub(super) line_regex: Vec<String>,
}
/// Recursive matcher requirements for one logical gate.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct ManifestGate {
    /// Nested gates that must all match.
    #[serde(default)]
    pub(super) all: Vec<ManifestGate>,
    /// Nested gates of which at least one must match when nonempty.
    #[serde(default)]
    pub(super) any: Vec<ManifestGate>,
    /// Nested gates whose matches reject the enclosing gate.
    #[serde(default, rename = "not")]
    pub(super) not_gate: Vec<ManifestGate>,
    /// Case-insensitive substring requirements.
    #[serde(default)]
    pub(super) contains: Vec<String>,
    /// Whole-region regular expression requirements.
    #[serde(default)]
    pub(super) regex: Vec<String>,
    /// Regular expressions matched independently against individual lines.
    #[serde(default)]
    pub(super) line_regex: Vec<String>,
}
/// Compiled matcher tree for a validated rule.
#[derive(Debug, Clone)]
pub(super) struct CompiledRule {
    /// Matcher tree owned by this rule.
    pub(super) gate: CompiledGate,
}
/// Compiled regexes and normalized needles for a logical gate.
#[derive(Debug, Clone)]
pub(super) struct CompiledGate {
    /// Nested gates that must all match.
    pub(super) all: Vec<CompiledGate>,
    /// Nested gates of which at least one must match when nonempty.
    pub(super) any: Vec<CompiledGate>,
    /// Nested gates whose matches reject the enclosing gate.
    pub(super) not_gate: Vec<CompiledGate>,
    /// Case-insensitive substring requirements.
    pub(super) contains: Vec<String>,
    /// Whole-region regular expression requirements.
    pub(super) regex: Vec<Regex>,
    /// Regular expressions matched independently against individual lines.
    pub(super) line_regex: Vec<Regex>,
}
/// States admitted by detection documents; completion is derived elsewhere.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ManifestState {
    /// Waiting for input.
    Idle,
    /// Actively producing output or running work.
    Working,
    /// Waiting for a human decision.
    Blocked,
    /// No state assertion; used by neutral suppression rules.
    Unknown,
}

impl From<ManifestState> for AgentState {
    fn from(value: ManifestState) -> Self {
        match value {
            ManifestState::Idle => AgentState::Idle,
            ManifestState::Working => AgentState::Working,
            ManifestState::Blocked => AgentState::Blocked,
            ManifestState::Unknown => AgentState::Unknown,
        }
    }
}
