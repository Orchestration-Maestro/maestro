//! Session operations on the retained state.
use super::{
    FullLifecycleHookSuppressionReason, Instant, SessionStartReport,
    SuppressedFullLifecycleHookReport, TerminalState, TerminalStateMutation,
};
impl TerminalState {
    /// Borrow the live session identity, falling back to the restored identity.
    pub(crate) fn current_session_identity_for_persistence(
        &self,
    ) -> Option<(&str, &str, &crate::resume::AgentSessionRef)> {
        if let Some(authority) = self.hook_authority.as_ref()
            && let Some(session_ref) = authority.session_ref.as_ref()
        {
            return Some((
                authority.source.as_str(),
                authority.agent_label.as_str(),
                session_ref,
            ));
        }
        self.persisted_agent_session.as_ref().map(|session| {
            (
                session.source.as_str(),
                session.agent.as_str(),
                &session.session_ref,
            )
        })
    }

    /// Retain identity only when it must survive a mutation.
    pub(in crate::terminal) fn session_identity_snapshot(
        &self,
    ) -> Option<(String, String, crate::resume::AgentSessionRef)> {
        self.current_session_identity_for_persistence()
            .map(|(source, agent, session_ref)| {
                (source.to_owned(), agent.to_owned(), session_ref.clone())
            })
    }

    /// Compare the current borrowed identity with the retained before-state.
    pub(in crate::terminal) fn session_identity_changed_since(
        &self,
        before: Option<&(String, String, crate::resume::AgentSessionRef)>,
    ) -> bool {
        before.map(|(source, agent, session_ref)| (source.as_str(), agent.as_str(), session_ref))
            != self.current_session_identity_for_persistence()
    }

    /// Whether a retained session belongs to a different source or agent.
    pub(in crate::terminal) fn current_session_owner_conflicts(
        &self,
        source: &str,
        agent_label: &str,
    ) -> bool {
        self.current_session_identity_for_persistence().is_some_and(
            |(current_source, current_agent, _)| {
                current_source != source || current_agent != agent_label
            },
        )
    }

    /// Return an existing ID when the integration does not permit its replacement.
    pub(in crate::terminal) fn conflicting_same_owner_session_ref(
        &self,
        source: &str,
        agent_label: &str,
        session_ref: &crate::resume::AgentSessionRef,
        session_start_source: Option<&str>,
    ) -> Option<&crate::resume::AgentSessionRef> {
        self.current_session_identity_for_persistence().and_then(
            |(current_source, current_agent, current_ref)| {
                (current_source == source
                    && current_agent == agent_label
                    && current_ref.kind == crate::resume::AgentSessionRefKind::Id
                    && session_ref.kind == crate::resume::AgentSessionRefKind::Id
                    && current_ref.value != session_ref.value
                    && !Self::session_report_allows_session_replacement(
                        source,
                        agent_label,
                        session_start_source,
                    ))
                .then_some(current_ref)
            },
        )
    }

    /// Whether a native thread lifecycle report replaces an unclaimed restored thread.
    pub(in crate::terminal) fn lifecycle_hook_report_replaces_persisted_session(
        &self,
        source: &str,
        agent_label: &str,
        session_ref: &crate::resume::AgentSessionRef,
    ) -> bool {
        self.hook_authority.is_none()
            && (source, agent_label) == ("maestro:mastracode", "mastracode")
            && self
                .persisted_agent_session
                .as_ref()
                .is_some_and(|session| {
                    session.source == source
                        && session.agent == agent_label
                        && session.session_ref.kind == crate::resume::AgentSessionRefKind::Id
                        && session_ref.kind == crate::resume::AgentSessionRefKind::Id
                        && session.session_ref.value != session_ref.value
                })
    }

    /// Match each integration to the lifecycle reasons that permit session replacement.
    pub(in crate::terminal) fn session_report_allows_session_replacement(
        source: &str,
        agent_label: &str,
        session_start_source: Option<&str>,
    ) -> bool {
        matches!(
            (source, agent_label, session_start_source),
            (
                "maestro:claude",
                "claude",
                Some("clear" | "resume" | "compact")
            ) | (
                "maestro:codex",
                "codex",
                Some("startup" | "clear" | "resume" | "compact")
            ) | ("maestro:mastracode", "mastracode", Some("startup"))
                | (
                    "maestro:hermes",
                    "hermes",
                    Some("startup" | "new" | "resume")
                )
                | ("maestro:opencode", "opencode", Some("select"))
                | ("maestro:pi", "pi", Some("new" | "resume" | "fork"))
                | (
                    "maestro:omp",
                    "omp",
                    Some("startup" | "new" | "resume" | "fork")
                )
                | (
                    "maestro:qwen",
                    "qwen",
                    Some("startup" | "clear" | "resume" | "compact" | "branch")
                )
                | ("maestro:antigravity_cli", "agy", None)
        )
    }

    /// Whether a start reason can anchor a lifecycle session.
    pub(in crate::terminal) fn session_start_source_is_recognized(
        session_start_source: Option<&str>,
    ) -> bool {
        matches!(
            session_start_source,
            Some("startup" | "clear" | "resume" | "compact" | "new" | "fork" | "select")
        )
    }

    /// Whether this is a native unsequenced session-selection report.
    pub(in crate::terminal) fn is_unsequenced_opencode_selection(
        source: &str,
        agent_label: &str,
        session_start_source: Option<&str>,
        seq: Option<u64>,
    ) -> bool {
        (source, agent_label, session_start_source, seq)
            == ("maestro:opencode", "opencode", Some("select"), None)
    }

    /// Retain restored session identity with canonical source provenance.
    pub fn set_persisted_agent_session(&mut self, session: crate::resume::PersistedAgentSession) {
        let mut session = session;
        session.source = maestro_detection::agents::normalize_agent_source(session.source);
        self.persisted_agent_session = Some(session);
    }

    /// Apply a session identity report without a lifecycle start reason.
    pub fn set_agent_session_ref(
        &mut self,
        source: String,
        agent_label: String,
        session_ref: Option<crate::resume::AgentSessionRef>,
        seq: Option<u64>,
    ) -> Option<TerminalStateMutation> {
        self.set_agent_session_ref_for_session_start(SessionStartReport {
            source,
            agent_label,
            session_ref,
            seq,
            session_start_source: None,
        })
    }

    /// Arbitrate session-start identity, generation evidence and source-local freshness.
    pub fn set_agent_session_ref_for_session_start(
        &mut self,
        report: SessionStartReport,
    ) -> Option<TerminalStateMutation> {
        let SessionStartReport {
            source,
            agent_label,
            session_ref,
            seq,
            session_start_source,
        } = report;
        let session = crate::resume::PersistedAgentSession {
            source: maestro_detection::agents::normalize_agent_source(source),
            agent: agent_label,
            session_ref: session_ref?,
        };
        let process_present = self.session_process_present(&session.agent);
        let full_lifecycle_source = maestro_detection::agents::full_lifecycle_hook_authority(
            &session.source,
            &session.agent,
        );
        let generation_gated = self
            .suppressed_full_lifecycle_hook_reports
            .get(&session.source)
            .is_some_and(|suppressed| {
                suppressed.agent_label == session.agent
                    && suppressed.reason != FullLifecycleHookSuppressionReason::HookClear
            });
        let session_anchored = self.hook_authority.as_ref().is_some_and(|authority| {
            authority.source == session.source
                && authority.agent_label == session.agent
                && authority.session_ref.is_some()
        }) || self
            .persisted_agent_session_matches(&session.source, &session.agent);
        let unsequenced_selection = Self::is_unsequenced_opencode_selection(
            &session.source,
            &session.agent,
            session_start_source.as_deref(),
            seq,
        );
        let selection_can_reconcile = unsequenced_selection && process_present;
        if selection_can_reconcile {
            self.suppressed_full_lifecycle_hook_reports
                .remove(&session.source);
        } else if full_lifecycle_source && unsequenced_selection {
            self.buffer_unsequenced_selection(session);
            return None;
        }
        if full_lifecycle_source
            && !selection_can_reconcile
            && (!process_present || generation_gated || !session_anchored)
        {
            if !Self::session_start_source_is_recognized(session_start_source.as_deref()) {
                return None;
            }
            return self.buffer_session_start(session, seq?, process_present);
        }
        self.apply_session_start(session, seq, session_start_source.as_deref())
    }

    /// Whether a session identity has matching foreground process evidence.
    fn session_process_present(&self, agent_label: &str) -> bool {
        let known_agent = maestro_detection::agents::parse_agent_label(agent_label);
        known_agent.is_some()
            && self.detected_agent == known_agent
            && self.recent_agent_process_exit.is_none()
    }

    /// Retain a native session selection until its foreground process is observed.
    fn buffer_unsequenced_selection(&mut self, session: crate::resume::PersistedAgentSession) {
        let previous_session_ref = self
            .hook_authority
            .as_ref()
            .filter(|authority| {
                authority.source == session.source && authority.agent_label == session.agent
            })
            .and_then(|authority| authority.session_ref.clone())
            .or_else(|| {
                self.persisted_agent_session
                    .as_ref()
                    .filter(|current| {
                        current.source == session.source && current.agent == session.agent
                    })
                    .map(|current| current.session_ref.clone())
            });
        let suppressed = self
            .suppressed_full_lifecycle_hook_reports
            .entry(session.source)
            .or_insert_with(|| {
                SuppressedFullLifecycleHookReport::new(
                    session.agent,
                    previous_session_ref,
                    FullLifecycleHookSuppressionReason::ProcessExit,
                    Instant::now(),
                )
            });
        suppressed.replacement_session_ref = Some(session.session_ref);
        suppressed.pending_replacement_report = None;
    }

    /// Validate a fresh start sequence and replay it only when process evidence is present.
    fn buffer_session_start(
        &mut self,
        session: crate::resume::PersistedAgentSession,
        seq: u64,
        process_present: bool,
    ) -> Option<TerminalStateMutation> {
        if self
            .hook_report_sequences
            .get(&session.source)
            .is_some_and(|previous| seq <= *previous)
        {
            return None;
        }
        let now = Instant::now();
        let previous = self.previous_effective_state_at(now);
        let previous_session = self.session_identity_snapshot();
        let known_agent = maestro_detection::agents::parse_agent_label(&session.agent);
        let suppressed = self
            .suppressed_full_lifecycle_hook_reports
            .entry(session.source.clone())
            .or_insert_with(|| {
                SuppressedFullLifecycleHookReport::new(
                    session.agent,
                    None,
                    FullLifecycleHookSuppressionReason::ProcessExit,
                    now,
                )
            });
        if suppressed.replacement_session_ref.as_ref() != Some(&session.session_ref) {
            if suppressed
                .pending_replacement_report
                .as_ref()
                .is_some_and(|pending| {
                    pending.authority.session_ref.as_ref() != Some(&session.session_ref)
                })
            {
                suppressed.pending_replacement_report = None;
            }
            suppressed.replacement_session_ref = Some(session.session_ref);
        }
        self.hook_report_sequences.insert(session.source, seq);
        if !process_present {
            return None;
        }
        self.clear_full_lifecycle_hook_suppression_for_detected_agent(None, known_agent);
        Some(TerminalStateMutation {
            effective_state_change: self.recompute_effective_state(previous, now),
            session_ref_changed: self.session_identity_changed_since(previous_session.as_ref()),
            agent_released: false,
        })
    }

    /// Check identity replacement permissions before changing live or durable ownership.
    fn admit_session_start(
        &self,
        session: &crate::resume::PersistedAgentSession,
        session_start_source: Option<&str>,
    ) -> Option<(bool, bool, Option<crate::resume::AgentSessionRef>)> {
        let crate::resume::PersistedAgentSession {
            source,
            agent: agent_label,
            session_ref,
        } = session;
        let session_replacement_allowed = Self::session_report_allows_session_replacement(
            source,
            agent_label,
            session_start_source,
        );
        let replacing_identity_only_session =
            maestro_detection::agents::session_identity_only_integration(source, agent_label)
                && session_replacement_allowed
                && self.current_session_identity_for_persistence().is_some_and(
                    |(current_source, current_agent, current_ref)| {
                        current_source == source
                            && current_agent == agent_label
                            && current_ref.kind == crate::resume::AgentSessionRefKind::Id
                            && session_ref.kind == crate::resume::AgentSessionRefKind::Id
                            && current_ref.value != session_ref.value
                    },
                );
        if replacing_identity_only_session && !self.session_process_present(agent_label) {
            return None;
        }
        let owner_conflicts = self.current_session_owner_conflicts(source, agent_label);
        let foreground_takeover_allowed = owner_conflicts
            && self.foreground_agent_confirms_different_owner_takeover(
                source,
                agent_label,
                session_ref,
                session_start_source,
            );
        if owner_conflicts && !foreground_takeover_allowed {
            return None;
        }
        if self
            .conflicting_same_owner_session_ref(
                source,
                agent_label,
                session_ref,
                session_start_source,
            )
            .is_some()
        {
            return None;
        }
        let replaced_hook_session = self.same_owner_full_lifecycle_hook_authority_session_ref(
            source,
            agent_label,
            session_ref,
        );
        if replaced_hook_session.is_some() && !session_replacement_allowed {
            return None;
        }
        Some((
            session_replacement_allowed,
            foreground_takeover_allowed,
            replaced_hook_session,
        ))
    }

    /// Apply admitted session replacement and report its effective and durable changes.
    fn apply_session_start(
        &mut self,
        session: crate::resume::PersistedAgentSession,
        seq: Option<u64>,
        session_start_source: Option<&str>,
    ) -> Option<TerminalStateMutation> {
        if !Self::is_unsequenced_opencode_selection(
            &session.source,
            &session.agent,
            session_start_source,
            seq,
        ) && !self.accept_hook_report(&session.source, seq)
        {
            return None;
        }
        if self.known_agent_label_conflicts_with_detected_agent(&session.agent) {
            return None;
        }
        let (session_replacement_allowed, foreground_takeover_allowed, replaced_hook_session) =
            self.admit_session_start(&session, session_start_source)?;
        let now = Instant::now();
        let previous = self.previous_effective_state_at(now);
        let previous_session = self.session_identity_snapshot();
        if session_replacement_allowed || foreground_takeover_allowed {
            self.forget_stale_full_lifecycle_hook_session(
                &session.source,
                &session.agent,
                &session.session_ref,
            );
        }
        if let Some(replaced_hook_session) = replaced_hook_session {
            self.remember_stale_full_lifecycle_hook_session(
                session.source.clone(),
                session.agent.clone(),
                replaced_hook_session,
            );
            self.hook_authority = None;
        } else if foreground_takeover_allowed {
            self.suppress_current_full_lifecycle_hook_authority(
                FullLifecycleHookSuppressionReason::HookClear,
            );
            self.hook_authority = None;
        }
        self.reconcile_agent_name_owner(&session.agent, Some(&session.session_ref));
        self.persisted_agent_session = Some(session);
        Some(TerminalStateMutation {
            effective_state_change: self.recompute_effective_state(previous, now),
            session_ref_changed: self.session_identity_changed_since(previous_session.as_ref()),
            agent_released: false,
        })
    }
}
