//! Retained presentation update operations.
use super::{
    AgentMetadata, AgentMetadataReport, HashMap, Instant, TerminalState, TerminalStateMutation,
};
impl TerminalState {
    /// Admit canonical per-source presentation fields without changing execution state.
    pub fn set_agent_metadata(
        &mut self,
        mut report: AgentMetadataReport,
    ) -> Option<TerminalStateMutation> {
        report.source = maestro_detection::agents::normalize_agent_source(report.source);
        report.applies_to_source = report
            .applies_to_source
            .map(maestro_detection::agents::normalize_agent_source);
        if self.metadata_report_blocked_by_process_exit(
            &report.source,
            report.agent_label.as_deref(),
            report.applies_to_source.as_deref(),
        ) {
            return None;
        }
        let report_agent = Self::metadata_report_agent(
            &report.source,
            report.agent_label.as_deref(),
            report.applies_to_source.as_deref(),
        );
        if !matches!(
            self.accept_metadata_report(&report.source, report.seq, false, report_agent),
            Ok(true)
        ) {
            return None;
        }

        let now = Instant::now();
        if self
            .agent_metadata
            .get(&report.source)
            .is_some_and(|metadata| Self::agent_metadata_is_expired(metadata, now))
        {
            self.agent_metadata.remove(&report.source);
        }
        let previous = self.previous_effective_state_at(now);
        let report_source = report.source.clone();
        let report_has_ttl = report.ttl.is_some();
        if report.clear_title || report.clear_display_agent || report.clear_state_labels {
            self.update_agent_metadata_fields(report, now);
        } else {
            self.replace_agent_metadata(report, now);
        }
        if report_has_ttl
            && self
                .agent_metadata
                .get(&report_source)
                .is_some_and(|metadata| self.agent_metadata_is_visible_ignoring_ttl(metadata))
            && let Some(metadata) = self.agent_metadata.get_mut(&report_source)
        {
            metadata.expiry_event_pending = true;
        }

        let effective_state_change = self.recompute_effective_state(previous, now);

        if effective_state_change.is_some() {
            self.clear_expiry_pending_for_hidden_metadata();
        }

        Some(TerminalStateMutation {
            effective_state_change,
            session_ref_changed: false,
            agent_released: false,
        })
    }
    /// Clear selected fields, then apply only supplied replacements and their timestamps.
    pub(super) fn update_agent_metadata_fields(
        &mut self,
        report: AgentMetadataReport,
        now: Instant,
    ) {
        let has_set_fields = report.title.is_some()
            || report.display_agent.is_some()
            || !report.state_labels.is_empty();
        let metadata = self
            .agent_metadata
            .entry(report.source.clone())
            .or_insert_with(|| AgentMetadata {
                source: report.source.clone(),
                agent_label: report.agent_label.clone(),
                applies_to_source: report.applies_to_source.clone(),
                title: None,
                display_agent: None,
                state_labels: HashMap::new(),
                reported_at: now,
                title_reported_at: None,
                display_agent_reported_at: None,
                state_label_reported_at: HashMap::new(),
                ttl: report.ttl,
                expiry_event_pending: false,
            });
        if report.clear_title {
            metadata.title = None;
            metadata.title_reported_at = None;
        }
        if report.clear_display_agent {
            metadata.display_agent = None;
            metadata.display_agent_reported_at = None;
        }
        if report.clear_state_labels {
            metadata.state_labels.clear();
            metadata.state_label_reported_at.clear();
        }
        if let Some(agent_label) = report.agent_label {
            metadata.agent_label = Some(agent_label);
        }
        if let Some(applies_to_source) = report.applies_to_source {
            metadata.applies_to_source = Some(applies_to_source);
        }
        if let Some(title) = report.title {
            metadata.title = Some(title);
            metadata.title_reported_at = Some(now);
        }
        if let Some(display_agent) = report.display_agent {
            metadata.display_agent = Some(display_agent);
            metadata.display_agent_reported_at = Some(now);
        }
        for (state, label) in report.state_labels {
            metadata.state_labels.insert(state.clone(), label);
            metadata.state_label_reported_at.insert(state, now);
        }
        if has_set_fields || report.ttl.is_some() {
            metadata.reported_at = now;
            metadata.ttl = report.ttl;
            metadata.expiry_event_pending = false;
        }
    }

    /// Replace a source report and initialize independent field precedence.
    pub(super) fn replace_agent_metadata(&mut self, report: AgentMetadataReport, now: Instant) {
        let title_reported_at = report.title.as_ref().map(|_| now);
        let display_agent_reported_at = report.display_agent.as_ref().map(|_| now);
        let state_label_reported_at = report
            .state_labels
            .keys()
            .map(|state| (state.clone(), now))
            .collect();
        self.agent_metadata.insert(
            report.source.clone(),
            AgentMetadata {
                source: report.source,
                agent_label: report.agent_label,
                applies_to_source: report.applies_to_source,
                title: report.title,
                display_agent: report.display_agent,
                state_labels: report.state_labels,
                reported_at: now,
                title_reported_at,
                display_agent_reported_at,
                state_label_reported_at,
                ttl: report.ttl,
                expiry_event_pending: false,
            },
        );
    }
}
