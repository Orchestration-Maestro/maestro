//! Asynchronous pane process and screen detection.

use super::{
    agent_detection::{
        AGENT_PENDING_IDLE_RECHECK, DetectionPublishDecision, DetectionPublishState,
        DetectionScreenReadDecision, DetectionScreenReadInput, PendingIdleConfirmation,
        ScreenDetectionPublishInput, decide_detection_screen_read, decide_screen_detection_publish,
        detection_update_for_publish_with_osc,
    },
    probe::{
        ProcessProbeResult,
        presence::{AgentDetectionPresence, ForegroundShellObservation, ForegroundShellState},
        probe_foreground_process,
        schedule::{
            AcquisitionWindow, ProcessProbeInput, foreground_group_changed,
            process_group_for_change_tracking, should_probe_foreground_job,
            should_skip_process_probe_for_lifecycle_authority,
        },
    },
    terminal::PaneTerminal,
};
use crate::events::AppEvent;
use maestro_detection::{
    agents::{Agent, AgentState, AgentVisibility},
    manifest::{DetectionInput, ManifestCache},
};
use maestro_process::pty::foreground::{self, ProcessDetectionMode};
use maestro_workspace::layout::PaneId;
use std::{
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{Notify, mpsc};

/// Grace period before interpreting a newly identified process's screen.
pub const AGENT_STARTUP_GRACE_WINDOW: Duration = Duration::from_secs(3);
/// Suppression of immediate reacquisition after an explicit agent release.
const RELEASE_REACQUIRE_SUPPRESSION: Duration = Duration::from_secs(1);

/// Agent identity temporarily suppressed after release.
#[derive(Debug, Clone, Copy)]
struct PendingAgentRelease {
    /// Released foreground identity.
    agent: Agent,
    /// End of suppression.
    until: Instant,
}

/// Lifecycle authority and detector reset notification for one pane.
#[derive(Debug, Default)]
pub struct DetectionControl {
    /// Whether lifecycle reports currently own state publication.
    full_lifecycle_authority_active: AtomicBool,
    /// Reset permits retained until the detector next waits.
    reset: Notify,
    /// Pending explicit release, shared with the runtime.
    pending_release: Mutex<Option<PendingAgentRelease>>,
}

impl DetectionControl {
    /// Resets fallback tracking only when lifecycle authority becomes active.
    pub fn set_full_lifecycle_authority_active(&self, active: bool) {
        let previous = self
            .full_lifecycle_authority_active
            .swap(active, Ordering::AcqRel);
        if active && !previous {
            self.reset_agent_detection();
        }
    }

    /// Requests a fresh foreground identity and screen observation.
    pub fn reset_agent_detection(&self) {
        self.reset.notify_one();
    }

    /// Waits for the next retained request to refresh detection observations.
    pub(super) async fn wait_for_reset(&self) {
        self.reset.notified().await;
    }

    /// Temporarily suppresses a released identity and resets fallback tracking.
    pub fn begin_graceful_release(&self, agent: Agent) {
        if let Ok(mut pending) = self.pending_release.lock() {
            *pending = Some(PendingAgentRelease {
                agent,
                until: tokio::time::Instant::now().into_std() + RELEASE_REACQUIRE_SUPPRESSION,
            });
        }
        self.reset_agent_detection();
    }

    /// Returns the active suppressed identity, expiring it on observation.
    fn active_pending_release(&self, now: Instant) -> Option<Agent> {
        let mut pending = self.pending_release.lock().ok()?;
        match *pending {
            Some(release) if now < release.until => Some(release.agent),
            _ => {
                *pending = None;
                None
            }
        }
    }
}

/// Supplied owners used by one native pane detector.
pub struct DetectionTaskConfig {
    /// Pane receiving the observations.
    pub pane_id: PaneId,
    /// Direct child session identity retained for native probes.
    pub child_pid: Arc<AtomicU32>,
    /// Retained terminal and OSC evidence.
    pub terminal: Arc<PaneTerminal>,
    /// Revision incremented by raw reads and local terminal mutations.
    pub detection_content_seq: Arc<AtomicU64>,
    /// Runtime lifecycle authority and reset requests.
    pub control: Arc<DetectionControl>,
    /// Workspace event queue.
    pub state_events: mpsc::Sender<AppEvent>,
    /// Caller-owned, replaceable compiled detection snapshot.
    pub manifests: Arc<RwLock<ManifestCache>>,
    /// Native foreground observation policy.
    pub process_detection_mode: ProcessDetectionMode,
}

/// Initial identity and render effects for a newly spawned pane.
pub struct SpawnedDetection {
    /// Restored foreground identity, if supplied by the caller.
    pub detected_agent: Option<Agent>,
    /// Coalesced workspace render origins.
    pub render_dirty: Arc<crate::render_signal::RenderSignal>,
    /// Workspace render wakeup.
    pub render_notify: Arc<Notify>,
}

/// Starts fallback detection for an imported pane without an initial identity.
/// The runtime must abort and join the returned task on teardown.
#[must_use]
pub fn spawn_basic_detection_task(config: DetectionTaskConfig) -> tokio::task::JoinHandle<()> {
    spawn_detection_task(config, None)
}

/// Starts the shared detector with optional spawned-pane scheduling and render effects.
/// The runtime must abort and join the returned task on teardown.
#[must_use]
pub fn spawn_detection_task(
    config: DetectionTaskConfig,
    spawned: Option<SpawnedDetection>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut detector = Detector::new(tokio::time::Instant::now().into_std());
        if let Some(initial) = &spawned {
            detector.presence = AgentDetectionPresence::from_agent(initial.detected_agent);
            detector.published = DetectionPublishState {
                state: AgentState::Idle,
                visible: AgentVisibility {
                    visible_idle: initial.detected_agent.is_some(),
                    ..AgentVisibility::default()
                },
            };
            detector.pending_restore_probe = initial.detected_agent.is_some();
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        loop {
            let delay = detector.delay(&config, spawned.is_some());
            tokio::select! {
                () = tokio::time::sleep(delay) => {},
                () = config.control.wait_for_reset() => {
                    detector = Detector::new(tokio::time::Instant::now().into_std());
                }
            }
            detector
                .tick(
                    &config,
                    tokio::time::Instant::now().into_std(),
                    spawned.as_ref(),
                )
                .await;
        }
    })
}

/// Restores transient child colours before notifying the workspace renderer.
fn restore_theme(config: &DetectionTaskConfig, effects: &SpawnedDetection) {
    let pid = config.child_pid.load(Ordering::Acquire);
    if pid == 0 || !config.terminal.should_probe_host_terminal_theme_restore() {
        return;
    }
    let job = foreground::foreground_job(pid, config.process_detection_mode);
    if config
        .terminal
        .maybe_restore_host_terminal_theme(pid, job.as_ref())
        && effects.render_dirty.request_pty(config.pane_id)
    {
        effects.render_notify.notify_one();
    }
}

/// Retained process, acquisition and publication observations for one detector.
struct Detector {
    /// Identity confirmation owner.
    presence: AgentDetectionPresence,
    /// Last published screen observation.
    published: DetectionPublishState,
    /// Last publication with working or blocking evidence.
    last_visible_signal_refresh: Option<Instant>,
    /// Whether a restored identity still needs native confirmation.
    pending_restore_probe: bool,
    /// Last full native scan.
    last_process_check: Instant,
    /// Previously observed foreground group.
    last_foreground_pgid: Option<u32>,
    /// Whether a full scan has completed.
    has_process_probe: bool,
    /// Same-group acquisition retry window.
    acquisition: AcquisitionWindow,
    /// Completion-before-release publication state.
    shell: ForegroundShellState,
    /// Whether release suppression was active on the preceding tick.
    release_was_active: bool,
    /// Last scanned bottom buffer.
    last_detection_text: String,
    /// Raw-read revision of the last screen scan.
    last_screen_scan_detection_content_seq: Option<u64>,
    /// End of a newly acquired agent's startup grace.
    agent_startup_grace_until: Option<Instant>,
    /// Plain-idle transition confirmation owner.
    pending_idle: PendingIdleConfirmation,
}

impl Detector {
    /// Starts or resets all fallback tracking.
    fn new(now: Instant) -> Self {
        Self {
            presence: AgentDetectionPresence::from_agent(None),
            published: DetectionPublishState {
                state: AgentState::Unknown,
                visible: AgentVisibility::default(),
            },
            last_visible_signal_refresh: None,
            pending_restore_probe: false,
            last_process_check: now,
            last_foreground_pgid: None,
            has_process_probe: false,
            acquisition: AcquisitionWindow::default(),
            shell: ForegroundShellState::default(),
            release_was_active: false,
            last_detection_text: String::new(),
            last_screen_scan_detection_content_seq: None,
            agent_startup_grace_until: None,
            pending_idle: PendingIdleConfirmation::default(),
        }
    }

    /// Selects the reference cadence from live release, colour and identity state.
    fn delay(&self, config: &DetectionTaskConfig, spawned: bool) -> Duration {
        if spawned
            && (config
                .control
                .active_pending_release(tokio::time::Instant::now().into_std())
                .is_some()
                || config.terminal.has_transient_default_color_override())
        {
            Duration::from_millis(50)
        } else if self.pending_idle.active() {
            AGENT_PENDING_IDLE_RECHECK
        } else if spawned && self.presence.current_agent().is_none() {
            Duration::from_millis(500)
        } else {
            Duration::from_millis(300)
        }
    }

    /// Combines native process observations with screen publication policy.
    async fn tick(
        &mut self,
        config: &DetectionTaskConfig,
        now: Instant,
        effects: Option<&SpawnedDetection>,
    ) {
        let suppressed = config.control.active_pending_release(now);
        if suppressed.is_none() && self.release_was_active {
            self.has_process_probe = false;
            self.acquisition = AcquisitionWindow::default();
        }
        self.release_was_active = suppressed.is_some();
        let pid = config.child_pid.load(Ordering::Acquire);
        let foreground_group = (pid > 0)
            .then(|| foreground::foreground_process_group_id(pid))
            .flatten();
        let input = self.probe_input(suppressed, foreground_group, now);
        let lifecycle_authority = config
            .control
            .full_lifecycle_authority_active
            .load(Ordering::Acquire);
        let mut agent_changed = false;
        if pid > 0
            && !should_skip_process_probe_for_lifecycle_authority(lifecycle_authority, input)
            && should_probe_foreground_job(input)
        {
            let probe =
                probe_foreground_process(pid, foreground_group, config.process_detection_mode);
            agent_changed = self.apply_probe(config, probe, input, now).await;
        }
        if let Some(effects) = effects {
            restore_theme(config, effects);
        }
        if self.screen_allowed(lifecycle_authority, now) {
            self.scan_screen(config, input, agent_changed, now).await;
        }
    }

    /// Builds the shared scheduling input from live retained state.
    fn probe_input(
        &self,
        suppressed: Option<Agent>,
        pgid: Option<u32>,
        now: Instant,
    ) -> ProcessProbeInput {
        ProcessProbeInput {
            current_agent: self.presence.current_agent(),
            suppressed_agent: suppressed,
            foreground_pgid: pgid,
            last_foreground_pgid: self.last_foreground_pgid,
            has_process_probe: self.has_process_probe,
            acquisition_age: self
                .acquisition
                .started_at
                .map(|started| now.duration_since(started)),
            pending_foreground_shell_clear: self.shell.pending_clear,
            pending_restore_probe: self.pending_restore_probe,
            elapsed_since_process_check: now.duration_since(self.last_process_check),
        }
    }

    /// Applies native scan facts and publishes a newly acquired process before its screen.
    async fn apply_probe(
        &mut self,
        config: &DetectionTaskConfig,
        mut probe: ProcessProbeResult,
        input: ProcessProbeInput,
        now: Instant,
    ) -> bool {
        self.last_process_check = now;
        self.has_process_probe = true;
        self.pending_restore_probe = false;
        if let Some(suppressed) = input.suppressed_agent {
            if probe.agent == Some(suppressed) {
                probe.agent = None;
            } else if let Ok(mut pending) = config.control.pending_release.lock() {
                *pending = None;
            }
        }
        let previous_agent = self.presence.current_agent();
        let changed = self.shell.apply(
            &mut self.presence,
            ForegroundShellObservation {
                previous_agent,
                new_agent: probe.agent,
                foreground_is_pane_shell: probe.foreground_is_pane_shell,
                process_exit_reported: self.shell.exit_reported,
            },
        );
        self.last_foreground_pgid =
            process_group_for_change_tracking(input.foreground_pgid, probe.process_group_id);
        if probe.agent.is_some() {
            self.acquisition = AcquisitionWindow::default();
        } else if self.presence.current_agent().is_none()
            && input.has_process_probe
            && foreground_group_changed(input.foreground_pgid, input.last_foreground_pgid)
        {
            self.acquisition.started_at = Some(now);
        }
        if !changed {
            return false;
        }
        self.pending_idle.clear();
        self.last_screen_scan_detection_content_seq = None;
        config.terminal.clear_agent_osc_state();
        self.agent_startup_grace_until = self
            .presence
            .current_agent()
            .map(|_| now + AGENT_STARTUP_GRACE_WINDOW);
        if let Some(agent) = self.presence.current_agent() {
            self.published = DetectionPublishState {
                state: AgentState::Unknown,
                visible: AgentVisibility::default(),
            };
            self.last_visible_signal_refresh = None;
            AppEvent::AgentProcessDetected {
                pane_id: config.pane_id,
                agent,
                observed_at: now,
            }
            .publish(&config.state_events)
            .await;
        }
        true
    }

    /// Whether completion must precede releasing the retained identity.
    fn process_exited(&self) -> bool {
        self.shell.pending_clear
            && self.presence.current_agent().is_some()
            && !self.shell.exit_reported
    }

    /// Gates fallback state during lifecycle authority and process startup.
    fn screen_allowed(&mut self, lifecycle_authority: bool, now: Instant) -> bool {
        let process_exited = self.process_exited();
        if lifecycle_authority && !process_exited {
            self.pending_idle.clear();
            return false;
        }
        let Some(until) = self.agent_startup_grace_until else {
            return true;
        };
        self.pending_idle.clear();
        if !process_exited && now < until {
            return false;
        }
        self.agent_startup_grace_until = None;
        self.last_screen_scan_detection_content_seq = None;
        if !process_exited {
            return false;
        }
        true
    }

    /// Reads only due bottom buffers and applies the existing publication policies.
    async fn scan_screen(
        &mut self,
        config: &DetectionTaskConfig,
        input: ProcessProbeInput,
        agent_changed: bool,
        now: Instant,
    ) {
        let agent = self.presence.current_agent();
        let process_exited = self.process_exited();
        let revision = agent.map(|_| config.detection_content_seq.load(Ordering::Relaxed));
        if decide_detection_screen_read(DetectionScreenReadInput {
            state: self.published.state,
            agent,
            pending_idle_active: self.pending_idle.active(),
            agent_changed,
            process_exited,
            current_detection_content_seq: revision,
            last_screen_scan_detection_content_seq: self.last_screen_scan_detection_content_seq,
        }) == DetectionScreenReadDecision::Skip
        {
            return;
        }
        self.last_screen_scan_detection_content_seq = revision;
        let detection = self.detect_screen(config, input, now);
        let Some(screen_detection) = detection else {
            self.pending_idle.clear();
            return;
        };
        self.publish_detection(
            config,
            ScreenDetectionPublishInput {
                previous: self.published,
                last_visible_signal_refresh: self.last_visible_signal_refresh,
                screen_detection,
                process_exited,
                agent_changed,
                now,
            },
        )
        .await;
    }

    /// Evaluates fresh terminal evidence while retaining acquisition observations.
    fn detect_screen(
        &mut self,
        config: &DetectionTaskConfig,
        input: ProcessProbeInput,
        now: Instant,
    ) -> Option<maestro_detection::agents::AgentDetection> {
        let agent = self.presence.current_agent();
        let process_exited = self.process_exited();
        let content = config.terminal.detection_text();
        let content_changed = content != self.last_detection_text;
        self.last_detection_text.clone_from(&content);
        let title = config.terminal.agent_osc_title();
        let progress = config.terminal.agent_osc_progress();
        {
            let cache = config
                .manifests
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !process_exited
                && maestro_detection::should_skip_state_update(&cache, agent, &content)
            {
                self.pending_idle.clear();
                return None;
            }
            self.acquisition.sync(
                ProcessProbeInput {
                    current_agent: agent,
                    ..input
                },
                content_changed,
                now,
            );
            detection_update_for_publish_with_osc(
                &cache,
                agent,
                DetectionInput {
                    screen: &content,
                    osc_title: &title,
                    osc_progress: &progress,
                },
                process_exited,
            )
        }
    }

    /// Retains each selected publication before waiting for workspace queue space.
    async fn publish_detection(
        &mut self,
        config: &DetectionTaskConfig,
        input: ScreenDetectionPublishInput,
    ) {
        if let DetectionPublishDecision::Publish {
            next,
            process_exited,
        } = decide_screen_detection_publish(input, &mut self.pending_idle)
        {
            let now = input.now;
            self.published = next;
            self.last_visible_signal_refresh =
                (next.visible.visible_blocker || next.visible.visible_working).then_some(now);
            self.shell.exit_reported |= process_exited;
            AppEvent::StateChanged {
                pane_id: config.pane_id,
                agent: self.presence.current_agent(),
                state: next.state,
                visible_blocker: next.visible.visible_blocker,
                visible_working: next.visible.visible_working,
                process_exited,
                observed_at: now,
            }
            .publish(&config.state_events)
            .await;
        }
    }
}

#[cfg(test)]
mod tests;
