//! Retained identity and centralized effective-state arbitration.
pub use super::metadata::{AgentMetadata, AgentMetadataReport, EffectivePresentation};
use maestro_detection::agents::{Agent, AgentState, AgentVisibility};
use maestro_workspace::terminal_id::TerminalId;
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

/// Detection operations.
mod detection;

/// Hooks operations.
mod hooks;

/// Routing operations.
mod routing;

/// Suppression operations.
mod suppression;

/// Session operations.
mod session;

/// Identity operations.
mod identity;

/// Managed operations.
mod managed;

/// Recompute operations.
mod recompute;

/// Retained integration state and session identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookAuthority {
    /// Canonical integration provenance.
    pub source: String,
    /// External catalogue label or custom identity.
    pub agent_label: String,
    /// Effective or reported execution state.
    pub state: AgentState,
    /// Optional integration status detail.
    pub message: Option<String>,
    /// Observation time of the retained report.
    pub reported_at: Instant,
    /// Validated session identity, when supplied.
    pub session_ref: Option<crate::agent_resume::AgentSessionRef>,
}

/// Lifecycle generation suppression with optional replacement evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::terminal) struct SuppressedFullLifecycleHookReport {
    /// External catalogue label or custom identity.
    pub(in crate::terminal) agent_label: String,
    /// Validated session identity, when supplied.
    pub(in crate::terminal) session_ref: Option<crate::agent_resume::AgentSessionRef>,
    /// Process-generation observation time.
    pub(in crate::terminal) observed_at: Instant,
    /// Why lifecycle reports are suppressed.
    pub(in crate::terminal) reason: FullLifecycleHookSuppressionReason,
    /// Replacement session awaiting fresh process evidence.
    pub(in crate::terminal) replacement_session_ref: Option<crate::agent_resume::AgentSessionRef>,
    /// Newest report buffered for the replacement session.
    pub(in crate::terminal) pending_replacement_report: Option<PendingFullLifecycleHookReport>,
}

impl SuppressedFullLifecycleHookReport {
    /// Copy the suppressed session unless it is the explicitly validated replacement.
    fn stale_session(
        &self,
        replacement: Option<&crate::agent_resume::AgentSessionRef>,
    ) -> Option<StaleFullLifecycleHookSession> {
        self.session_ref
            .as_ref()
            .filter(|session| Some(*session) != replacement)
            .map(|session| StaleFullLifecycleHookSession {
                agent_label: self.agent_label.clone(),
                session_ref: session.clone(),
            })
    }

    /// Start suppression without replacement-session or buffered-report evidence.
    fn new(
        agent_label: String,
        session_ref: Option<crate::agent_resume::AgentSessionRef>,
        reason: FullLifecycleHookSuppressionReason,
        observed_at: Instant,
    ) -> Self {
        Self {
            agent_label,
            session_ref,
            reason,
            observed_at,
            replacement_session_ref: None,
            pending_replacement_report: None,
        }
    }
}

/// Sequenced lifecycle report awaiting matching fresh process evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::terminal) struct PendingFullLifecycleHookReport {
    /// Retained lifecycle state and identity.
    pub(in crate::terminal) authority: HookAuthority,
    /// Source-local freshness sequence.
    pub(in crate::terminal) seq: u64,
}

/// Observation that requires lifecycle generation reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::terminal) enum FullLifecycleHookSuppressionReason {
    /// Explicit authority clear or conflicting owner replacement.
    HookClear,
    /// Observed foreground process exit.
    ProcessExit,
}

/// Whether lifecycle evidence may become effective authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::terminal) enum FullLifecycleHookReportRoute {
    /// Admit the report with the selected sequence-reset policy.
    Accept {
        /// Whether this generation starts a new source-local sequence.
        reanchor_sequence: bool,
    },
    /// Do not apply the report to live authority.
    Ignore,
}

/// Source-local session that cannot reacquire authority without explicit reactivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::terminal) struct StaleFullLifecycleHookSession {
    /// External catalogue label or custom identity.
    pub(in crate::terminal) agent_label: String,
    /// Validated session identity, when supplied.
    pub(in crate::terminal) session_ref: crate::agent_resume::AgentSessionRef,
}

/// Readiness phase of a named managed launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::terminal) enum ManagedAgentPhase {
    /// Launch waiting for settling and idle readiness.
    Pending {
        /// Earliest settling observation, if still pending.
        ready_after: Option<Instant>,
        /// Caller-supplied launch deadline.
        deadline: Instant,
        /// Whether the expected agent has been detected during acquisition.
        observed_expected: bool,
    },
    /// Expected agent awaits user interaction.
    Blocked,
    /// Managed agent has reached interactive idle readiness.
    Active,
}

/// Expected agent and its retained launch phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::terminal) struct ManagedAgent {
    /// Managed catalogue identity.
    pub(in crate::terminal) kind: Agent,
    /// Current managed launch phase.
    pub(in crate::terminal) phase: ManagedAgentPhase,
}

/// Effective identity, execution state and presentation before and after a mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveStateChange {
    /// Effective label before the mutation.
    pub previous_agent_label: Option<String>,
    /// Known catalogue identity before the mutation.
    pub previous_known_agent: Option<Agent>,
    /// Execution state before the mutation.
    pub previous_state: AgentState,
    /// Visible presentation before the mutation.
    pub previous_presentation: EffectivePresentation,
    /// External catalogue label or custom identity.
    pub agent_label: Option<String>,
    /// Resolved catalogue identity, if known.
    pub known_agent: Option<Agent>,
    /// Effective or reported execution state.
    pub state: AgentState,
    /// Visible presentation fields.
    pub presentation: EffectivePresentation,
}

/// Independent raw and normalized title changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalTitleChange {
    /// Whether the retained raw title changed.
    pub raw_changed: bool,
    /// Whether the normalized presented title changed.
    pub stripped_changed: bool,
}

/// Observable state, session and release effects of one mutation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalStateMutation {
    /// Observable identity, state or presentation change, if any.
    pub effective_state_change: Option<EffectiveStateChange>,
    /// Whether retained session identity changed.
    pub session_ref_changed: bool,
    /// Whether this mutation released agent ownership.
    pub agent_released: bool,
}

/// Agent and optional session owning the retained name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::terminal) struct AgentNameOwner {
    /// External catalogue label or custom identity.
    pub(in crate::terminal) agent_label: String,
    /// Validated session identity, when supplied.
    pub(in crate::terminal) session_ref: Option<crate::agent_resume::AgentSessionRef>,
}

/// A real foreground process exit and its observation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::terminal) struct RecentAgentProcessExit {
    /// Catalogue identity observed at process exit.
    pub(in crate::terminal) agent: Agent,
    /// Process-generation observation time.
    pub(in crate::terminal) observed_at: Instant,
}

/// Retained identity, presentation and arbitration for one terminal.
pub struct TerminalState {
    /// Terminal identity supplied by the topology owner.
    pub id: TerminalId,
    /// Owned working directory.
    pub cwd: PathBuf,
    /// Latest detected foreground agent.
    pub detected_agent: Option<Agent>,
    /// Latest screen-derived fallback state.
    pub fallback_state: AgentState,
    /// Whether fallback evidence contains a live blocker.
    pub(in crate::terminal) fallback_visible_blocker: bool,
    /// Observation time of fallback screen evidence.
    pub(in crate::terminal) fallback_observed_at: Option<Instant>,
    /// Current integration state and session authority.
    pub hook_authority: Option<HookAuthority>,
    /// Presentation reports indexed by canonical source.
    pub agent_metadata: HashMap<String, AgentMetadata>,
    /// Workspace-owned explicitly expired token values.
    pub metadata_tokens: maestro_workspace::metadata_tokens::MetadataTokens,
    /// Restored or detached identity retained for persistence.
    pub persisted_agent_session: Option<crate::agent_resume::PersistedAgentSession>,
    /// Raw title retained independently from normalized presentation.
    pub terminal_title: Option<String>,
    /// Trimmed nonempty manual border label.
    pub manual_label: Option<String>,
    /// Untrimmed nonempty agent name.
    pub agent_name: Option<String>,
    /// Session identity owning the retained name.
    pub(in crate::terminal) agent_name_owner: Option<AgentNameOwner>,
    /// Named managed launch and readiness phase.
    pub(in crate::terminal) managed_agent: Option<ManagedAgent>,
    /// Latest admitted hook sequence per canonical source.
    pub(in crate::terminal) hook_report_sequences: HashMap<String, u64>,
    /// Generation suppression and pending reports per canonical source.
    pub(in crate::terminal) suppressed_full_lifecycle_hook_reports:
        HashMap<String, SuppressedFullLifecycleHookReport>,
    /// Remembered stale lifecycle sessions per canonical source.
    pub(in crate::terminal) stale_full_lifecycle_hook_sessions:
        HashMap<String, Vec<StaleFullLifecycleHookSession>>,
    /// Latest admitted presentation sequence per canonical source.
    pub(in crate::terminal) metadata_report_sequences: HashMap<String, u64>,
    /// Agent ownership retained for metadata sequence reset on process exit.
    pub(in crate::terminal) metadata_report_agents: HashMap<String, Agent>,
    /// Sequenced token reporters counted against workspace capacity.
    pub(in crate::terminal) metadata_token_sequence_sources: std::collections::HashSet<String>,
    /// Effective or reported execution state.
    pub state: AgentState,
    /// Caller-owned sequence of the last effective agent state change.
    pub last_agent_state_change_seq: Option<u64>,
    /// Wrapping presentation revision.
    pub revision: u64,
    /// Literal command arguments retained for launch.
    pub launch_argv: Option<Vec<String>>,
    /// Whether the caller should respawn a shell after command exit.
    pub respawn_shell_on_exit: bool,
    /// Most recent real process exit, if not superseded by fresh evidence.
    pub(in crate::terminal) recent_agent_process_exit: Option<RecentAgentProcessExit>,
    /// Whether initial foreground-process acquisition still awaits idle.
    pub(in crate::terminal) agent_process_acquisition_pending: bool,
    /// Resume plan awaiting caller execution.
    pub pending_agent_resume_plan: Option<crate::agent_resume::AgentResumePlan>,
}
impl TerminalState {
    /// Create an unacquired terminal with caller-owned identity and working directory.
    #[must_use]
    pub fn new(id: TerminalId, cwd: PathBuf) -> Self {
        Self {
            id,
            cwd,
            detected_agent: None,
            fallback_state: AgentState::Unknown,
            fallback_visible_blocker: false,
            fallback_observed_at: None,
            hook_authority: None,
            agent_metadata: HashMap::new(),
            metadata_tokens: maestro_workspace::metadata_tokens::MetadataTokens::default(),
            persisted_agent_session: None,
            terminal_title: None,
            manual_label: None,
            agent_name: None,
            agent_name_owner: None,
            managed_agent: None,
            hook_report_sequences: HashMap::new(),
            suppressed_full_lifecycle_hook_reports: HashMap::new(),
            stale_full_lifecycle_hook_sessions: HashMap::new(),
            metadata_report_sequences: HashMap::new(),
            metadata_report_agents: HashMap::new(),
            metadata_token_sequence_sources: std::collections::HashSet::new(),
            state: AgentState::Unknown,
            last_agent_state_change_seq: None,
            revision: 0,
            launch_argv: None,
            respawn_shell_on_exit: false,
            recent_agent_process_exit: None,
            agent_process_acquisition_pending: false,
            pending_agent_resume_plan: None,
        }
    }

    /// Begin acquiring a newly observed foreground agent process.
    pub fn set_detected_agent_process_at(
        &mut self,
        agent: Agent,
        now: Instant,
    ) -> TerminalStateMutation {
        let starts_acquisition = !self
            .should_ignore_detected_state_under_full_lifecycle_hook(Some(agent), false)
            && !self.detected_state_observed_before_release_suppression(Some(agent), now);
        let mutation = self.set_detected_state_with_screen_signals_at(
            DetectedStateUpdate {
                agent: Some(agent),
                fallback_state: AgentState::Unknown,
                visible: AgentVisibility {
                    visible_blocker: false,
                    visible_idle: false,
                    visible_working: false,
                },
                process_exited: false,
            },
            now,
        );
        if starts_acquisition {
            self.agent_process_acquisition_pending = true;
        }
        mutation
    }

    /// Finish initial idle acquisition, reporting whether completion should be suppressed.
    pub fn finish_agent_process_acquisition(&mut self) -> bool {
        let reached_idle = self.agent_process_acquisition_pending && self.state == AgentState::Idle;
        let suppress_completion = reached_idle && self.recent_agent_process_exit.is_none();
        if reached_idle {
            self.agent_process_acquisition_pending = false;
        }
        suppress_completion
    }

    /// Normalize the retained raw title for presentation.
    #[must_use]
    pub fn terminal_title_stripped(&self) -> Option<String> {
        self.terminal_title
            .as_deref()
            .and_then(super::title::stripped_terminal_title)
    }

    /// Retain a raw title and advance revision only when its normalized text changes.
    pub fn set_terminal_title(&mut self, title: Option<String>) -> TerminalTitleChange {
        if self.terminal_title == title {
            return TerminalTitleChange::default();
        }
        let previous_stripped = self.terminal_title_stripped();
        self.terminal_title = title;
        let stripped_changed = previous_stripped != self.terminal_title_stripped();
        if stripped_changed {
            self.revision = self.revision.wrapping_add(1);
        }
        TerminalTitleChange {
            raw_changed: true,
            stripped_changed,
        }
    }

    /// Retain literal launch arguments without acquiring agent identity.
    #[must_use]
    pub fn with_launch_argv(mut self, argv: Vec<String>) -> Self {
        self.launch_argv = Some(argv);
        self
    }

    /// Request shell respawn after the launched command exits.
    #[must_use]
    pub fn with_respawn_shell_on_exit(mut self) -> Self {
        self.respawn_shell_on_exit = true;
        self
    }

    /// Whether a real agent process exit falls within the supplied observation window.
    #[cfg(any(windows, test))]
    #[must_use]
    pub fn agent_process_exited_within(&self, now: Instant, max_age: Duration) -> bool {
        self.recent_agent_process_exit
            .is_some_and(|exit| now.saturating_duration_since(exit.observed_at) <= max_age)
    }

    /// Retain a resume plan for the caller to execute.
    #[must_use]
    pub fn with_pending_agent_resume_plan(
        mut self,
        plan: crate::agent_resume::AgentResumePlan,
    ) -> Self {
        self.pending_agent_resume_plan = Some(plan);
        self
    }
}

/// Return the detector policy state without display-derived changes.
#[must_use]
pub fn stabilize_agent_detection(
    detection: maestro_detection::agents::AgentDetection,
) -> AgentState {
    detection.state
}

#[cfg(test)]
mod tests;

/// Supplied detection facts for one observed process generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectedStateUpdate {
    /// Detected foreground agent, if any.
    pub agent: Option<Agent>,
    /// Screen fallback state.
    pub fallback_state: AgentState,
    /// Recognized live screen indicators.
    pub visible: AgentVisibility,
    /// Whether this observation reports a real process exit.
    pub process_exited: bool,
}
/// An integration's retained state and session report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookReport {
    /// Integration provenance admitted into the canonical source domain.
    pub source: String,
    /// External catalogue label or custom identity.
    pub agent_label: String,
    /// Reported execution state.
    pub state: AgentState,
    /// Optional status detail.
    pub message: Option<String>,
    /// Optional validated session identity.
    pub session_ref: Option<crate::agent_resume::AgentSessionRef>,
    /// Source-local sequence, if supplied.
    pub seq: Option<u64>,
}

/// Effective identity and presentation observed before one mutation.
pub(in crate::terminal) struct PreviousEffectiveState {
    /// Previously selected label.
    pub(in crate::terminal) agent_label: Option<String>,
    /// Previously recognized catalogue entry.
    pub(in crate::terminal) known_agent: Option<Agent>,
    /// Previously selected execution state.
    pub(in crate::terminal) state: AgentState,
    /// Previously visible presentation fields.
    pub(in crate::terminal) presentation: EffectivePresentation,
}

/// Supplied name, identity and settling policy for one managed launch.
pub struct ManagedAgentLaunch {
    /// Untrimmed managed name.
    pub name: String,
    /// Expected catalogue identity.
    pub kind: Agent,
    /// Caller-supplied settling delay.
    pub settle_delay: Duration,
    /// Caller-supplied launch timeout.
    pub timeout: Duration,
}

/// Supplied session identity, freshness and lifecycle-start provenance.
#[derive(Debug)]
pub struct SessionStartReport {
    /// Integration provenance admitted into the canonical source domain.
    pub source: String,
    /// External catalogue label or custom identity.
    pub agent_label: String,
    /// Optional validated session identity.
    pub session_ref: Option<crate::agent_resume::AgentSessionRef>,
    /// Source-local freshness sequence, if supplied.
    pub seq: Option<u64>,
    /// Recognized lifecycle-start reason, when supplied.
    pub session_start_source: Option<String>,
}
