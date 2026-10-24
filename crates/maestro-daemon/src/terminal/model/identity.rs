//! Identity operations on the retained state.
use super::{
    Agent, AgentState, EffectiveStateChange, FullLifecycleHookSuppressionReason, HookAuthority,
    Instant, TerminalState, TerminalStateMutation,
};
impl TerminalState {
    /// Whether a known incoming identity disagrees with foreground process evidence.
    pub(in crate::terminal) fn known_agent_label_conflicts_with_detected_agent(
        &self,
        agent_label: &str,
    ) -> bool {
        let Some(detected_agent) = self.detected_agent else {
            return false;
        };
        maestro_detection::agents::parse_agent_label(agent_label)
            .is_some_and(|hook_agent| hook_agent != detected_agent)
    }

    /// Require a recognized start reason and matching foreground identity for owner takeover.
    pub(in crate::terminal) fn foreground_agent_confirms_different_owner_takeover(
        &self,
        source: &str,
        agent_label: &str,
        session_ref: &crate::resume::AgentSessionRef,
        session_start_source: Option<&str>,
    ) -> bool {
        Self::session_start_source_is_recognized(session_start_source)
            && self.foreground_agent_confirms_session_owner(source, agent_label, session_ref)
    }

    /// Require a resumable matching foreground session before replacing another hook owner.
    pub(in crate::terminal) fn foreground_agent_confirms_hook_authority_takeover(
        &self,
        source: &str,
        agent_label: &str,
        session_ref: Option<&crate::resume::AgentSessionRef>,
    ) -> bool {
        session_ref.is_some_and(|session_ref| {
            self.foreground_agent_confirms_session_owner(source, agent_label, session_ref)
        })
    }

    /// Whether the detected agent matches a resumable reported session.
    pub(in crate::terminal) fn foreground_agent_confirms_session_owner(
        &self,
        source: &str,
        agent_label: &str,
        session_ref: &crate::resume::AgentSessionRef,
    ) -> bool {
        let Some(detected_agent) = self.detected_agent else {
            return false;
        };
        maestro_detection::agents::parse_agent_label(agent_label) == Some(detected_agent)
            && crate::resume::plan(source, agent_label, session_ref).is_some()
    }

    /// Admit strictly increasing source-local sequences, rejecting later unsequenced reports.
    pub(in crate::terminal) fn accept_hook_report(
        &mut self,
        source: &str,
        seq: Option<u64>,
    ) -> bool {
        let Some(seq) = seq else {
            return !self.hook_report_sequences.contains_key(source);
        };

        if self
            .hook_report_sequences
            .get(source)
            .is_some_and(|last_seq| seq <= *last_seq)
        {
            return false;
        }

        self.hook_report_sequences.insert(source.to_string(), seq);
        true
    }

    /// Clear matching authority and return any effective state change.
    #[cfg(test)]
    pub fn clear_hook_authority(
        &mut self,
        source: Option<&str>,
        seq: Option<u64>,
    ) -> Option<EffectiveStateChange> {
        self.clear_hook_authority_with_mutation(source, seq)
            .and_then(|mutation| mutation.effective_state_change)
    }

    /// Clear matching canonical authority after checking its source-local sequence.
    pub fn clear_hook_authority_with_mutation(
        &mut self,
        source: Option<&str>,
        seq: Option<u64>,
    ) -> Option<TerminalStateMutation> {
        let source = source
            .map(|source| maestro_detection::agents::normalize_agent_source(source.to_owned()));
        let source = source.as_deref();
        let sequence_source = source.map(str::to_string).or_else(|| {
            self.hook_authority
                .as_ref()
                .map(|authority| authority.source.clone())
        });
        let should_clear = self
            .hook_authority
            .as_ref()
            .is_some_and(|authority| source.is_none_or(|source| authority.source == source));
        if !should_clear {
            return None;
        }
        if let Some(source) = sequence_source.as_deref()
            && !self.accept_hook_report(source, seq)
        {
            return None;
        }

        let now = Instant::now();
        let previous = self.previous_effective_state_at(now);
        let had_session = self.current_session_identity_for_persistence().is_some();
        self.suppress_current_full_lifecycle_hook_authority(
            FullLifecycleHookSuppressionReason::HookClear,
        );
        self.hook_authority = None;
        self.persisted_agent_session = None;
        Some(TerminalStateMutation {
            effective_state_change: self.recompute_effective_state(previous, now),
            session_ref_changed: had_session,
            agent_released: false,
        })
    }

    /// Release matching canonical identity while preserving live process and foreign session ownership.
    pub fn release_agent_with_mutation(
        &mut self,
        source: &str,
        agent_label: &str,
        seq: Option<u64>,
    ) -> Option<TerminalStateMutation> {
        let source = maestro_detection::agents::normalize_agent_source(source.to_owned());
        let source = source.as_str();
        if self.hook_authority.as_ref().is_some_and(|authority| {
            authority.agent_label != agent_label || authority.source != source
        }) {
            return None;
        }

        let matches_current_agent = self.effective_agent_label() == Some(agent_label);
        let matches_persisted_session = self.persisted_agent_session_matches(source, agent_label);
        if !matches_current_agent && !matches_persisted_session {
            return None;
        }
        if !self.accept_hook_report(source, seq) {
            return None;
        }
        let preserve_foreign_persisted_session = self
            .persisted_agent_session
            .as_ref()
            .is_some_and(|session| session.source != source || session.agent != agent_label);
        let process_owns_agent = maestro_detection::agents::parse_agent_label(agent_label)
            .is_some_and(|agent| {
                self.detected_agent == Some(agent) && self.recent_agent_process_exit.is_none()
            });

        let now = Instant::now();
        let previous = self.previous_effective_state_at(now);
        let previous_session = self.session_identity_snapshot();
        self.suppress_full_lifecycle_hook_report(
            source,
            agent_label,
            FullLifecycleHookSuppressionReason::HookClear,
        );
        if !process_owns_agent {
            self.detected_agent = None;
            self.fallback_state = AgentState::Unknown;
            self.fallback_visible_blocker = false;
            self.fallback_observed_at = None;
            self.clear_agent_name();
        }
        self.hook_authority = None;
        if !preserve_foreign_persisted_session {
            self.persisted_agent_session = None;
        }
        Some(TerminalStateMutation {
            effective_state_change: self.recompute_effective_state(previous, now),
            session_ref_changed: self.session_identity_changed_since(previous_session.as_ref()),
            agent_released: !process_owns_agent,
        })
    }

    /// Whether a lifecycle authority still has matching live process evidence.
    pub(in crate::terminal) fn hook_authority_is_effective(
        &self,
        authority: &HookAuthority,
    ) -> bool {
        !maestro_detection::agents::full_lifecycle_hook_authority(
            &authority.source,
            &authority.agent_label,
        ) || maestro_detection::agents::parse_agent_label(&authority.agent_label).is_none_or(
            |agent| self.detected_agent == Some(agent) && self.recent_agent_process_exit.is_none(),
        )
    }

    /// Select effective hook identity before live detected identity.
    #[must_use]
    pub fn effective_agent_label(&self) -> Option<&str> {
        self.hook_authority
            .as_ref()
            .filter(|authority| self.hook_authority_is_effective(authority))
            .map(|authority| authority.agent_label.as_str())
            .or_else(|| {
                self.recent_agent_process_exit
                    .is_none()
                    .then(|| {
                        self.detected_agent
                            .map(maestro_detection::agents::agent_label)
                    })
                    .flatten()
            })
    }

    /// Resolve the effective identity to a catalogue entry when known.
    #[must_use]
    pub fn effective_known_agent(&self) -> Option<Agent> {
        self.effective_agent_label()
            .and_then(maestro_detection::agents::parse_agent_label)
    }

    /// Project the current state into both sides of an unchanged mutation record.
    #[must_use]
    pub fn unchanged_effective_state_change_at(&self, now: Instant) -> EffectiveStateChange {
        let agent_label = self.effective_agent_label().map(str::to_string);
        let known_agent = self.effective_known_agent();
        let state = self.state;
        let presentation = self.effective_presentation_for_state_at(state, now);
        EffectiveStateChange {
            previous_agent_label: agent_label.clone(),
            previous_known_agent: known_agent,
            previous_state: state,
            previous_presentation: presentation.clone(),
            agent_label,
            known_agent,
            state,
            presentation,
        }
    }

    /// Whether a full-lifecycle integration currently owns effective state.
    #[must_use]
    pub fn full_lifecycle_hook_authority_active(&self) -> bool {
        self.live_full_lifecycle_hook_authority()
    }

    /// Whether fresh same-agent screen evidence overrides a non-lifecycle hook state.
    pub(in crate::terminal) fn visible_blocker_overrides_hook(&self) -> bool {
        if self.live_full_lifecycle_hook_authority() {
            return false;
        }
        self.fallback_visible_blocker
            && self.fallback_not_older_than_hook()
            && self.hook_authority.as_ref().is_some_and(|authority| {
                authority.state != AgentState::Blocked
                    && maestro_detection::agents::parse_agent_label(&authority.agent_label)
                        == self.detected_agent
            })
    }

    /// Whether effective hook authority belongs to a lifecycle integration.
    pub(in crate::terminal) fn live_full_lifecycle_hook_authority(&self) -> bool {
        self.hook_authority.as_ref().is_some_and(|authority| {
            self.hook_authority_is_effective(authority)
                && maestro_detection::agents::full_lifecycle_hook_authority(
                    &authority.source,
                    &authority.agent_label,
                )
        })
    }
}
