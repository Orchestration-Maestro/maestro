//! Suppression operations on the retained state.
use super::{
    Agent, FullLifecycleHookSuppressionReason, Instant, StaleFullLifecycleHookSession,
    TerminalState,
};
impl TerminalState {
    /// Reconcile replacement sessions and replay eligible reports after fresh process evidence.
    pub(in crate::terminal) fn clear_full_lifecycle_hook_suppression_for_detected_agent(
        &mut self,
        previous_detected_agent: Option<Agent>,
        detected_agent: Option<Agent>,
    ) {
        let Some(detected_agent) = detected_agent else {
            return;
        };
        if previous_detected_agent == Some(detected_agent) {
            return;
        }
        let detected_label = maestro_detection::agents::agent_label(detected_agent);
        let mut stale_sessions = Vec::new();
        let mut validated_replacement_sessions = Vec::new();
        self.suppressed_full_lifecycle_hook_reports
            .retain(|source, suppressed| {
                let should_clear =
                    maestro_detection::agents::parse_agent_label(&suppressed.agent_label)
                        == Some(detected_agent);
                if !should_clear {
                    return true;
                }
                let process_exited =
                    suppressed.reason == FullLifecycleHookSuppressionReason::ProcessExit;
                let replacement = process_exited
                    .then(|| suppressed.replacement_session_ref.take())
                    .flatten();
                if process_exited && replacement.is_none() {
                    return true;
                }
                stale_sessions.extend(
                    suppressed
                        .stale_session(replacement.as_ref())
                        .map(|stale| (source.clone(), stale)),
                );
                let Some(session_ref) = replacement else {
                    return false;
                };
                let session_start_seq = self.hook_report_sequences.get(source).copied();
                let pending = suppressed
                    .pending_replacement_report
                    .take()
                    .filter(|pending| {
                        pending.authority.session_ref.as_ref() == Some(&session_ref)
                            && session_start_seq.is_none_or(|seq| pending.seq > seq)
                    });
                validated_replacement_sessions.push((
                    crate::agent_resume::PersistedAgentSession {
                        source: source.clone(),
                        agent: suppressed.agent_label.clone(),
                        session_ref,
                    },
                    pending,
                ));
                false
            });
        for (source, stale_session) in stale_sessions {
            self.remember_stale_full_lifecycle_hook_session(
                source,
                stale_session.agent_label,
                stale_session.session_ref,
            );
        }
        self.replay_validated_replacement_sessions(validated_replacement_sessions, detected_label);
    }

    /// Release sequence suppression and replay only reports matching validated replacement sessions.
    fn replay_validated_replacement_sessions(
        &mut self,
        validated: Vec<(
            crate::agent_resume::PersistedAgentSession,
            Option<super::PendingFullLifecycleHookReport>,
        )>,
        detected_label: &str,
    ) {
        self.hook_report_sequences.retain(|source, _| {
            validated
                .iter()
                .any(|(session, _)| &session.source == source)
                || !maestro_detection::agents::full_lifecycle_hook_authority(source, detected_label)
        });
        for (session, pending) in validated {
            self.forget_stale_full_lifecycle_hook_session(
                &session.source,
                &session.agent,
                &session.session_ref,
            );
            self.reconcile_agent_name_owner(&session.agent, Some(&session.session_ref));
            let source = session.source.clone();
            self.persisted_agent_session = Some(session);
            if let Some(pending) = pending {
                self.hook_report_sequences.insert(source, pending.seq);
                self.hook_authority = Some(pending.authority);
            }
        }
    }

    /// Remember a stale source-local session without duplicating it.
    pub(in crate::terminal) fn remember_stale_full_lifecycle_hook_session(
        &mut self,
        source: String,
        agent_label: String,
        session_ref: crate::agent_resume::AgentSessionRef,
    ) {
        let stale_session = StaleFullLifecycleHookSession {
            agent_label,
            session_ref,
        };
        let source_stale_sessions = self
            .stale_full_lifecycle_hook_sessions
            .entry(source)
            .or_default();
        if !source_stale_sessions
            .iter()
            .any(|existing| existing == &stale_session)
        {
            source_stale_sessions.push(stale_session);
        }
    }

    /// Forget an explicitly reactivated session and remove empty source buckets.
    pub(in crate::terminal) fn forget_stale_full_lifecycle_hook_session(
        &mut self,
        source: &str,
        agent_label: &str,
        session_ref: &crate::agent_resume::AgentSessionRef,
    ) {
        let remove_source = self
            .stale_full_lifecycle_hook_sessions
            .get_mut(source)
            .is_some_and(|stale_sessions| {
                stale_sessions.retain(|stale| {
                    stale.agent_label != agent_label || &stale.session_ref != session_ref
                });
                stale_sessions.is_empty()
            });
        if remove_source {
            self.stale_full_lifecycle_hook_sessions.remove(source);
        }
    }

    /// Whether a detected observation predates or coincides with release suppression.
    pub(in crate::terminal) fn detected_state_observed_before_release_suppression(
        &self,
        detected_agent: Option<Agent>,
        observed_at: Instant,
    ) -> bool {
        let Some(detected_agent) = detected_agent else {
            return false;
        };
        self.suppressed_full_lifecycle_hook_reports
            .values()
            .any(|suppressed| {
                maestro_detection::agents::parse_agent_label(&suppressed.agent_label)
                    == Some(detected_agent)
                    && observed_at <= suppressed.observed_at
            })
    }
}
