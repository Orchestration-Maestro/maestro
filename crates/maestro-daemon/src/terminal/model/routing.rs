//! Routing operations on the retained state.
use super::{
    FullLifecycleHookReportRoute, FullLifecycleHookSuppressionReason, HookAuthority, HookReport,
    Instant, PendingFullLifecycleHookReport, SuppressedFullLifecycleHookReport, TerminalState,
};
impl TerminalState {
    /// Admit live lifecycle reports or buffer them until process and session evidence agrees.
    pub(in crate::terminal) fn route_full_lifecycle_hook_report(
        &mut self,
        report: &HookReport,
        reported_at: Instant,
    ) -> FullLifecycleHookReportRoute {
        let HookReport {
            source,
            agent_label,
            session_ref,
            ..
        } = report;
        if !maestro_detection::agents::full_lifecycle_hook_authority(source, agent_label) {
            return FullLifecycleHookReportRoute::Accept {
                reanchor_sequence: false,
            };
        }
        if self.full_lifecycle_hook_report_matches_stale_session(
            source,
            agent_label,
            session_ref.as_ref(),
        ) {
            return FullLifecycleHookReportRoute::Ignore;
        }
        if let Some(route) = self.anchored_lifecycle_hook_route(report) {
            return route;
        }
        self.buffer_lifecycle_hook_report(report, reported_at)
    }

    /// Resolve already anchored sessions, selection cross-talk and explicitly cleared owners.
    fn anchored_lifecycle_hook_route(
        &self,
        report: &HookReport,
    ) -> Option<FullLifecycleHookReportRoute> {
        let HookReport {
            source,
            agent_label,
            session_ref,
            ..
        } = report;
        let known_agent = maestro_detection::agents::parse_agent_label(agent_label);
        let process_present = known_agent.is_some()
            && self.detected_agent == known_agent
            && self.recent_agent_process_exit.is_none();
        let anchored_session_ref = self
            .hook_authority
            .as_ref()
            .filter(|authority| {
                authority.source == *source && authority.agent_label == *agent_label
            })
            .and_then(|authority| authority.session_ref.as_ref())
            .or_else(|| {
                self.persisted_agent_session
                    .as_ref()
                    .filter(|session| session.source == *source && session.agent == *agent_label)
                    .map(|session| &session.session_ref)
            });
        let session_anchored = anchored_session_ref.is_some_and(|anchored| {
            session_ref
                .as_ref()
                .is_none_or(|incoming| incoming == anchored)
        });
        let selection_cross_talk = (source.as_str(), agent_label.as_str())
            == ("maestro:opencode", "opencode")
            && process_present
            && anchored_session_ref
                .zip(session_ref.as_ref())
                .is_some_and(|(anchored, incoming)| anchored != incoming);
        if selection_cross_talk {
            return Some(FullLifecycleHookReportRoute::Ignore);
        }
        if let Some(suppressed) = self.suppressed_full_lifecycle_hook_reports.get(source) {
            return Self::suppressed_lifecycle_hook_route(suppressed, report);
        }
        (process_present && session_anchored).then(|| FullLifecycleHookReportRoute::Accept {
            reanchor_sequence: self
                .full_lifecycle_hook_report_has_fresh_session_after_stale_session(
                    source,
                    agent_label,
                    session_ref.as_ref(),
                ),
        })
    }

    /// Permit a new session to reanchor after an explicit authority clear.
    fn suppressed_lifecycle_hook_route(
        suppressed: &SuppressedFullLifecycleHookReport,
        report: &HookReport,
    ) -> Option<FullLifecycleHookReportRoute> {
        if suppressed.agent_label != report.agent_label {
            return Some(FullLifecycleHookReportRoute::Ignore);
        }
        if suppressed.reason != FullLifecycleHookSuppressionReason::HookClear {
            return None;
        }
        Some(
            if suppressed
                .session_ref
                .as_ref()
                .zip(report.session_ref.as_ref())
                .is_some_and(|(previous, incoming)| previous != incoming)
            {
                FullLifecycleHookReportRoute::Accept {
                    reanchor_sequence: true,
                }
            } else {
                FullLifecycleHookReportRoute::Ignore
            },
        )
    }

    /// Retain only the freshest sequenced report until its replacement session is validated.
    fn buffer_lifecycle_hook_report(
        &mut self,
        report: &HookReport,
        reported_at: Instant,
    ) -> FullLifecycleHookReportRoute {
        let HookReport {
            source,
            agent_label,
            state,
            message,
            session_ref,
            seq,
        } = report;
        let Some(session_ref) = session_ref.clone() else {
            return FullLifecycleHookReportRoute::Ignore;
        };
        let Some(seq) = *seq else {
            return FullLifecycleHookReportRoute::Ignore;
        };
        if self
            .hook_report_sequences
            .get(source)
            .is_some_and(|previous| seq <= *previous)
        {
            return FullLifecycleHookReportRoute::Ignore;
        }

        let previous_session_ref = self
            .persisted_agent_session
            .as_ref()
            .filter(|session| session.source == *source && session.agent == *agent_label)
            .map(|session| session.session_ref.clone());
        let suppressed = self
            .suppressed_full_lifecycle_hook_reports
            .entry(source.clone())
            .or_insert_with(|| {
                SuppressedFullLifecycleHookReport::new(
                    agent_label.clone(),
                    previous_session_ref,
                    FullLifecycleHookSuppressionReason::ProcessExit,
                    reported_at,
                )
            });
        let replace_pending = suppressed
            .pending_replacement_report
            .as_ref()
            .is_none_or(|pending| seq > pending.seq);
        if replace_pending {
            suppressed.pending_replacement_report = Some(PendingFullLifecycleHookReport {
                authority: HookAuthority {
                    source: source.clone(),
                    agent_label: agent_label.clone(),
                    state: *state,
                    message: message.clone(),
                    reported_at,
                    session_ref: Some(session_ref),
                },
                seq,
            });
        }
        FullLifecycleHookReportRoute::Ignore
    }

    /// Whether a report refers to a remembered stale session for this source.
    pub(in crate::terminal) fn full_lifecycle_hook_report_matches_stale_session(
        &self,
        source: &str,
        agent_label: &str,
        session_ref: Option<&crate::resume::AgentSessionRef>,
    ) -> bool {
        if !maestro_detection::agents::full_lifecycle_hook_authority(source, agent_label) {
            return false;
        }
        let Some(incoming_ref) = session_ref else {
            return false;
        };
        self.stale_full_lifecycle_hook_sessions
            .get(source)
            .is_some_and(|stale_sessions| {
                stale_sessions.iter().any(|stale| {
                    stale.agent_label == agent_label && incoming_ref == &stale.session_ref
                })
            })
    }

    /// Whether a source has stale sessions but this report identifies a different session.
    pub(in crate::terminal) fn full_lifecycle_hook_report_has_fresh_session_after_stale_session(
        &self,
        source: &str,
        agent_label: &str,
        session_ref: Option<&crate::resume::AgentSessionRef>,
    ) -> bool {
        if !maestro_detection::agents::full_lifecycle_hook_authority(source, agent_label) {
            return false;
        }
        let Some(incoming_ref) = session_ref else {
            return false;
        };
        self.stale_full_lifecycle_hook_sessions
            .get(source)
            .is_some_and(|stale_sessions| {
                stale_sessions
                    .iter()
                    .any(|stale| stale.agent_label == agent_label)
                    && stale_sessions.iter().all(|stale| {
                        stale.agent_label != agent_label || incoming_ref != &stale.session_ref
                    })
            })
    }

    /// Whether a report tries to replace a live same-owner lifecycle session.
    pub(in crate::terminal) fn live_full_lifecycle_hook_authority_conflicts_with_session(
        &self,
        source: &str,
        agent_label: &str,
        session_ref: Option<&crate::resume::AgentSessionRef>,
    ) -> bool {
        let Some(authority) = self.hook_authority.as_ref() else {
            return false;
        };
        if !maestro_detection::agents::full_lifecycle_hook_authority(
            &authority.source,
            &authority.agent_label,
        ) {
            return false;
        }
        if authority.source != source || authority.agent_label != agent_label {
            return false;
        }
        authority
            .session_ref
            .as_ref()
            .zip(session_ref)
            .is_some_and(|(current, incoming)| current != incoming)
    }

    /// Clone a conflicting live session owned by the same lifecycle integration.
    pub(in crate::terminal) fn same_owner_full_lifecycle_hook_authority_session_ref(
        &self,
        source: &str,
        agent_label: &str,
        session_ref: &crate::resume::AgentSessionRef,
    ) -> Option<crate::resume::AgentSessionRef> {
        let authority = self.hook_authority.as_ref()?;
        if !maestro_detection::agents::full_lifecycle_hook_authority(
            &authority.source,
            &authority.agent_label,
        ) || authority.source != source
            || authority.agent_label != agent_label
        {
            return None;
        }
        authority
            .session_ref
            .as_ref()
            .filter(|current| *current != session_ref)
            .cloned()
    }
}
