//! Notification sound preferences and path selection.

use maestro_detection::agents::Agent;
use maestro_protocol::schema::NotificationShowSound;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Global and per-agent sound preferences.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SoundConfig {
    /// Global sound switch; false overrides every agent preference.
    pub enabled: bool,
    /// Shared authored mp3 path.
    pub path: Option<String>,
    /// Authored mp3 path for completion notifications.
    pub done_path: Option<String>,
    /// Authored mp3 path for request notifications.
    pub request_path: Option<String>,
    /// Per-agent overrides.
    pub agents: AgentSoundOverrides,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: None,
            done_path: None,
            request_path: None,
            agents: AgentSoundOverrides::default(),
        }
    }
}

impl SoundConfig {
    /// Whether the global switch and agent override allow a sound.
    #[must_use]
    pub fn allows(&self, agent: Option<Agent>) -> bool {
        self.enabled && self.agents.for_agent(agent) != AgentSoundSetting::Off
    }
}

impl SoundConfig {
    /// Select the kind-specific path before the shared path, relative to the agent root.
    #[must_use]
    pub fn path_for(&self, sound: NotificationShowSound, agent_dir: &Path) -> Option<PathBuf> {
        let path = match sound {
            NotificationShowSound::Done => self.done_path.as_ref().or(self.path.as_ref()),
            NotificationShowSound::Request => self.request_path.as_ref().or(self.path.as_ref()),
            NotificationShowSound::None => None,
        }?;
        Some(agent_dir.join(path))
    }
}

impl SoundConfig {
    /// Diagnose unusable authored files without selecting a replacement asset.
    #[must_use]
    pub fn diagnostics(&self, agent_dir: &Path) -> Vec<String> {
        let mut diagnostics = Vec::new();
        for (field, path) in [
            ("multiplexer.sound.path", self.path.as_deref()),
            ("multiplexer.sound.donePath", self.done_path.as_deref()),
            (
                "multiplexer.sound.requestPath",
                self.request_path.as_deref(),
            ),
        ] {
            let Some(path) = path else { continue };
            let resolved = agent_dir.join(path);
            let problem = if resolved
                .extension()
                .and_then(|ext| ext.to_str())
                .is_none_or(|ext| !ext.eq_ignore_ascii_case("mp3"))
            {
                Some(("unsupported sound file format", "; expected an mp3 file"))
            } else if !resolved.exists() {
                Some(("missing sound file", ""))
            } else if !resolved.is_file() {
                Some(("invalid sound file", ""))
            } else {
                None
            };
            if let Some((problem, detail)) = problem {
                diagnostics.push(format!(
                    "{problem}: {field} = {path} resolves to {}{detail}; using default sound",
                    resolved.display()
                ));
            }
        }
        diagnostics
    }
}

/// Agent-specific sound preference, subordinate to the global switch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSoundSetting {
    /// Inherit the global setting.
    #[default]
    Default,
    /// Enable sounds while the global switch is on.
    On,
    /// Disable this agent's sounds.
    Off,
}

/// Explicit per-agent sound settings.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentSoundOverrides {
    /// Sound preference for the `pi` catalog entry.
    pub pi: AgentSoundSetting,
    /// Sound preference for the `claude` catalog entry.
    pub claude: AgentSoundSetting,
    /// Sound preference for the `codex` catalog entry.
    pub codex: AgentSoundSetting,
    /// Sound preference for the `gemini` catalog entry.
    pub gemini: AgentSoundSetting,
    /// Sound preference for the `cursor` catalog entry.
    pub cursor: AgentSoundSetting,
    /// Sound preference for the `devin` catalog entry.
    pub devin: AgentSoundSetting,
    /// Sound preference for the `agy` catalog entry.
    pub agy: AgentSoundSetting,
    /// Sound preference for the `cline` catalog entry.
    pub cline: AgentSoundSetting,
    /// Sound preference for the `open_code` catalog entry.
    pub open_code: AgentSoundSetting,
    /// Sound preference for the `github_copilot` catalog entry.
    pub github_copilot: AgentSoundSetting,
    /// Sound preference for the `kimi` catalog entry.
    pub kimi: AgentSoundSetting,
    /// Sound preference for the `kiro` catalog entry.
    pub kiro: AgentSoundSetting,
    /// Sound preference for the `droid` catalog entry.
    pub droid: AgentSoundSetting,
    /// Sound preference for the `amp` catalog entry.
    pub amp: AgentSoundSetting,
    /// Sound preference for the `grok` catalog entry.
    pub grok: AgentSoundSetting,
    /// Sound preference for the `hermes` catalog entry.
    pub hermes: AgentSoundSetting,
    /// Sound preference for the `kilo` catalog entry.
    pub kilo: AgentSoundSetting,
    /// Sound preference for the `qodercli` catalog entry.
    pub qodercli: AgentSoundSetting,
    /// Sound preference for the `qwen` catalog entry.
    pub qwen: AgentSoundSetting,
    /// Sound preference for the `maki` catalog entry.
    pub maki: AgentSoundSetting,
}

impl AgentSoundOverrides {
    /// Resolve an agent's override without changing the global switch.
    #[must_use]
    pub const fn for_agent(&self, agent: Option<Agent>) -> AgentSoundSetting {
        match agent {
            Some(Agent::Pi) => self.pi,
            Some(Agent::Claude) => self.claude,
            Some(Agent::Codex) => self.codex,
            Some(Agent::Gemini) => self.gemini,
            Some(Agent::Cursor) => self.cursor,
            Some(Agent::Devin) => self.devin,
            Some(Agent::Antigravity) => self.agy,
            Some(Agent::Cline) => self.cline,
            Some(Agent::OpenCode) => self.open_code,
            Some(Agent::GithubCopilot) => self.github_copilot,
            Some(Agent::Kimi) => self.kimi,
            Some(Agent::Kiro) => self.kiro,
            Some(Agent::Droid) => self.droid,
            Some(Agent::Amp) => self.amp,
            Some(Agent::Grok) => self.grok,
            Some(Agent::Hermes) => self.hermes,
            Some(Agent::Kilo) => self.kilo,
            Some(Agent::Qodercli) => self.qodercli,
            Some(Agent::Qwen) => self.qwen,
            Some(Agent::Maki) => self.maki,
            Some(Agent::Omp | Agent::Mastracode) | None => AgentSoundSetting::Default,
        }
    }
}

impl Default for AgentSoundOverrides {
    fn default() -> Self {
        Self {
            pi: AgentSoundSetting::Default,
            claude: AgentSoundSetting::Default,
            codex: AgentSoundSetting::Default,
            gemini: AgentSoundSetting::Default,
            cursor: AgentSoundSetting::Default,
            devin: AgentSoundSetting::Default,
            agy: AgentSoundSetting::Default,
            cline: AgentSoundSetting::Default,
            open_code: AgentSoundSetting::Default,
            github_copilot: AgentSoundSetting::Default,
            kimi: AgentSoundSetting::Default,
            kiro: AgentSoundSetting::Default,
            droid: AgentSoundSetting::Off,
            amp: AgentSoundSetting::Default,
            grok: AgentSoundSetting::Default,
            hermes: AgentSoundSetting::Default,
            kilo: AgentSoundSetting::Default,
            qodercli: AgentSoundSetting::Default,
            qwen: AgentSoundSetting::Default,
            maki: AgentSoundSetting::Default,
        }
    }
}
