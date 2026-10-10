//! Strict records for the observable update corpus.
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// One distinct recorded input and its entire projected observation tree.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Stable case label for diagnostics.
    pub id: String,
    /// Public input and controlled adapter responses.
    pub input: Input,
    /// Recorded observable result.
    pub expected: Expected,
}
/// The public operation scenario and the fields it consumes.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Input {
    /// A named selection error.
    Named {
        user: Value,
        project: Value,
        source: String,
    },
    /// Offline policy applied after selection.
    Offline {
        user: Value,
        source: Option<String>,
        offline: String,
    },
    /// Pin or local admission.
    Skip { source: String },
    /// Installed and latest npm version comparison.
    Npm {
        installed: Option<String>,
        latest: Response,
    },
    /// Ordered scoped installation.
    Batch {
        user: Value,
        project: Value,
        command: Option<Vec<String>>,
        existing: bool,
    },
    /// Stateful progress observer.
    Callback { phase: String },
    /// Targeted Git refresh.
    Git(GitInput),
    /// Native checkout and history.
    Native { scenario: String },
    /// Independent execution groups at a producer barrier.
    Parallel {
        #[serde(rename = "userJobs")]
        user_jobs: usize,
        #[serde(rename = "projectJobs")]
        project_jobs: usize,
        #[serde(rename = "gitJobs")]
        git_jobs: usize,
    },
}
/// Controlled capture text or failure, without implicit coercion.
#[derive(Clone, Deserialize)]
#[serde(untagged)]
pub enum Response {
    /// Text from stdout.
    Text(String),
    /// A process-boundary error.
    Failure(Failure),
}
/// A process error response.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    /// Exact diagnostic returned at the process seam.
    pub error: String,
}
/// Selected Git target probes and inherited command failures.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GitInput {
    /// Upstream branch probe.
    pub upstream: Option<Response>,
    /// Initial and refreshed upstream head probe.
    pub upstream_head: Option<Response>,
    /// Required origin HEAD fallback.
    pub origin_head: Option<Response>,
    /// Optional symbolic fallback branch.
    pub symbolic: Option<Response>,
    /// Ignored remote set-head failure.
    #[serde(default)]
    pub set_head_error: bool,
    /// Whether the local checkout is current.
    #[serde(default)]
    pub current: Option<bool>,
    /// Inherited command to fail.
    pub fail: Option<String>,
    /// Captured probe to fail.
    pub fail_capture: Option<String>,
    /// Whether dependency installation is reached.
    #[serde(default)]
    pub manifest: bool,
    /// Live npm command prefix.
    pub command: Option<Vec<String>>,
    /// Missing checkout uses acquisition.
    #[serde(default)]
    pub missing: bool,
}
/// A command's public process-boundary operands.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    /// Selected executable.
    pub program: String,
    /// Literal argv in original order.
    pub args: Vec<String>,
    /// Optional authored working directory.
    pub cwd: Option<String>,
    /// Captured probe deadline in milliseconds.
    pub timeout_ms: Option<u64>,
}
/// A delivered progress event with optional replacement observer identity.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// Event phase.
    pub r#type: String,
    /// Public operation action.
    pub action: String,
    /// Original source or group label.
    pub source: String,
    /// Optional start or error message.
    pub message: Option<String>,
    /// The replacement observer, when used.
    pub observer: Option<String>,
}
/// Generated or preserved project files.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Files {
    /// Manifest bytes.
    pub manifest: String,
    /// Ignore-file bytes.
    pub ignore: String,
}
/// Native checkout observations after completion.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Native {
    /// Comparisons of the installed head against both retained identities.
    pub heads: Heads,
    /// Actual tracked file bytes.
    pub content: String,
    /// Untracked sentinel survival.
    pub untracked_exists: bool,
    /// Project checkout acquisition.
    pub project_acquired: bool,
}
/// Sealed admission and completion memberships.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Groups {
    /// Initial concurrently admitted jobs.
    pub first: BTreeMap<String, bool>,
    /// Jobs admitted before completion.
    pub started: BTreeMap<String, bool>,
    /// Jobs with after-effect completion witnesses.
    pub finished: BTreeMap<String, bool>,
}
/// Entire observable result; every retained field participates in equality.
#[derive(Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// Ordered captured and inherited commands.
    pub commands: Vec<Command>,
    /// Ordered delivered progress events.
    pub events: Vec<Event>,
    /// Winning failure, if any.
    pub error: Option<String>,
    /// Surviving user settings.
    pub settings: Option<Value>,
    /// Surviving project settings.
    pub project: Option<Value>,
    /// Project preparation bytes.
    pub files: Option<Files>,
    /// Native post-update observations.
    pub native: Option<Native>,
    /// Execution group memberships.
    pub groups: Option<Groups>,
}

/// Independent comparisons against the remote and originally installed heads.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Heads {
    /// Installed head equals the remote head.
    pub remote: bool,
    /// Installed head equals its initial value.
    pub original: bool,
}
