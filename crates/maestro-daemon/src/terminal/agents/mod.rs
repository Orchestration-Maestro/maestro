//! Per-source presentation reports and explicit expiry.
use std::collections::HashMap;
use std::time::{Duration, Instant};

use maestro_detection::agents::AgentState;

use super::model::{PreviousEffectiveState, TerminalState, TerminalStateMutation};

/// Retained source presentation with independent field precedence and expiry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMetadata {
    /// Canonical integration provenance.
    pub source: String,
    /// External catalogue label or custom identity.
    pub agent_label: Option<String>,
    /// Optional canonical hook-source guard.
    pub applies_to_source: Option<String>,
    /// Optional pane-title override.
    pub title: Option<String>,
    /// Optional agent-display override.
    pub display_agent: Option<String>,
    /// Presentation labels indexed by execution state text.
    pub state_labels: HashMap<String, String>,
    /// Observation time of the retained report.
    pub reported_at: Instant,
    /// Last update time of the title field.
    title_reported_at: Option<Instant>,
    /// Last update time of the display-agent field.
    display_agent_reported_at: Option<Instant>,
    /// Last update time of each state-label field.
    state_label_reported_at: HashMap<String, Instant>,
    /// Optional lifetime measured from the report observation.
    pub ttl: Option<Duration>,
    /// Whether visible expiry still requires an explicit notification.
    expiry_event_pending: bool,
}

/// Source-local presentation update with clear, guard, sequence and lifetime controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMetadataReport {
    /// Canonical integration provenance.
    pub source: String,
    /// External catalogue label or custom identity.
    pub agent_label: Option<String>,
    /// Optional canonical hook-source guard.
    pub applies_to_source: Option<String>,
    /// Optional pane-title override.
    pub title: Option<String>,
    /// Optional agent-display override.
    pub display_agent: Option<String>,
    /// Presentation labels indexed by execution state text.
    pub state_labels: HashMap<String, String>,
    /// Whether to clear the retained title before applying replacements.
    pub clear_title: bool,
    /// Whether to clear the retained display agent before applying replacements.
    pub clear_display_agent: bool,
    /// Whether to clear retained state labels before applying replacements.
    pub clear_state_labels: bool,
    /// Optional lifetime measured from the report observation.
    pub ttl: Option<Duration>,
    /// Source-local freshness sequence.
    pub seq: Option<u64>,
}

/// Visible pane title, display identity and execution-state labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectivePresentation {
    /// Optional pane-title override.
    pub title: Option<String>,
    /// Optional agent-display override.
    pub display_agent: Option<String>,
    /// Presentation labels indexed by execution state text.
    pub state_labels: HashMap<String, String>,
}

impl EffectivePresentation {
    /// Construct presentation without any overrides.
    fn empty() -> Self {
        Self {
            title: None,
            display_agent: None,
            state_labels: HashMap::new(),
        }
    }
}

impl TerminalState {
    /// Check presentation sequence freshness without imposing token-source capacity.
    pub(crate) fn metadata_report_sequence_is_fresh(&self, source: &str, seq: Option<u64>) -> bool {
        maestro_workspace::metadata_tokens::sequence_is_fresh(
            &self.metadata_report_sequences,
            source,
            seq,
        )
    }

    /// Resolve explicit metadata identity or its official source and guard provenance.
    pub(crate) fn metadata_report_agent(
        source: &str,
        agent_label: Option<&str>,
        applies_to_source: Option<&str>,
    ) -> Option<maestro_detection::agents::Agent> {
        let explicit = agent_label.and_then(maestro_detection::agents::parse_agent_label);
        if explicit.is_some() {
            return explicit;
        }
        maestro_detection::agents::Agent::ALL
            .iter()
            .copied()
            .find(|agent| {
                let label = maestro_detection::agents::agent_label(*agent);
                crate::resume::is_official_agent_source(source, label)
                    || applies_to_source.is_some_and(|source| {
                        crate::resume::is_official_agent_source(source, label)
                    })
            })
    }

    /// Whether a metadata report belongs to the most recently exited agent.
    pub(crate) fn metadata_report_blocked_by_process_exit(
        &self,
        source: &str,
        agent_label: Option<&str>,
        applies_to_source: Option<&str>,
    ) -> bool {
        let Some(exit) = self.recent_agent_process_exit else {
            return false;
        };
        let exited_agent_label = maestro_detection::agents::agent_label(exit.agent);
        agent_label.and_then(maestro_detection::agents::parse_agent_label) == Some(exit.agent)
            || crate::resume::is_official_agent_source(source, exited_agent_label)
            || applies_to_source.is_some_and(|source| {
                crate::resume::is_official_agent_source(source, exited_agent_label)
            })
    }

    /// Admit fresh presentation sequences, bounding only sequenced token reporters.
    pub(crate) fn accept_metadata_report(
        &mut self,
        source: &str,
        seq: Option<u64>,
        includes_tokens: bool,
        agent: Option<maestro_detection::agents::Agent>,
    ) -> Result<bool, ()> {
        let Some(seq) = seq else {
            return Ok(true);
        };
        if !self.metadata_report_sequence_is_fresh(source, Some(seq)) {
            return Ok(false);
        }
        if includes_tokens
            && !self.metadata_token_sequence_sources.contains(source)
            && self.metadata_token_sequence_sources.len()
                >= maestro_workspace::metadata_tokens::MAX_SEQUENCE_SOURCES
        {
            return Err(());
        }
        self.metadata_report_sequences
            .insert(source.to_string(), seq);
        if let Some(agent) = agent {
            self.metadata_report_agents
                .insert(source.to_string(), agent);
        }
        if includes_tokens {
            self.metadata_token_sequence_sources
                .insert(source.to_string());
        }
        Ok(true)
    }

    /// Require each supplied agent and hook-source guard to match current ownership.
    fn metadata_guards_match(
        &self,
        agent_label: Option<&str>,
        applies_to_source: Option<&str>,
    ) -> bool {
        if agent_label.is_some_and(|agent| self.effective_agent_label() != Some(agent)) {
            return false;
        }
        !applies_to_source.is_some_and(|source| {
            self.hook_authority
                .as_ref()
                .is_none_or(|authority| authority.source != source)
        })
    }
}

/// Canonical report admission and field updates.
mod update;

/// Guarded per-field presentation selection.
mod projection;

/// Explicit metadata deadlines and expiry notifications.
mod expiry;

#[cfg(test)]
mod tests;
