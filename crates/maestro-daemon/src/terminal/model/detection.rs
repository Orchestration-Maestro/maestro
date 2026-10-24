//! Detection operations on the retained state.
use super::{
    Agent, AgentState, DetectedStateUpdate, FullLifecycleHookSuppressionReason, Instant,
    RecentAgentProcessExit, SuppressedFullLifecycleHookReport, TerminalState,
    TerminalStateMutation,
};
#[cfg(test)]
use super::{AgentVisibility, EffectiveStateChange};
impl TerminalState {
    /// Apply a fallback detection without visible screen signals.
    #[cfg(test)]
    pub fn set_detected_state(
        &mut self,
        agent: Option<Agent>,
        fallback_state: AgentState,
    ) -> Option<EffectiveStateChange> {
        self.set_detected_state_with_mutation(agent, fallback_state)
            .effective_state_change
    }

    /// Apply fallback detection and return all mutation flags.
    #[cfg(test)]
    pub fn set_detected_state_with_mutation(
        &mut self,
        agent: Option<Agent>,
        fallback_state: AgentState,
    ) -> TerminalStateMutation {
        self.set_detected_state_with_screen_signals_at(
            DetectedStateUpdate {
                agent,
                fallback_state,
                visible: AgentVisibility {
                    visible_blocker: false,
                    visible_idle: false,
                    visible_working: false,
                },
                process_exited: false,
            },
            Instant::now(),
        )
    }

    /// Arbitrate supplied screen and process evidence at its observation time.
    pub fn set_detected_state_with_screen_signals_at(
        &mut self,
        update: DetectedStateUpdate,
        now: Instant,
    ) -> TerminalStateMutation {
        let previous = self.previous_effective_state_at(now);
        let previous_session = self.session_identity_snapshot();
        let agent_released = self.apply_detected_state_at(update, now);
        TerminalStateMutation {
            effective_state_change: self.recompute_effective_state(previous, now),
            session_ref_changed: self.session_identity_changed_since(previous_session.as_ref()),
            agent_released,
        }
    }

    /// Apply detection arbitration without changing the mutation's before-state snapshot.
    fn apply_detected_state_at(&mut self, update: DetectedStateUpdate, now: Instant) -> bool {
        let DetectedStateUpdate {
            agent,
            fallback_state: _,
            visible: _,
            process_exited,
        } = update;
        let previous_detected_agent = self.detected_agent;
        let newer_custom_authority = process_exited && self.custom_authority_newer_than(agent, now);
        let agent_released = process_exited
            && !newer_custom_authority
            && (self.effective_agent_label().is_some() || self.agent_name.is_some());
        if self.should_ignore_detected_state_under_full_lifecycle_hook(agent, process_exited) {
            if self.hook_authority.as_ref().and_then(|authority| {
                maestro_detection::agents::parse_agent_label(&authority.agent_label)
            }) == agent
            {
                self.detected_agent = agent;
            }
            return false;
        }
        let replacement_process_detected = !process_exited
            && agent.is_some()
            && self
                .recent_agent_process_exit
                .is_some_and(|exit| Some(exit.agent) == agent && exit.observed_at < now);
        if !process_exited && self.detected_state_observed_before_release_suppression(agent, now) {
            return false;
        }
        self.apply_detected_observation(
            update,
            previous_detected_agent,
            now,
            replacement_process_detected,
        );
        if process_exited {
            self.reset_exited_generation(agent, now);
            self.suppress_exited_official_session(agent, now);
            self.clear_exited_hook_identity(agent, newer_custom_authority);
            if let Some(agent) = agent {
                self.clear_exited_metadata(agent, now);
            }
        }
        self.clear_conflicting_detected_authority(agent, previous_detected_agent, now);
        if agent_released {
            self.clear_agent_name();
        }
        agent_released
    }
    /// Discard unclaimed reports and remember the sessions of an exited generation.
    fn reset_exited_generation(&mut self, agent: Option<Agent>, now: Instant) {
        let mut reset_sources = Vec::new();
        let mut stale_sessions = Vec::new();
        for (source, suppressed) in &mut self.suppressed_full_lifecycle_hook_reports {
            if maestro_detection::agents::parse_agent_label(&suppressed.agent_label) != agent
                || suppressed.reason == FullLifecycleHookSuppressionReason::HookClear
            {
                continue;
            }
            let exited_session_ref = suppressed
                .replacement_session_ref
                .take()
                .or_else(|| {
                    suppressed
                        .pending_replacement_report
                        .as_ref()
                        .and_then(|pending| pending.authority.session_ref.clone())
                })
                .or_else(|| suppressed.session_ref.clone());
            if let (Some(previous), Some(exited)) =
                (suppressed.session_ref.as_ref(), exited_session_ref.as_ref())
                && previous != exited
            {
                stale_sessions.push((
                    source.clone(),
                    suppressed.agent_label.clone(),
                    previous.clone(),
                ));
            }
            suppressed.session_ref = exited_session_ref;
            suppressed.pending_replacement_report = None;
            suppressed.observed_at = now;
            reset_sources.push(source.clone());
        }
        for (source, agent_label, session_ref) in stale_sessions {
            self.remember_stale_full_lifecycle_hook_session(source, agent_label, session_ref);
        }
        for source in reset_sources {
            self.hook_report_sequences.remove(&source);
        }
    }

    /// Suppress the official session observed at process exit.
    fn suppress_exited_official_session(&mut self, agent: Option<Agent>, now: Instant) {
        let official_session = self
            .hook_authority
            .as_ref()
            .filter(|authority| {
                crate::resume::is_official_agent_source(&authority.source, &authority.agent_label)
                    && maestro_detection::agents::parse_agent_label(&authority.agent_label) == agent
            })
            .map(|authority| {
                (
                    authority.source.clone(),
                    authority.agent_label.clone(),
                    authority.session_ref.clone(),
                )
            })
            .or_else(|| {
                self.persisted_agent_session
                    .as_ref()
                    .filter(|session| {
                        crate::resume::is_official_agent_source(&session.source, &session.agent)
                            && maestro_detection::agents::parse_agent_label(&session.agent) == agent
                    })
                    .map(|session| {
                        (
                            session.source.clone(),
                            session.agent.clone(),
                            Some(session.session_ref.clone()),
                        )
                    })
            });
        if let Some((source, agent_label, session_ref)) = official_session {
            self.hook_report_sequences.remove(&source);
            self.suppressed_full_lifecycle_hook_reports.insert(
                source,
                SuppressedFullLifecycleHookReport::new(
                    agent_label,
                    session_ref,
                    FullLifecycleHookSuppressionReason::ProcessExit,
                    now,
                ),
            );
        }
    }

    /// Clear matching hook and persisted identity unless a newer custom report owns it.
    fn clear_exited_hook_identity(&mut self, agent: Option<Agent>, newer_custom_authority: bool) {
        let cleared_hook_source = self.hook_authority.as_ref().and_then(|authority| {
            (maestro_detection::agents::parse_agent_label(&authority.agent_label) == agent
                && !newer_custom_authority)
                .then(|| authority.source.clone())
        });
        if let Some(source) = cleared_hook_source {
            self.hook_report_sequences.remove(&source);
            self.hook_authority = None;
        }
        if !newer_custom_authority
            && self
                .persisted_agent_session
                .as_ref()
                .is_some_and(|session| {
                    maestro_detection::agents::parse_agent_label(&session.agent) == agent
                })
        {
            self.persisted_agent_session = None;
        }
    }

    /// Clear metadata and sequence ownership belonging to the exited generation.
    fn clear_exited_metadata(&mut self, agent: Agent, now: Instant) {
        let agent_label = maestro_detection::agents::agent_label(agent);
        let mut cleared_metadata_sources = Vec::new();
        self.agent_metadata.retain(|source, metadata| {
            let official_metadata =
                crate::resume::is_official_agent_source(&metadata.source, agent_label)
                    || metadata
                        .applies_to_source
                        .as_deref()
                        .is_some_and(|applies_to| {
                            crate::resume::is_official_agent_source(applies_to, agent_label)
                        });
            let matches_agent =
                metadata.agent_label.as_deref() == Some(agent_label) || official_metadata;
            let clear = matches_agent && (official_metadata || metadata.reported_at <= now);
            if clear {
                cleared_metadata_sources.push(source.clone());
            }
            !clear
        });
        for source in cleared_metadata_sources {
            self.metadata_report_sequences.remove(&source);
            self.metadata_report_agents.remove(&source);
            self.metadata_token_sequence_sources.remove(&source);
        }
        let mut exited_generation_sources = Vec::new();
        self.metadata_report_agents.retain(|source, owner| {
            if *owner == agent {
                exited_generation_sources.push(source.clone());
                false
            } else {
                true
            }
        });
        for source in exited_generation_sources {
            self.metadata_report_sequences.remove(&source);
            self.metadata_token_sequence_sources.remove(&source);
        }
    }

    /// Preserve the durable session while removing conflicting live authority.
    fn clear_conflicting_detected_authority(
        &mut self,
        agent: Option<Agent>,
        previous_detected_agent: Option<Agent>,
        now: Instant,
    ) {
        if self.hook_authority_not_newer_than(now)
            && (self.hook_authority_conflicts_with_detected_agent(agent)
                || (previous_detected_agent.is_some()
                    && agent != previous_detected_agent
                    && self.hook_authority.as_ref().is_some_and(|authority| {
                        maestro_detection::agents::parse_agent_label(&authority.agent_label)
                            == previous_detected_agent
                    })))
        {
            let durable_session = self
                .hook_authority
                .as_ref()
                .zip(
                    self.hook_authority
                        .as_ref()
                        .and_then(|authority| authority.session_ref.as_ref()),
                )
                .map(
                    |(authority, session_ref)| crate::resume::PersistedAgentSession {
                        source: authority.source.clone(),
                        agent: authority.agent_label.clone(),
                        session_ref: session_ref.clone(),
                    },
                );
            self.suppress_current_full_lifecycle_hook_authority(
                FullLifecycleHookSuppressionReason::HookClear,
            );
            self.hook_authority = None;
            self.persisted_agent_session = durable_session;
        }
    }

    /// Whether a custom report postdates this process-exit observation.
    fn custom_authority_newer_than(&self, agent: Option<Agent>, now: Instant) -> bool {
        self.hook_authority.as_ref().is_some_and(|authority| {
            maestro_detection::agents::parse_agent_label(&authority.agent_label) == agent
                && !crate::resume::is_official_agent_source(
                    &authority.source,
                    &authority.agent_label,
                )
                && authority.reported_at > now
        })
    }

    /// Retain one detected observation and its fresh-process evidence.
    fn apply_detected_observation(
        &mut self,
        update: DetectedStateUpdate,
        previous_detected_agent: Option<Agent>,
        now: Instant,
        replacement_process_detected: bool,
    ) {
        let DetectedStateUpdate {
            agent,
            fallback_state,
            visible,
            process_exited,
        } = update;
        let visible_blocker = visible.visible_blocker;
        self.detected_agent = agent;
        if let Some(agent) = agent {
            let agent_label = maestro_detection::agents::agent_label(agent);
            self.reconcile_agent_name_owner(agent_label, None);
        }
        if !process_exited {
            self.clear_full_lifecycle_hook_suppression_for_detected_agent(
                if replacement_process_detected {
                    None
                } else {
                    previous_detected_agent
                },
                agent,
            );
        }
        self.fallback_state = fallback_state;
        self.fallback_visible_blocker = visible_blocker && fallback_state == AgentState::Blocked;
        self.fallback_observed_at = Some(now);
        if process_exited {
            if let Some(agent) = agent {
                self.recent_agent_process_exit = Some(RecentAgentProcessExit {
                    agent,
                    observed_at: now,
                });
            }
        } else if agent.is_some() {
            self.recent_agent_process_exit = None;
        }
    }
}
