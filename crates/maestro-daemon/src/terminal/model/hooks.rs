//! Hooks operations on the retained state.
#[cfg(test)]
use super::EffectiveStateChange;
use super::{
    Agent, FullLifecycleHookReportRoute, FullLifecycleHookSuppressionReason, HookAuthority,
    HookReport, Instant, SuppressedFullLifecycleHookReport, TerminalState, TerminalStateMutation,
};
impl TerminalState {
    /// Apply a hook report and return any effective presentation change.
    #[cfg(test)]
    pub fn set_hook_authority(&mut self, report: HookReport) -> Option<EffectiveStateChange> {
        self.set_hook_authority_at(report, Instant::now())
            .and_then(|mutation| mutation.effective_state_change)
    }

    /// Admit a hook report with an optional validated session identity.
    pub fn set_hook_authority_with_session_ref(
        &mut self,
        report: HookReport,
    ) -> Option<TerminalStateMutation> {
        self.set_hook_authority_at(report, Instant::now())
    }

    /// Admit a hook report into the canonical owner domain at its report time.
    pub fn set_hook_authority_at(
        &mut self,
        report: HookReport,
        now: Instant,
    ) -> Option<TerminalStateMutation> {
        let mut report = report;
        report.source = maestro_detection::agents::normalize_agent_source(report.source);
        let (session_ref, reanchor_sequence, foreground_takeover_allowed) =
            self.admit_hook_report(&report, now)?;
        let HookReport {
            source,
            agent_label,
            state,
            message,
            ..
        } = report;
        let previous = self.previous_effective_state_at(now);
        let previous_session = self.session_identity_snapshot();
        self.reconcile_agent_name_owner(&agent_label, session_ref.as_ref());
        if foreground_takeover_allowed {
            self.suppress_current_full_lifecycle_hook_authority(
                FullLifecycleHookSuppressionReason::HookClear,
            );
        }
        if (session_ref.is_some() || reanchor_sequence)
            && let Some(suppressed) = self.suppressed_full_lifecycle_hook_reports.remove(&source)
            && let Some(suppressed_ref) = suppressed.session_ref
        {
            self.remember_stale_full_lifecycle_hook_session(
                source.clone(),
                suppressed.agent_label,
                suppressed_ref,
            );
        }
        self.persisted_agent_session = None;
        self.hook_authority = Some(HookAuthority {
            source,
            agent_label,
            state,
            message,
            reported_at: now,
            session_ref,
        });
        Some(TerminalStateMutation {
            effective_state_change: self.recompute_effective_state(previous, now),
            session_ref_changed: self.session_identity_changed_since(previous_session.as_ref()),
            agent_released: false,
        })
    }

    /// Check routing, session ownership and freshness before applying live authority.
    fn admit_hook_report(
        &mut self,
        report: &HookReport,
        now: Instant,
    ) -> Option<(Option<crate::resume::AgentSessionRef>, bool, bool)> {
        let HookReport {
            source,
            agent_label,
            session_ref,
            seq,
            ..
        } = report;

        if maestro_detection::agents::session_identity_only_integration(source, agent_label) {
            return None;
        }
        if !maestro_detection::agents::full_lifecycle_hook_authority(source, agent_label)
            && self.recent_agent_process_exit.is_some_and(|exit| {
                maestro_detection::agents::parse_agent_label(agent_label) == Some(exit.agent)
            })
        {
            return None;
        }
        let reanchor_sequence = match self.route_full_lifecycle_hook_report(report, now) {
            FullLifecycleHookReportRoute::Accept { reanchor_sequence } => reanchor_sequence,
            FullLifecycleHookReportRoute::Ignore => return None,
        };
        if self.known_agent_label_conflicts_with_detected_agent(agent_label) {
            return None;
        }
        let owner_conflicts = self.current_session_owner_conflicts(source, agent_label);
        let foreground_takeover_allowed = owner_conflicts
            && self.foreground_agent_confirms_hook_authority_takeover(
                source,
                agent_label,
                session_ref.as_ref(),
            );
        if owner_conflicts && !foreground_takeover_allowed {
            return None;
        }
        let session_ref = session_ref.clone().map(|session_ref| {
            if self.lifecycle_hook_report_replaces_persisted_session(
                source,
                agent_label,
                &session_ref,
            ) {
                session_ref
            } else {
                self.conflicting_same_owner_session_ref(source, agent_label, &session_ref, None)
                    .map_or(session_ref, Clone::clone)
            }
        });
        if self.live_full_lifecycle_hook_authority_conflicts_with_session(
            source,
            agent_label,
            session_ref.as_ref(),
        ) {
            return None;
        }
        if reanchor_sequence {
            self.hook_report_sequences.remove(source);
        }
        if !self.accept_hook_report(source, *seq) {
            return None;
        }

        Some((session_ref, reanchor_sequence, foreground_takeover_allowed))
    }

    /// Whether current hook evidence does not postdate the supplied observation.
    pub(in crate::terminal) fn hook_authority_not_newer_than(&self, observed_at: Instant) -> bool {
        self.hook_authority
            .as_ref()
            .is_none_or(|authority| authority.reported_at <= observed_at)
    }

    /// Whether screen evidence is at least as recent as current hook evidence.
    pub(in crate::terminal) fn fallback_not_older_than_hook(&self) -> bool {
        self.hook_authority.as_ref().is_none_or(|authority| {
            self.fallback_observed_at
                .is_some_and(|observed_at| authority.reported_at <= observed_at)
        })
    }

    /// Whether a known hook identity differs from the detected foreground agent.
    pub(in crate::terminal) fn hook_authority_conflicts_with_detected_agent(
        &self,
        detected_agent: Option<Agent>,
    ) -> bool {
        let Some(detected_agent) = detected_agent else {
            return false;
        };
        self.hook_authority.as_ref().is_some_and(|authority| {
            maestro_detection::agents::parse_agent_label(&authority.agent_label)
                .is_some_and(|hook_agent| hook_agent != detected_agent)
        })
    }

    /// Whether a live lifecycle owner suppresses fallback detection.
    pub(in crate::terminal) fn should_ignore_detected_state_under_full_lifecycle_hook(
        &self,
        detected_agent: Option<Agent>,
        process_exited: bool,
    ) -> bool {
        self.live_full_lifecycle_hook_authority()
            && !process_exited
            && !self.hook_authority_conflicts_with_detected_agent(detected_agent)
    }

    /// Whether retained session provenance and agent match the supplied owner.
    pub(in crate::terminal) fn persisted_agent_session_matches(
        &self,
        source: &str,
        agent: &str,
    ) -> bool {
        self.persisted_agent_session
            .as_ref()
            .is_some_and(|session| session.source == source && session.agent == agent)
    }

    /// Suppress the current lifecycle owner until its generation is reconciled.
    pub(in crate::terminal) fn suppress_current_full_lifecycle_hook_authority(
        &mut self,
        reason: FullLifecycleHookSuppressionReason,
    ) {
        if let Some((source, agent_label, session_ref)) =
            self.hook_authority.as_ref().and_then(|authority| {
                maestro_detection::agents::full_lifecycle_hook_authority(
                    &authority.source,
                    &authority.agent_label,
                )
                .then(|| {
                    (
                        authority.source.clone(),
                        authority.agent_label.clone(),
                        authority.session_ref.clone(),
                    )
                })
            })
        {
            self.suppressed_full_lifecycle_hook_reports.insert(
                source,
                SuppressedFullLifecycleHookReport::new(
                    agent_label,
                    session_ref,
                    reason,
                    Instant::now(),
                ),
            );
        }
    }

    /// Suppress reports for a source while preserving its current session identity.
    pub(in crate::terminal) fn suppress_full_lifecycle_hook_report(
        &mut self,
        source: &str,
        agent_label: &str,
        reason: FullLifecycleHookSuppressionReason,
    ) {
        if maestro_detection::agents::full_lifecycle_hook_authority(source, agent_label) {
            let session_ref = self
                .hook_authority
                .as_ref()
                .and_then(|authority| authority.session_ref.clone());
            self.suppressed_full_lifecycle_hook_reports.insert(
                source.to_string(),
                SuppressedFullLifecycleHookReport::new(
                    agent_label.to_string(),
                    session_ref,
                    reason,
                    Instant::now(),
                ),
            );
        }
    }
}
