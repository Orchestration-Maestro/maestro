//! Managed operations on the retained state.
use super::{
    Agent, AgentNameOwner, AgentState, Instant, ManagedAgent, ManagedAgentLaunch,
    ManagedAgentPhase, TerminalState,
};
impl TerminalState {
    /// Trim and retain a nonempty manual border label.
    pub fn set_manual_label(&mut self, label: &str) {
        let label = label.trim().to_string();
        self.manual_label = (!label.is_empty()).then_some(label);
    }

    /// Clear the manual border label.
    pub fn clear_manual_label(&mut self) {
        self.manual_label = None;
    }

    /// Retain an untrimmed nonempty name with its current session owner.
    pub fn set_agent_name(&mut self, name: String) {
        self.agent_name = (!name.is_empty()).then_some(name);
        self.agent_name_owner = self.agent_name.as_ref().and_then(|_| {
            self.hook_authority
                .as_ref()
                .map(|authority| AgentNameOwner {
                    agent_label: authority.agent_label.clone(),
                    session_ref: authority.session_ref.clone(),
                })
                .or_else(|| {
                    self.persisted_agent_session
                        .as_ref()
                        .map(|session| AgentNameOwner {
                            agent_label: session.agent.clone(),
                            session_ref: Some(session.session_ref.clone()),
                        })
                })
                .or_else(|| {
                    self.effective_agent_label()
                        .map(|agent_label| AgentNameOwner {
                            agent_label: agent_label.to_string(),
                            session_ref: None,
                        })
                })
        });
    }

    /// Track a named launch until matching idle readiness, mismatch or the supplied deadline.
    pub fn begin_managed_agent(&mut self, launch: ManagedAgentLaunch, now: Instant) {
        let ManagedAgentLaunch {
            name,
            kind,
            settle_delay,
            timeout,
        } = launch;
        self.set_agent_name(name);
        self.agent_name_owner = Some(AgentNameOwner {
            agent_label: maestro_detection::agents::agent_label(kind).to_string(),
            session_ref: None,
        });
        self.managed_agent = Some(ManagedAgent {
            kind,
            phase: ManagedAgentPhase::Pending {
                ready_after: Some(now.checked_add(settle_delay).unwrap_or(now)),
                deadline: now.checked_add(timeout).unwrap_or(now),
                observed_expected: false,
            },
        });
    }

    /// Whether a managed launch is still settling or blocked.
    #[must_use]
    pub fn managed_agent_launch_pending(&self) -> bool {
        self.managed_agent.is_some_and(|managed| {
            matches!(
                managed.phase,
                ManagedAgentPhase::Pending { .. } | ManagedAgentPhase::Blocked
            )
        })
    }

    /// Whether a managed launch has reached interactive idle readiness.
    #[must_use]
    pub fn managed_agent_interactive_ready(&self) -> bool {
        self.managed_agent
            .is_some_and(|managed| matches!(managed.phase, ManagedAgentPhase::Active))
    }

    /// Return the catalogue identity owned by the managed launch.
    #[must_use]
    pub fn managed_agent_kind(&self) -> Option<Agent> {
        self.managed_agent.map(|managed| managed.kind)
    }

    /// Return the next settling or launch deadline without creating a timer.
    #[must_use]
    pub fn next_managed_agent_deadline(&self) -> Option<Instant> {
        let ManagedAgentPhase::Pending {
            ready_after,
            deadline,
            ..
        } = self.managed_agent?.phase
        else {
            return None;
        };
        Some(ready_after.unwrap_or(deadline).min(deadline))
    }

    /// Update managed readiness or release the name on mismatch, exit or timeout.
    pub fn reconcile_managed_agent_at(&mut self, now: Instant, process_exited: bool) -> bool {
        let Some(managed) = self.managed_agent else {
            return false;
        };
        let known_agent = self.effective_known_agent();
        let observed_expected = match managed.phase {
            ManagedAgentPhase::Pending {
                observed_expected, ..
            } => observed_expected || known_agent == Some(managed.kind),
            ManagedAgentPhase::Blocked | ManagedAgentPhase::Active => false,
        };
        let clear = process_exited
            || known_agent.is_some_and(|agent| agent != managed.kind)
            || matches!(managed.phase, ManagedAgentPhase::Pending { .. })
                && observed_expected
                && known_agent.is_none();
        if clear {
            self.clear_agent_name();
            return true;
        }
        if managed.phase == ManagedAgentPhase::Blocked {
            if known_agent == Some(managed.kind) && self.state == AgentState::Idle {
                self.managed_agent = Some(ManagedAgent {
                    kind: managed.kind,
                    phase: ManagedAgentPhase::Active,
                });
                return true;
            }
            return false;
        }
        self.reconcile_pending_managed_agent(managed, observed_expected, now)
    }

    /// Advance a pending launch through blocked, timed-out or settled observations.
    fn reconcile_pending_managed_agent(
        &mut self,
        managed: ManagedAgent,
        observed_expected: bool,
        now: Instant,
    ) -> bool {
        let ManagedAgentPhase::Pending {
            ready_after,
            deadline,
            observed_expected: previous_observed_expected,
        } = managed.phase
        else {
            return false;
        };
        let known_agent = self.effective_known_agent();
        if known_agent == Some(managed.kind) && self.state == AgentState::Blocked {
            self.managed_agent = Some(ManagedAgent {
                kind: managed.kind,
                phase: ManagedAgentPhase::Blocked,
            });
            return true;
        }
        if now >= deadline {
            self.clear_agent_name();
            return true;
        }
        let settled = ready_after.is_none_or(|ready_after| now >= ready_after);
        if settled && known_agent == Some(managed.kind) && self.state == AgentState::Idle {
            self.managed_agent = Some(ManagedAgent {
                kind: managed.kind,
                phase: ManagedAgentPhase::Active,
            });
            return true;
        }
        if settled && ready_after.is_some() {
            self.managed_agent = Some(ManagedAgent {
                kind: managed.kind,
                phase: ManagedAgentPhase::Pending {
                    ready_after: None,
                    deadline,
                    observed_expected,
                },
            });
            return true;
        }
        if observed_expected != previous_observed_expected {
            self.managed_agent = Some(ManagedAgent {
                kind: managed.kind,
                phase: ManagedAgentPhase::Pending {
                    ready_after,
                    deadline,
                    observed_expected,
                },
            });
            return true;
        }
        false
    }

    /// Restore a named managed agent in its active phase.
    pub fn restore_managed_agent(&mut self, name: String, kind: Agent) {
        self.set_agent_name(name);
        self.agent_name_owner = Some(AgentNameOwner {
            agent_label: maestro_detection::agents::agent_label(kind).to_string(),
            session_ref: None,
        });
        self.managed_agent = Some(ManagedAgent {
            kind,
            phase: ManagedAgentPhase::Active,
        });
    }

    /// Clear the name, its session owner and managed launch state.
    pub fn clear_agent_name(&mut self) {
        self.agent_name = None;
        self.agent_name_owner = None;
        self.managed_agent = None;
    }

    /// Clear retained runtime identity and pending launch state after respawn.
    pub fn clear_agent_runtime_identity_after_respawn(&mut self) {
        self.detected_agent = None;
        self.fallback_state = AgentState::Unknown;
        self.fallback_visible_blocker = false;
        self.fallback_observed_at = None;
        self.hook_authority = None;
        self.persisted_agent_session = None;
        self.agent_metadata.clear();
        self.metadata_report_agents.clear();
        self.suppressed_full_lifecycle_hook_reports.clear();
        self.stale_full_lifecycle_hook_sessions.clear();
        self.state = AgentState::Unknown;
        self.last_agent_state_change_seq = None;
        self.launch_argv = None;
        self.respawn_shell_on_exit = false;
        self.recent_agent_process_exit = None;
        self.agent_process_acquisition_pending = false;
        self.pending_agent_resume_plan = None;
        self.clear_agent_name();
    }

    /// Whether a name or effective agent identity belongs to this terminal.
    #[must_use]
    pub fn is_agent_terminal(&self) -> bool {
        self.agent_name.is_some() || self.effective_agent_label().is_some()
    }

    /// Preserve names for the same session and clear aliases on identity replacement.
    pub(in crate::terminal) fn reconcile_agent_name_owner(
        &mut self,
        agent_label: &str,
        session_ref: Option<&crate::resume::AgentSessionRef>,
    ) {
        if self.agent_name.is_none() {
            return;
        }
        if self.managed_agent.is_some_and(|managed| {
            maestro_detection::agents::parse_agent_label(agent_label) == Some(managed.kind)
        }) {
            return;
        }
        match self.agent_name_owner.as_mut() {
            Some(owner)
                if owner.agent_label != agent_label
                    || owner
                        .session_ref
                        .as_ref()
                        .zip(session_ref)
                        .is_some_and(|(current, incoming)| current != incoming) =>
            {
                self.agent_name = None;
                self.agent_name_owner = None;
            }
            Some(owner) if owner.session_ref.is_none() && session_ref.is_some() => {
                owner.session_ref = session_ref.cloned();
            }
            None => {
                self.agent_name_owner = Some(AgentNameOwner {
                    agent_label: agent_label.to_string(),
                    session_ref: session_ref.cloned(),
                });
            }
            _ => {}
        }
    }

    /// Select metadata title, manual label, then enabled agent presentation identity.
    #[must_use]
    pub fn border_label(&self, show_agent_labels: bool) -> Option<String> {
        self.effective_title()
            .or_else(|| self.manual_label.clone())
            .or_else(|| {
                if !show_agent_labels {
                    return None;
                }
                self.effective_display_agent()
                    .or_else(|| self.effective_agent_label().map(str::to_string))
            })
    }
}
