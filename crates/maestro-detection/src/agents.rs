//! Agent identity from catalogue labels and supplied process facts.

/// The shared four-state pane detection vocabulary.
pub use maestro_protocol::schema::PaneAgentState as AgentState;

/// Screen-derived state and confidence for arbitration with integration reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentDetection {
    /// State recognized from the screen.
    pub state: AgentState,
    /// Whether the screen displays history rather than the live prompt.
    pub skip_state_update: bool,
    /// Live chrome recognized on the screen.
    pub visible: AgentVisibility,
}

/// Independently recognized live-screen indicators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(default)]
pub struct AgentVisibility {
    /// Whether live idle chrome is visible.
    pub visible_idle: bool,
    /// Whether live chrome requests human input.
    pub visible_blocker: bool,
    /// Whether live working chrome is visible.
    pub visible_working: bool,
}

/// Which agent we detected running in a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    /// The `Pi` catalogue entry.
    Pi,
    /// The `Claude` catalogue entry.
    Claude,
    /// The `Codex` catalogue entry.
    Codex,
    /// The `Gemini` catalogue entry.
    Gemini,
    /// The `Cursor` catalogue entry.
    Cursor,
    /// The `Devin` catalogue entry.
    Devin,
    /// The `Antigravity` catalogue entry.
    Antigravity,
    /// The `Cline` catalogue entry.
    Cline,
    /// The `Omp` catalogue entry.
    Omp,
    /// The `Mastracode` catalogue entry.
    Mastracode,
    /// The `OpenCode` catalogue entry.
    OpenCode,
    /// The `GithubCopilot` catalogue entry.
    GithubCopilot,
    /// The `Kimi` catalogue entry.
    Kimi,
    /// The `Kiro` catalogue entry.
    Kiro,
    /// The `Droid` catalogue entry.
    Droid,
    /// The `Amp` catalogue entry.
    Amp,
    /// The `Grok` catalogue entry.
    Grok,
    /// The `Hermes` catalogue entry.
    Hermes,
    /// The `Kilo` catalogue entry.
    Kilo,
    /// The `Qodercli` catalogue entry.
    Qodercli,
    /// The `Qwen` catalogue entry.
    Qwen,
    /// The `Maki` catalogue entry.
    Maki,
}

impl Agent {
    /// Every catalogue entry in declaration order.
    pub const ALL: [Self; 22] = [
        Self::Pi,
        Self::Claude,
        Self::Codex,
        Self::Gemini,
        Self::Cursor,
        Self::Devin,
        Self::Antigravity,
        Self::Cline,
        Self::Omp,
        Self::Mastracode,
        Self::OpenCode,
        Self::GithubCopilot,
        Self::Kimi,
        Self::Kiro,
        Self::Droid,
        Self::Amp,
        Self::Grok,
        Self::Hermes,
        Self::Kilo,
        Self::Qodercli,
        Self::Qwen,
        Self::Maki,
    ];

    /// Entries with screen manifests.
    pub const SCREEN_MANIFEST_AGENTS: [Self; 20] = [
        Self::Pi,
        Self::Claude,
        Self::Codex,
        Self::Gemini,
        Self::Cursor,
        Self::Devin,
        Self::Antigravity,
        Self::Cline,
        Self::OpenCode,
        Self::GithubCopilot,
        Self::Kimi,
        Self::Kiro,
        Self::Droid,
        Self::Amp,
        Self::Grok,
        Self::Hermes,
        Self::Kilo,
        Self::Qodercli,
        Self::Qwen,
        Self::Maki,
    ];
}

/// Returns the canonical catalogue label.
///
#[must_use]
pub fn agent_label(agent: Agent) -> &'static str {
    match agent {
        Agent::Pi => "pi",
        Agent::Claude => "claude",
        Agent::Codex => "codex",
        Agent::Gemini => "gemini",
        Agent::Cursor => "cursor",
        Agent::Devin => "devin",
        Agent::Antigravity => "agy",
        Agent::Cline => "cline",
        Agent::Omp => "omp",
        Agent::Mastracode => "mastracode",
        Agent::OpenCode => "opencode",
        Agent::GithubCopilot => "copilot",
        Agent::Kimi => "kimi",
        Agent::Kiro => "kiro",
        Agent::Droid => "droid",
        Agent::Amp => "amp",
        Agent::Grok => "grok",
        Agent::Hermes => "hermes",
        Agent::Kilo => "kilo",
        Agent::Qodercli => "qodercli",
        Agent::Qwen => "qwen",
        Agent::Maki => "maki",
    }
}

/// Returns the interactive executable for the current platform.
///
#[must_use]
pub fn interactive_agent_executable(agent: Agent) -> &'static str {
    match agent {
        Agent::Pi => "pi",
        Agent::Claude => "claude",
        Agent::Codex => "codex",
        Agent::Gemini => "gemini",
        Agent::Cursor => {
            if cfg!(windows) {
                "cursor-agent.cmd"
            } else {
                "cursor-agent"
            }
        }
        Agent::Devin => "devin",
        Agent::Antigravity => "agy",
        Agent::Cline => "cline",
        Agent::Omp => "omp",
        Agent::Mastracode => "mastracode",
        Agent::OpenCode => "opencode",
        Agent::GithubCopilot => "copilot",
        Agent::Kimi => "kimi",
        Agent::Kiro => "kiro-cli",
        Agent::Droid => "droid",
        Agent::Amp => "amp",
        Agent::Grok => "grok",
        Agent::Hermes => "hermes",
        Agent::Kilo => "kilo",
        Agent::Qodercli => "qodercli",
        Agent::Qwen => "qwen",
        Agent::Maki => "maki",
    }
}

/// Recognizes canonical labels, aliases, case and executable suffixes.
///
#[must_use]
pub fn parse_agent_label(agent: &str) -> Option<Agent> {
    let name = normalized_agent_lookup_name(agent);
    parse_canonical_agent_label(&name).or_else(|| lookup_agent(&name))
}

/// Recognizes only an exact canonical catalogue label.
///
#[must_use]
pub fn parse_canonical_agent_label(label: &str) -> Option<Agent> {
    let agent = lookup_agent(label)?;
    (agent_label(agent) == label).then_some(agent)
}

/// Matches a normalized label or alias.
fn lookup_agent(name: &str) -> Option<Agent> {
    match name {
        "pi" => Some(Agent::Pi),
        "claude" | "claude-code" => Some(Agent::Claude),
        "codex" => Some(Agent::Codex),
        "gemini" => Some(Agent::Gemini),
        "cursor" | "cursor-agent" => Some(Agent::Cursor),
        "devin" | "devin-cli" | "devin cli" => Some(Agent::Devin),
        "agy" | "antigravity" | "antigravity-cli" => Some(Agent::Antigravity),
        "cline" => Some(Agent::Cline),
        "omp" => Some(Agent::Omp),
        "mastracode" | "mastra-code" | "mastra code" => Some(Agent::Mastracode),
        "opencode" | "opencode2" | "open-code" => Some(Agent::OpenCode),
        "copilot" | "github-copilot" | "ghcs" => Some(Agent::GithubCopilot),
        "kimi" | "kimi-code" | "kimi code" => Some(Agent::Kimi),
        "kiro" | "kiro-cli" => Some(Agent::Kiro),
        "droid" => Some(Agent::Droid),
        "amp" | "amp-local" => Some(Agent::Amp),
        "grok" | "grok-build" => Some(Agent::Grok),
        "hermes" | "hermes-agent" => Some(Agent::Hermes),
        "kilo" | "kilo-code" | "kilo code" => Some(Agent::Kilo),
        "qodercli" | "qoderclicn" | "qoder" | "qodercn" => Some(Agent::Qodercli),
        "qwen" | "qwen-code" | "qwen code" => Some(Agent::Qwen),
        "maki" => Some(Agent::Maki),
        _ => None,
    }
}

/// Returns `None` for plain shells or unrecognized programs.
#[must_use]
pub fn identify_agent(process_name: &str) -> Option<Agent> {
    parse_agent_label(process_name)
}

/// Trims and lowercases a name, stripping its first matching executable suffix.
fn normalized_agent_lookup_name(name: &str) -> String {
    let mut name = name.trim().to_lowercase();
    for suffix in [".exe", ".cmd", ".bat", ".ps1", ".js"] {
        if name.ends_with(suffix) {
            name.truncate(name.len() - suffix.len());
            break;
        }
    }
    name
}

#[cfg(test)]
mod tests;

/// Canonicalizes the legacy provenance prefix, preserving all other text.
#[must_use]
pub fn normalize_agent_source(source: String) -> String {
    if let Some(suffix) = source.strip_prefix("herdr:") {
        format!("maestro:{suffix}")
    } else {
        source
    }
}

/// Returns whether the source and canonical label have lifecycle authority.
#[must_use]
pub fn full_lifecycle_hook_authority(source: &str, agent_label: &str) -> bool {
    matches!(
        (source, agent_label),
        ("maestro:pi", "pi")
            | ("maestro:omp", "omp")
            | ("maestro:mastracode", "mastracode")
            | ("maestro:opencode", "opencode")
            | ("maestro:kilo", "kilo")
            | ("maestro:kimi", "kimi")
    )
}

/// Returns whether the source supplies session identity without lifecycle state.
#[must_use]
pub fn session_identity_only_integration(source: &str, agent_label: &str) -> bool {
    matches!(
        (source, agent_label),
        ("maestro:hermes", "hermes")
            | ("maestro:qwen", "qwen")
            | ("maestro:antigravity_cli", "agy")
    )
}

#[cfg(target_os = "linux")]
mod process;
#[cfg(target_os = "linux")]
pub use process::identify_agent_in_job;
