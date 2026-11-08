//! Session identity admission and literal resume plans.

use std::path::Path;

use maestro_detection::agents::normalize_agent_source;
/// Protocol-owned session reference representation.
pub use maestro_protocol::shared::AgentSessionRefKind;

/// Maximum accepted session identifier length in UTF-8 bytes.
const MAX_SESSION_ID_LEN: usize = 512;
/// Maximum accepted absolute session path length in UTF-8 bytes.
const MAX_SESSION_PATH_LEN: usize = 4096;

/// A validated session identifier or absolute path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionRef {
    /// Reference representation.
    pub kind: AgentSessionRefKind,
    /// Owned session identifier or path.
    pub value: String,
}

/// A literal command and its stable duplicate-suppression identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentResumePlan {
    /// External catalogue agent label.
    pub agent: String,
    /// Executable followed by literal arguments.
    pub argv: Vec<String>,
    /// NUL-separated provenance, agent, kind and value.
    pub dedupe_key: String,
}

/// Canonical provenance and validated identity retained for restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedAgentSession {
    /// Canonical integration provenance.
    pub source: String,
    /// External catalogue agent label.
    pub agent: String,
    /// Validated identity to resume.
    pub session_ref: AgentSessionRef,
}

impl AgentSessionRef {
    /// Validate and own a session identifier.
    pub fn id(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        valid_session_id(&value).then_some(Self {
            kind: AgentSessionRefKind::Id,
            value,
        })
    }

    /// Validate and own an absolute session path.
    pub fn path(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        valid_session_path(&value).then_some(Self {
            kind: AgentSessionRefKind::Path,
            value,
        })
    }
}

/// Admit an official report, preferring a supported path over an identifier.
#[must_use]
pub fn session_ref_from_report(
    source: &str,
    agent: &str,
    agent_session_id: Option<String>,
    agent_session_path: Option<String>,
) -> Option<AgentSessionRef> {
    if !is_official_agent_source(source, agent) {
        return None;
    }

    if agent == "pi" || agent == "omp" {
        return agent_session_path
            .and_then(AgentSessionRef::path)
            .or_else(|| agent_session_id.and_then(AgentSessionRef::id));
    }

    agent_session_id.and_then(AgentSessionRef::id)
}

/// Trim and admit a known lifecycle start reason.
#[must_use]
pub fn normalize_session_start_source(value: Option<String>) -> Option<String> {
    value.and_then(|value| match value.trim() {
        source @ ("startup" | "resume" | "clear" | "compact" | "branch" | "new" | "fork"
        | "select") => Some(source.to_owned()),
        _ => None,
    })
}

/// Whether native detection reserves state for this official integration.
#[must_use]
pub fn is_reserved_native_state_source(source: &str, agent: &str) -> bool {
    matches!(
        (source, agent),
        ("maestro:claude", "claude")
            | ("maestro:codex", "codex")
            | ("maestro:copilot", "copilot")
            | ("maestro:devin", "devin")
            | ("maestro:droid", "droid")
            | ("maestro:qodercli", "qodercli")
            | ("maestro:qwen", "qwen")
            | ("maestro:cursor", "cursor")
            | ("maestro:grok", "grok")
    )
}

/// Validate restored identity and retain canonical source provenance.
#[must_use]
pub fn session_ref_from_snapshot(
    source: &str,
    agent: &str,
    kind: AgentSessionRefKind,
    value: &str,
) -> Option<PersistedAgentSession> {
    let source = normalize_agent_source(source.to_owned());
    if !is_official_agent_source(&source, agent) {
        return None;
    }
    let session_ref = match (agent, kind) {
        ("pi" | "omp", AgentSessionRefKind::Path) => AgentSessionRef::path(value)?,
        (_, AgentSessionRefKind::Id) => AgentSessionRef::id(value)?,
        _ => return None,
    };
    Some(PersistedAgentSession {
        source,
        agent: agent.to_string(),
        session_ref,
    })
}

/// Build a literal resume command for an official, supported session reference.
#[must_use]
pub fn plan(source: &str, agent: &str, session_ref: &AgentSessionRef) -> Option<AgentResumePlan> {
    if !is_official_agent_source(source, agent) {
        return None;
    }

    if session_ref.kind == AgentSessionRefKind::Path && !matches!(agent, "pi" | "omp") {
        return None;
    }
    let argv = resume_argv(agent, &session_ref.value)?;

    Some(AgentResumePlan {
        agent: agent.to_string(),
        argv,
        dedupe_key: dedupe_key(source, agent, session_ref),
    })
}

/// Identify a resume operation without ambiguous field concatenation.
#[must_use]
pub fn dedupe_key(source: &str, agent: &str, session_ref: &AgentSessionRef) -> String {
    format!(
        "{source}\u{0}{agent}\u{0}{:?}\u{0}{}",
        session_ref.kind, session_ref.value
    )
}

/// Whether canonical provenance matches an integration-supported agent.
pub(crate) fn is_official_agent_source(source: &str, agent: &str) -> bool {
    matches!(
        (source, agent),
        ("maestro:claude", "claude")
            | ("maestro:codex", "codex")
            | ("maestro:copilot", "copilot")
            | ("maestro:devin", "devin")
            | ("maestro:droid", "droid")
            | ("maestro:kimi", "kimi")
            | ("maestro:omp", "omp")
            | ("maestro:mastracode", "mastracode")
            | ("maestro:pi", "pi")
            | ("maestro:hermes", "hermes")
            | ("maestro:opencode", "opencode")
            | ("maestro:qodercli", "qodercli")
            | ("maestro:qwen", "qwen")
            | ("maestro:kilo", "kilo")
            | ("maestro:cursor", "cursor")
            | ("maestro:antigravity_cli", "agy")
            | ("maestro:grok", "grok")
    )
}

/// Check identifier byte length and reject control characters.
fn valid_session_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_SESSION_ID_LEN && !value.chars().any(char::is_control)
}

/// Check path byte length, control characters and absoluteness.
fn valid_session_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SESSION_PATH_LEN
        && !value.chars().any(char::is_control)
        && Path::new(value).is_absolute()
}

/// Construct arguments without shell interpretation.
fn resume_argv(agent: &str, value: &str) -> Option<Vec<String>> {
    if matches!(agent, "copilot" | "omp") {
        return Some(vec![agent.into(), format!("--resume={value}")]);
    }
    let (executable, option) = match agent {
        "claude" | "devin" | "droid" | "hermes" | "qodercli" | "qwen" | "grok" => {
            (agent, "--resume")
        }
        "codex" => (agent, "resume"),
        "kimi" | "pi" | "opencode" | "kilo" => (agent, "--session"),
        "mastracode" => (agent, "--thread"),
        "agy" => (agent, "--conversation"),
        "cursor" => (
            if cfg!(windows) {
                "cursor-agent.cmd"
            } else {
                "cursor-agent"
            },
            "--resume",
        ),
        _ => return None,
    };
    Some(vec![executable.into(), option.into(), value.into()])
}

#[cfg(test)]
mod tests;
