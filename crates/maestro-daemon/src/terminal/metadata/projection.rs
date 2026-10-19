//! Retained presentation projection operations.
use super::{AgentMetadata, AgentState, EffectivePresentation, HashMap, Instant, TerminalState};
impl TerminalState {
    /// Select the newest guarded, unexpired title override.
    #[must_use]
    pub fn effective_title(&self) -> Option<String> {
        self.effective_presentation_for_state_at(self.state, Instant::now())
            .title
    }

    /// Select the newest guarded, unexpired display-agent override.
    #[must_use]
    pub fn effective_display_agent(&self) -> Option<String> {
        self.effective_presentation_for_state_at(self.state, Instant::now())
            .display_agent
    }

    /// Project the currently visible presentation overrides.
    #[must_use]
    pub fn effective_presentation(&self) -> EffectivePresentation {
        self.effective_presentation_for_state_at(self.state, Instant::now())
    }

    /// Project guarded presentation while enforcing per-source expiry.
    pub(in crate::terminal) fn effective_presentation_for_state_at(
        &self,
        state: AgentState,
        now: Instant,
    ) -> EffectivePresentation {
        self.effective_presentation_for_state_at_with_ttl(state, now, true)
    }

    /// Project guarded presentation before removing an expired visible report.
    pub(super) fn effective_presentation_for_state_at_ignoring_ttl(
        &self,
        state: AgentState,
        now: Instant,
    ) -> EffectivePresentation {
        self.effective_presentation_for_state_at_with_ttl(state, now, false)
    }

    /// Assemble independently newest presentation fields with the selected expiry policy.
    pub(super) fn effective_presentation_for_state_at_with_ttl(
        &self,
        _state: AgentState,
        now: Instant,
        enforce_ttl: bool,
    ) -> EffectivePresentation {
        let mut presentation = EffectivePresentation::empty();
        presentation.title = self.newest_metadata_title(now, enforce_ttl);
        presentation.display_agent = self.newest_metadata_display_agent(now, enforce_ttl);
        presentation.state_labels = self.effective_metadata_state_labels(now, enforce_ttl);
        presentation
    }

    /// Iterate nonempty reports matching identity guards and the selected expiry policy.
    pub(super) fn valid_agent_metadata(
        &self,
        now: Instant,
        enforce_ttl: bool,
    ) -> impl Iterator<Item = &AgentMetadata> {
        self.agent_metadata
            .values()
            .filter(move |metadata| self.agent_metadata_is_valid(metadata, now, enforce_ttl))
    }

    /// Select title precedence by its field-specific report time.
    pub(super) fn newest_metadata_title(&self, now: Instant, enforce_ttl: bool) -> Option<String> {
        self.valid_agent_metadata(now, enforce_ttl)
            .filter(|metadata| metadata.title.is_some())
            .max_by_key(|metadata| metadata.title_reported_at)
            .and_then(|metadata| metadata.title.clone())
    }

    /// Select display-agent precedence by its field-specific report time.
    pub(super) fn newest_metadata_display_agent(
        &self,
        now: Instant,
        enforce_ttl: bool,
    ) -> Option<String> {
        self.valid_agent_metadata(now, enforce_ttl)
            .filter(|metadata| metadata.display_agent.is_some())
            .max_by_key(|metadata| metadata.display_agent_reported_at)
            .and_then(|metadata| metadata.display_agent.clone())
    }

    /// Merge labels in per-field timestamp order so the newest value wins.
    pub(super) fn effective_metadata_state_labels(
        &self,
        now: Instant,
        enforce_ttl: bool,
    ) -> HashMap<String, String> {
        let mut labels: Vec<_> = self
            .valid_agent_metadata(now, enforce_ttl)
            .flat_map(|metadata| {
                metadata.state_labels.iter().filter_map(|(state, label)| {
                    Some((
                        *metadata.state_label_reported_at.get(state)?,
                        state.clone(),
                        label.clone(),
                    ))
                })
            })
            .collect();
        labels.sort_by_key(|(reported_at, _, _)| *reported_at);
        labels
            .into_iter()
            .map(|(_, state, label)| (state, label))
            .collect()
    }

    /// Whether a nonempty report matches guards and the selected expiry policy.
    pub(super) fn agent_metadata_is_valid(
        &self,
        metadata: &AgentMetadata,
        now: Instant,
        enforce_ttl: bool,
    ) -> bool {
        if metadata.title.is_none()
            && metadata.display_agent.is_none()
            && metadata.state_labels.is_empty()
        {
            return false;
        }
        if enforce_ttl && Self::agent_metadata_is_expired(metadata, now) {
            return false;
        }
        self.agent_metadata_matches_guards(metadata)
    }

    /// Whether nonempty presentation matches current identity guards.
    pub(super) fn agent_metadata_is_visible_ignoring_ttl(&self, metadata: &AgentMetadata) -> bool {
        (metadata.title.is_some()
            || metadata.display_agent.is_some()
            || !metadata.state_labels.is_empty())
            && self.agent_metadata_matches_guards(metadata)
    }

    /// Apply the retained report identity and hook-source guards.
    pub(super) fn agent_metadata_matches_guards(&self, metadata: &AgentMetadata) -> bool {
        self.metadata_guards_match(
            metadata.agent_label.as_deref(),
            metadata.applies_to_source.as_deref(),
        )
    }
}
