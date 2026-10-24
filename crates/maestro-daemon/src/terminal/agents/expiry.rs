//! Retained presentation expiry operations.
use super::{AgentMetadata, Instant, PreviousEffectiveState, TerminalState, TerminalStateMutation};
impl TerminalState {
    /// Find the next guarded expiry, retaining pending immediately due events.
    #[must_use]
    pub fn next_agent_metadata_expiry(&self) -> Option<Instant> {
        let now = Instant::now();
        self.agent_metadata
            .values()
            .filter(|metadata| self.agent_metadata_matches_guards(metadata))
            .filter_map(Self::agent_metadata_expiry)
            .filter(|deadline| {
                *deadline > now
                    || self.agent_metadata.values().any(|metadata| {
                        metadata.expiry_event_pending
                            && self.agent_metadata_matches_guards(metadata)
                            && Self::agent_metadata_expiry(metadata) == Some(*deadline)
                    })
            })
            .min()
    }

    /// Remove due reports and return the visible change associated with the scheduled expiry.
    pub fn expire_agent_metadata_at(
        &mut self,
        scheduled_deadline: Instant,
        now: Instant,
    ) -> Option<TerminalStateMutation> {
        let (expired_sources, stale_sources): (Vec<_>, Vec<_>) = self
            .agent_metadata
            .iter()
            .filter_map(|(source, metadata)| {
                let deadline = Self::agent_metadata_expiry(metadata)?;
                (deadline <= now).then_some((source.clone(), deadline))
            })
            .partition(|(_, deadline)| *deadline >= scheduled_deadline);
        let expired_sources: Vec<_> = expired_sources
            .into_iter()
            .map(|(source, _)| source)
            .collect();
        let stale_sources: Vec<_> = stale_sources
            .into_iter()
            .map(|(source, _)| source)
            .collect();
        for source in stale_sources {
            self.agent_metadata.remove(&source);
        }
        if expired_sources.is_empty() {
            return None;
        }

        let previous_agent_label = self.effective_agent_label().map(str::to_string);
        let previous_known_agent = self.effective_known_agent();
        let previous_state = self.state;
        let previous_presentation =
            self.effective_presentation_for_state_at_ignoring_ttl(previous_state, now);
        let previous = PreviousEffectiveState {
            agent_label: previous_agent_label,
            known_agent: previous_known_agent,
            state: previous_state,
            presentation: previous_presentation,
        };
        for source in expired_sources {
            if let Some(metadata) = self.agent_metadata.get_mut(&source) {
                metadata.expiry_event_pending = false;
            }
            self.agent_metadata.remove(&source);
        }

        Some(TerminalStateMutation {
            effective_state_change: self.recompute_effective_state(previous, now),
            session_ref_changed: false,
            agent_released: false,
        })
    }

    /// Cancel pending expiry notifications for reports hidden by lifecycle guards.
    pub(in crate::terminal) fn clear_expiry_pending_for_hidden_metadata(&mut self) {
        let hidden_sources: Vec<_> = self
            .agent_metadata
            .iter()
            .filter(|(_, metadata)| {
                metadata.expiry_event_pending
                    && !self.agent_metadata_is_visible_ignoring_ttl(metadata)
            })
            .map(|(source, _)| source.clone())
            .collect();
        for source in hidden_sources {
            if let Some(metadata) = self.agent_metadata.get_mut(&source) {
                metadata.expiry_event_pending = false;
            }
        }
    }

    /// Whether a report deadline has arrived at the supplied observation time.
    pub(super) fn agent_metadata_is_expired(metadata: &AgentMetadata, now: Instant) -> bool {
        Self::agent_metadata_expiry(metadata).is_some_and(|deadline| now >= deadline)
    }

    /// Compute a report deadline with overflow falling back to its report time.
    pub(super) fn agent_metadata_expiry(metadata: &AgentMetadata) -> Option<Instant> {
        metadata.ttl.map(|ttl| {
            metadata
                .reported_at
                .checked_add(ttl)
                .unwrap_or(metadata.reported_at)
        })
    }
}
