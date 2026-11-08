//! Launch preparation and the single concrete runtime constructor.
use super::super::{
    commands::{PaneShellConfig, ShellLaunchTarget, pane_shell_command_builder_for_target},
    monitoring::{DetectionControl, DetectionTaskConfig, SpawnedDetection, spawn_detection_task},
};
use super::{PaneRuntime, PaneRuntimeConfig, reader, shutdown as lifecycle};
use crate::panes::{display::PaneTerminal, launch::PaneLaunchEnv};
use maestro_detection::agents::Agent;
use maestro_process::terminal::PtyCommand;
use std::os::fd::OwnedFd;
use std::{
    cell::Cell,
    io,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

/// Whether a command pane runs fallback detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentDetection {
    /// Combine native foreground facts and terminal evidence.
    Enabled,
    /// Process terminal bytes without background agent detection.
    Disabled,
}

/// Supplied terminal history and restored identity for a spawned pane.
#[derive(Default)]
pub struct SpawnInitialState {
    /// Initial foreground identity awaiting native confirmation.
    pub detected_agent: Option<Agent>,
    /// Retained ANSI history seeded before live output.
    pub history_ansi: Option<String>,
}

/// Whether the descriptor was spawned here or received from another server.
#[derive(Clone, Copy)]
pub(super) enum RuntimeOrigin {
    /// A new child with optional restored foreground identity.
    Spawned(Option<Agent>),
    /// A received descriptor whose session ownership is initially external.
    Imported,
}

impl PaneRuntime {
    /// Starts the configured interactive shell.
    ///
    /// # Errors
    /// Returns shell resolution, terminal creation or native setup failures.
    pub fn spawn(
        config: PaneRuntimeConfig,
        shell: PaneShellConfig<'_>,
        cwd: PathBuf,
        env: &PaneLaunchEnv,
    ) -> io::Result<Self> {
        Self::spawn_with_initial_history(config, shell, cwd, env, None)
    }

    /// Starts a shell with retained history seeded before live terminal reads.
    ///
    /// # Errors
    /// Returns shell resolution, terminal creation or native setup failures.
    pub fn spawn_with_initial_history(
        config: PaneRuntimeConfig,
        shell: PaneShellConfig<'_>,
        cwd: PathBuf,
        env: &PaneLaunchEnv,
        history: Option<String>,
    ) -> io::Result<Self> {
        let command =
            pane_shell_command_builder_for_target(shell, ShellLaunchTarget::OtherUnix, |name| {
                std::env::var_os(name)
            })?;
        let command = prepare_command(&config, command, cwd, env);
        Self::spawn_command_builder(
            config,
            command,
            SpawnInitialState {
                history_ansi: history,
                ..SpawnInitialState::default()
            },
        )
    }

    /// Starts a real shell command under the pane's environment policy.
    ///
    /// # Errors
    /// Returns terminal creation or native setup failures.
    pub fn spawn_shell_command(
        config: PaneRuntimeConfig,
        command: &str,
        cwd: PathBuf,
        env: &PaneLaunchEnv,
    ) -> io::Result<Self> {
        let command = maestro_process::commands::pane_custom_command_pty_builder(command);
        let command = prepare_command(&config, command, cwd, env);
        Self::spawn_command_builder(config, command, SpawnInitialState::default())
    }

    /// Starts a native argv without shell interpretation.
    ///
    /// # Errors
    /// Returns an empty-argv error, terminal creation or native setup failures.
    pub fn spawn_argv_command(
        config: PaneRuntimeConfig,
        argv: &[String],
        cwd: PathBuf,
        env: &PaneLaunchEnv,
    ) -> io::Result<Self> {
        let Some((program, arguments)) = argv.split_first() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "argv must not be empty",
            ));
        };
        let command = PtyCommand {
            program: program.into(),
            argv0: None,
            args: arguments.iter().map(Into::into).collect(),
            cwd: None,
            environment: maestro_process::terminal::PtyEnvironment::Inherit(Vec::new()),
        };
        let command = prepare_command(&config, command, cwd, env);
        Self::spawn_command_builder(config, command, SpawnInitialState::default())
    }

    /// Starts prepared native launch operands with supplied initial terminal state.
    ///
    /// # Errors
    /// Returns terminal creation, entry launch or target setup failures.
    pub fn spawn_command_builder(
        config: PaneRuntimeConfig,
        command: PtyCommand,
        initial: SpawnInitialState,
    ) -> io::Result<Self> {
        crate::diagnostics::pane_spawn_started(
            config.pane_id.raw(),
            config.size.rows,
            config.size.cols,
            config.scrollback_limit_bytes,
        );
        let terminal = make_terminal(&config)?;
        if let Some(history) = initial.history_ansi {
            terminal.seed_history_ansi(&history);
        }
        let (pty, mut child) = config.setup_entry.spawn(config.size, command)?;
        let pid = child.id();
        crate::diagnostics::pane_spawned(config.pane_id.raw(), pid);
        let mut runtime = match Self::assemble(
            config,
            terminal,
            pty.master_fd,
            pid,
            RuntimeOrigin::Spawned(initial.detected_agent),
        ) {
            Ok(runtime) => runtime,
            Err(error) => {
                lifecycle::shutdown_processes(pid, None);
                let _ = child.wait();
                return Err(error);
            }
        };
        let completed = Arc::new(AtomicBool::new(false));
        runtime.watcher = Some(lifecycle::watch_child(
            child,
            Arc::clone(&completed),
            &runtime.shared,
        ));
        runtime.child_wait_completed = Some(completed);
        Ok(runtime)
    }

    /// Installs reader and detector ownership once for spawned and imported masters.
    pub(super) fn assemble(
        config: PaneRuntimeConfig,
        terminal: Arc<PaneTerminal>,
        fd: OwnedFd,
        pid: u32,
        origin: RuntimeOrigin,
    ) -> io::Result<Self> {
        let shared = Arc::new(reader::ReadState::new(&config, terminal, pid));
        let io = reader::spawn_actor(
            fd,
            Arc::clone(&shared),
            matches!(origin, RuntimeOrigin::Imported),
        )?;
        let control = Arc::new(DetectionControl::default());
        let detect_handle = (matches!(origin, RuntimeOrigin::Imported)
            || config.agent_detection == AgentDetection::Enabled)
            .then(|| {
                spawn_detection_task(
                    DetectionTaskConfig {
                        pane_id: config.pane_id,
                        child_pid: Arc::clone(&shared.child_pid),
                        terminal: Arc::clone(&shared.terminal),
                        detection_content_seq: Arc::clone(&shared.detection_content_seq),
                        control: Arc::clone(&control),
                        state_events: config.events,
                        manifests: config.manifests,
                        process_detection_mode: config.process_detection_mode,
                    },
                    match origin {
                        RuntimeOrigin::Imported => None,
                        RuntimeOrigin::Spawned(detected_agent) => Some(SpawnedDetection {
                            detected_agent,
                            render_dirty: config.render_dirty,
                            render_notify: config.render_notify,
                        }),
                    },
                )
            });
        Ok(Self {
            shared,
            io,
            size: Cell::new(config.size),
            child_wait_completed: None,
            watcher: None,
            control,
            detect_handle,
            preserve_processes_on_drop: matches!(origin, RuntimeOrigin::Imported),
        })
    }

    /// Cancels and joins the runtime's fallback detector.
    pub(super) async fn stop_detection(&mut self) {
        if let Some(task) = self.detect_handle.take() {
            task.abort();
            let _ = task.await;
        }
    }

    /// Resets fallback observations when full lifecycle authority activates.
    pub fn set_full_lifecycle_authority_active(&self, active: bool) {
        self.control.set_full_lifecycle_authority_active(active);
    }

    /// Requests a fresh native and screen observation.
    pub fn reset_agent_detection(&self) {
        self.control.reset_agent_detection();
    }

    /// Suppresses immediate reacquisition of an explicitly released foreground identity.
    pub fn begin_graceful_release(&self, agent: Agent) {
        self.control.begin_graceful_release(agent);
    }
}

/// Applies directory and identity edits at launch, before the child starts.
fn prepare_command(
    config: &PaneRuntimeConfig,
    mut command: PtyCommand,
    cwd: PathBuf,
    env: &PaneLaunchEnv,
) -> PtyCommand {
    command.cwd = Some(cwd);
    env.apply(&mut command, &config.socket, config.executable.as_deref());
    command
}

/// Creates and themes the terminal before history or handoff replay.
pub(super) fn make_terminal(config: &PaneRuntimeConfig) -> io::Result<Arc<PaneTerminal>> {
    let terminal = maestro_terminal::Terminal::new(
        config.size.cols,
        config.size.rows,
        config.scrollback_limit_bytes,
    )
    .map_err(io::Error::other)?;
    let terminal = Arc::new(PaneTerminal::new(terminal));
    terminal.apply_host_terminal_theme(config.theme);
    let _ = terminal.apply_host_terminal_appearance(config.appearance);
    Ok(terminal)
}
