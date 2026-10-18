//! Integrations protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Integration install params record.
#[schemars(description = "")]
pub struct IntegrationInstallParams {
    /// Target.
    #[schemars(description = "")]
    pub target: IntegrationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Integration uninstall params record.
#[schemars(description = "")]
pub struct IntegrationUninstallParams {
    /// Target.
    #[schemars(description = "")]
    pub target: IntegrationTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Integration target record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum IntegrationTarget {
    /// Pi.
    #[schemars(description = "")]
    Pi,
    /// Omp.
    #[schemars(description = "")]
    Omp,
    /// Claude.
    #[schemars(description = "")]
    Claude,
    /// Codex.
    #[schemars(description = "")]
    Codex,
    /// Copilot.
    #[schemars(description = "")]
    Copilot,
    /// Devin.
    #[schemars(description = "")]
    Devin,
    /// Droid.
    #[schemars(description = "")]
    Droid,
    /// Kimi.
    #[schemars(description = "")]
    Kimi,
    /// Opencode.
    #[schemars(description = "")]
    Opencode,
    /// Kilo.
    #[schemars(description = "")]
    Kilo,
    /// Hermes.
    #[schemars(description = "")]
    Hermes,
    /// Qodercli.
    #[schemars(description = "")]
    Qodercli,
    /// Qwen.
    #[schemars(description = "")]
    Qwen,
    /// Cursor.
    #[schemars(description = "")]
    Cursor,
    /// Mastracode.
    #[schemars(description = "")]
    Mastracode,
    /// Antigravity cli.
    #[schemars(description = "")]
    AntigravityCli,
    /// Grok.
    #[schemars(description = "")]
    Grok,
}

impl IntegrationTarget {
    /// All.
    pub const ALL: [Self; 17] = [
        Self::Pi,
        Self::Omp,
        Self::Claude,
        Self::Codex,
        Self::Copilot,
        Self::Devin,
        Self::Droid,
        Self::Kimi,
        Self::Opencode,
        Self::Kilo,
        Self::Hermes,
        Self::Qodercli,
        Self::Qwen,
        Self::Cursor,
        Self::Mastracode,
        Self::AntigravityCli,
        Self::Grok,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Integration install result record.
#[schemars(description = "")]
pub struct IntegrationInstallResult {
    /// Messages.
    #[schemars(description = "")]
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Integration uninstall result record.
#[schemars(description = "")]
pub struct IntegrationUninstallResult {
    /// Messages.
    #[schemars(description = "")]
    pub messages: Vec<String>,
}
