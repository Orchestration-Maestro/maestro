//! Workspace creation through the topology allocator and shared native launch inputs.

use super::{App, state::Mode};
use crate::{
    pane::{launch_env::PaneLaunchEnv, runtime::PaneRuntime, shell::PaneShellConfig},
    settings::model::NewTerminalCwdConfig,
    terminal::state::TerminalState,
};
use maestro_protocol::schema::{ErrorBody, WorkspaceCreateParams};
use maestro_workspace::workspace::{public_pane_id_for_number, public_tab_id_for_number};
use std::{collections::HashMap, io, path::PathBuf};

/// Expands only an exact `~` or a leading `~/` (`~\` on Windows), joining the rest as a
/// native path and keeping every other spelling without reading the home directory.
fn expand_home_path(path: &str) -> PathBuf {
    let Some(rest) = path.strip_prefix('~') else {
        return path.into();
    };
    if !rest.is_empty() && !rest.starts_with(std::path::is_separator) {
        return path.into();
    }
    let Ok(home) = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory is unknown"))
    else {
        return path.into();
    };
    if rest.is_empty() {
        return home;
    }
    rest.strip_prefix(std::path::is_separator)
        .map_or_else(|| path.into(), |rest| join_tilde_rest(home, rest))
}

/// Joins `rest` below `home` natively: on Windows every separator splits the rest and empty
/// components are dropped; elsewhere an absolute rest replaces the home.
fn join_tilde_rest(home: PathBuf, rest: &str) -> PathBuf {
    if cfg!(windows) {
        rest.split(['/', '\\'])
            .filter(|component| !component.is_empty())
            .fold(home, |path, component| path.join(component))
    } else {
        home.join(rest)
    }
}

/// Resolves the configured cwd policy against one observed focused directory.
fn resolve_new_terminal_cwd(policy: &NewTerminalCwdConfig, follow_cwd: Option<PathBuf>) -> PathBuf {
    let home = || std::env::var_os("HOME").map(PathBuf::from);
    let current = || std::env::current_dir().ok();
    match policy {
        NewTerminalCwdConfig::Follow => follow_cwd
            .or_else(home)
            .or_else(current)
            .unwrap_or_else(|| "/".into()),
        NewTerminalCwdConfig::Home => home().or_else(current).unwrap_or_else(|| "/".into()),
        NewTerminalCwdConfig::Current => current().unwrap_or_else(|| "/".into()),
        NewTerminalCwdConfig::Path(path) => expand_home_path(path),
    }
}

impl<C: FnMut() -> u128> App<C> {
    /// Retains creation intent or creates immediately, through the workspace command
    /// handler, when no name prompt is configured.
    ///
    /// # Errors
    /// Returns the handler's `workspace_create_failed` failure without adding a workspace.
    pub async fn begin_workspace_create(&mut self) -> Result<(), ErrorBody> {
        if self.state.prompt_new_workspace_name.0 {
            self.state.pending_workspace_create_cwd = Some(self.new_workspace_cwd().await);
            return Ok(());
        }
        let result = self
            .handle_workspace_create(WorkspaceCreateParams {
                cwd: None,
                focus: true,
                label: None,
                env: HashMap::new(),
            })
            .await
            .map(|_| ());
        self.state.mode = if self.state.active.is_some() {
            Mode::Terminal
        } else {
            Mode::Navigate
        };
        result
    }

    /// The directory a new workspace starts in: the creation source's focused directory
    /// under the configured policy.
    pub(super) async fn new_workspace_cwd(&self) -> PathBuf {
        let follow = match self.workspace_creation_source() {
            Some(workspace) => self.workspace_creation_cwd(workspace).await,
            None => None,
        };
        resolve_new_terminal_cwd(&self.config.terminal.new_cwd, follow)
    }

    /// The directory a new tab of `workspace` starts in: its focused pane's directory under
    /// the configured policy.
    pub(super) async fn new_tab_cwd(&self, workspace: usize) -> PathBuf {
        let follow = self.focused_pane_cwd_in_workspace(workspace).await;
        resolve_new_terminal_cwd(&self.config.terminal.new_cwd, follow)
    }

    /// Cancels the daemon's pending creation intent without allocating a workspace.
    pub fn cancel_workspace_create(&mut self) {
        self.state.pending_workspace_create_cwd = None;
    }

    /// Creates an initial workspace only when neither topology nor naming owns one.
    pub fn ensure_default_workspace(&mut self) -> bool {
        if !self.state.topology.workspaces.is_empty()
            || self.state.pending_workspace_create_cwd.is_some()
        {
            return false;
        }
        let cwd = resolve_new_terminal_cwd(&self.config.terminal.new_cwd, None);
        match self.create_workspace_with_options(cwd, true) {
            Ok(_) => true,
            Err(error) => {
                tracing::error!(%error, "failed to create default workspace");
                self.state.mode = Mode::Navigate;
                false
            }
        }
    }

    /// Opens a focused workspace in the starting server's `cwd` when no workspace exists yet.
    pub fn seed_startup_workspace(&mut self, cwd: Option<PathBuf>) {
        let Some(cwd) = cwd.filter(|cwd| !cwd.as_os_str().is_empty()) else {
            return;
        };
        if !self.state.topology.workspaces.is_empty() {
            tracing::info!(
                cwd = %cwd.display(),
                "restored session already has workspaces; ignoring startup cwd"
            );
            return;
        }
        match self.create_workspace_with_options(cwd.clone(), true) {
            Ok(_) => tracing::info!(cwd = %cwd.display(), "created startup workspace"),
            Err(error) => {
                tracing::warn!(cwd = %cwd.display(), err = %error, "failed to create startup workspace");
                self.state.mode = Mode::Navigate;
            }
        }
    }

    /// Creates the native root inside the topology's consuming spawn transaction.
    ///
    /// # Errors
    /// Returns the native spawn error without publishing topology or runtime ownership.
    pub fn create_workspace_with_options(
        &mut self,
        initial_cwd: PathBuf,
        focus: bool,
    ) -> io::Result<usize> {
        self.create_workspace_with_launch_env(initial_cwd, focus, Vec::new())
    }

    /// Creates a workspace whose root pane also receives the `extra_env` pairs.
    ///
    /// # Errors
    /// Returns the native spawn error without publishing topology or runtime ownership.
    pub(super) fn create_workspace_with_launch_env(
        &mut self,
        initial_cwd: PathBuf,
        focus: bool,
        extra_env: Vec<(String, String)>,
    ) -> io::Result<usize> {
        let size = self.state.estimate_pane_size();
        let launch = &self.launch;
        let config = &self.config;
        let cwd = initial_cwd.clone();
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let (pane, (terminal, runtime)) =
            self.state
                .topology
                .new_with_tab(initial_cwd, home.as_deref(), |pane, request| {
                    spawn_root(
                        launch.runtime_config(config, pane, size),
                        config,
                        cwd.clone(),
                        extra_env,
                        request,
                    )
                })?;
        self.terminal_runtimes.insert(terminal.id.clone(), runtime);
        self.state.terminals.insert(terminal.id.clone(), terminal);
        self.state.pane_id_aliases.remove(&pane.raw());
        let index = self.state.topology.workspaces.len() - 1;
        crate::workspace::admit_git_identity(
            &mut self.state.topology.workspaces[index],
            home.as_deref(),
        );
        crate::logging::workspace_created(&self.state.topology.workspaces[index].id, pane.raw());
        if focus || self.state.active.is_none() {
            self.state.switch_workspace(index);
            self.state.mode = Mode::Terminal;
        }
        self.state.session_dirty = true;
        Ok(index)
    }

    /// Adds a tab to `workspace` whose root pane starts in `cwd` with the `extra_env` pairs,
    /// returning the new tab's display position.
    ///
    /// # Errors
    /// Returns the native spawn error, or "workspace disappeared" for a missing workspace.
    pub(super) fn create_tab_with_launch_env(
        &mut self,
        workspace: usize,
        cwd: PathBuf,
        extra_env: Vec<(String, String)>,
    ) -> io::Result<usize> {
        let size = self.state.estimate_pane_size();
        let launch = &self.launch;
        let config = &self.config;
        let (pane, (terminal, runtime)) = self
            .state
            .topology
            .create_tab_with_runtime(workspace, |pane, request| {
                spawn_root(
                    launch.runtime_config(config, pane, size),
                    config,
                    cwd,
                    extra_env,
                    request,
                )
            })
            .ok_or_else(|| io::Error::other("workspace disappeared"))??;
        self.terminal_runtimes.insert(terminal.id.clone(), runtime);
        self.state.terminals.insert(terminal.id.clone(), terminal);
        self.state.pane_id_aliases.remove(&pane.raw());
        Ok(self.state.topology.workspaces[workspace].tabs.len() - 1)
    }

    /// Chooses navigation selection before active input focus, matching creation intent.
    fn workspace_creation_source(&self) -> Option<usize> {
        let selected = self
            .state
            .topology
            .workspaces
            .get(self.state.selected)
            .map(|_| self.state.selected);
        if self.state.mode == Mode::Navigate && selected.is_some() {
            return selected;
        }
        self.state.active.or(selected)
    }

    /// Follows the focused process, falling back to the workspace's root identity.
    async fn workspace_creation_cwd(&self, index: usize) -> Option<PathBuf> {
        if let Some(cwd) = self.focused_pane_cwd_in_workspace(index).await {
            return Some(cwd);
        }
        crate::workspace::resolved_identity_cwd_from(
            self.state.topology.workspaces.get(index)?,
            &self.state.terminals,
            &self.terminal_runtimes,
        )
    }

    /// The live directory of the focused pane of `index`, else its terminal's last known one.
    async fn focused_pane_cwd_in_workspace(&self, index: usize) -> Option<PathBuf> {
        let workspace = self.state.topology.workspaces.get(index)?;
        let terminal = workspace
            .tabs
            .get(workspace.active_tab)
            .and_then(|tab| tab.terminal_id(tab.layout.focused()))?;
        if let Some(runtime) = self.terminal_runtimes.get(terminal)
            && let Some(cwd) = runtime.follow_cwd().await
        {
            return Some(cwd);
        }
        self.state
            .terminals
            .get(terminal)
            .map(|terminal| terminal.cwd.clone())
    }
}

/// Spawns the root process of a new tab with this pane's managed identity and the caller's
/// extra environment, returning its terminal record and runtime.
fn spawn_root(
    runtime_config: crate::pane::runtime::PaneRuntimeConfig,
    config: &crate::settings::model::Config,
    cwd: PathBuf,
    extra_env: Vec<(String, String)>,
    request: maestro_workspace::tab::SpawnRequest,
) -> io::Result<(TerminalState, PaneRuntime)> {
    let environment = PaneLaunchEnv::from_extra(extra_env).with_identity(
        request.workspace_id.clone(),
        public_tab_id_for_number(&request.workspace_id, request.tab_number),
        public_pane_id_for_number(&request.workspace_id, request.pane_number),
    );
    let runtime = PaneRuntime::spawn(
        runtime_config,
        PaneShellConfig::new(&config.terminal.default_shell, config.terminal.shell_mode),
        cwd.clone(),
        &environment,
    )?;
    Ok((TerminalState::new(request.terminal_id, cwd), runtime))
}

impl<C: FnMut() -> u128> App<C> {
    /// Projects a tab's stable identity independently of its display position.
    #[must_use]
    pub fn tab_info(
        &self,
        workspace: usize,
        tab: usize,
    ) -> Option<maestro_protocol::schema::TabInfo> {
        let target = self.state.topology.workspaces.get(workspace)?;
        let record = target.tabs.get(tab)?;
        let (state, seen) = record
            .panes
            .values()
            .filter_map(|pane| {
                self.state
                    .terminals
                    .get(&pane.attached_terminal_id)
                    .map(|terminal| (terminal.state, pane.seen))
            })
            .max_by_key(|(state, seen)| {
                crate::workspace::aggregate::pane_attention_priority(*state, *seen)
            })
            .unwrap_or((maestro_detection::agents::AgentState::Unknown, true));
        Some(maestro_protocol::schema::TabInfo {
            tab_id: self.state.public_tab_id(workspace, tab)?,
            workspace_id: target.id.clone(),
            number: record.number,
            label: target.tab_display_name(tab)?,
            focused: self.state.active == Some(workspace) && target.active_tab == tab,
            pane_count: record.panes.len(),
            agent_status: super::actions::pane_agent_status(state, seen),
        })
    }
    /// Projects one retained workspace for metadata subscribers.
    pub(super) fn workspace_info(
        &self,
        index: usize,
    ) -> Option<maestro_protocol::schema::WorkspaceInfo> {
        use maestro_protocol::schema::{WorkspaceInfo, WorkspaceWorktreeInfo};
        let ws = self.state.topology.workspaces.get(index)?;
        let (state, seen) = crate::workspace::aggregate::aggregate_state(ws, &self.state.terminals);
        Some(WorkspaceInfo {
            workspace_id: ws.id.clone(),
            number: index + 1,
            label: crate::workspace::display_name_from(
                ws,
                &self.state.terminals,
                &self.terminal_runtimes,
                None,
            ),
            focused: self.state.active == Some(index),
            pane_count: ws.public_pane_numbers.len(),
            tab_count: ws.tabs.len(),
            active_tab_id: self
                .state
                .public_tab_id(index, ws.active_tab)
                .unwrap_or_else(|| {
                    maestro_workspace::workspace::public_tab_id_for_number(
                        &ws.id,
                        ws.active_tab + 1,
                    )
                }),
            agent_status: super::actions::pane_agent_status(state, seen),
            tokens: ws.metadata_tokens.values(),
            worktree: ws.worktree_space().map(|space| WorkspaceWorktreeInfo {
                repo_key: space.key.clone(),
                repo_name: space.label.clone(),
                repo_root: space.repo_root.display().to_string(),
                checkout_path: space.checkout_path.display().to_string(),
                is_linked_worktree: space.is_linked_worktree,
            }),
        })
    }
}

impl<C: FnMut() -> u128> App<C> {
    /// Publishes a new workspace with its first tab and that tab's root pane.
    pub(super) async fn emit_workspace_open_events(&self, index: usize) {
        let Some(workspace) = self.workspace_info(index) else {
            return;
        };
        let Some(tab) = self.tab_info(index, 0) else {
            return;
        };
        let Some(root_pane) = self.root_pane_info(index, 0).await else {
            return;
        };
        self.event_hub
            .push(maestro_protocol::events::EventData::WorkspaceCreated { workspace });
        self.emit_tab_and_pane_created_events(tab, root_pane);
        self.emit_layout_updated_event(index, 0);
    }

    /// Publishes a new tab and its root pane.
    pub(super) async fn emit_tab_created_events(&self, workspace: usize, tab: usize) {
        let Some(tab_record) = self.tab_info(workspace, tab) else {
            return;
        };
        if let Some(root_pane) = self.root_pane_info(workspace, tab).await {
            self.emit_tab_and_pane_created_events(tab_record, root_pane);
            self.emit_layout_updated_event(workspace, tab);
        }
    }

    /// Publishes a created tab, then its root pane.
    fn emit_tab_and_pane_created_events(
        &self,
        tab: maestro_protocol::schema::TabInfo,
        pane: maestro_protocol::schema::PaneInfo,
    ) {
        use maestro_protocol::events::EventData;
        self.event_hub.push(EventData::TabCreated { tab });
        self.event_hub.push(EventData::PaneCreated { pane });
    }

    /// The result of creating the workspace at `index`: it, its first tab and root pane.
    pub(super) async fn workspace_created_result(
        &self,
        index: usize,
    ) -> Option<maestro_protocol::schema::ResponseResult> {
        Some(maestro_protocol::schema::ResponseResult::WorkspaceCreated {
            workspace: self.workspace_info(index)?,
            tab: self.tab_info(index, 0)?,
            root_pane: self.root_pane_info(index, 0).await?,
        })
    }

    /// The result of creating a tab: it and its root pane.
    pub(super) async fn tab_created_result(
        &self,
        workspace: usize,
        tab: usize,
    ) -> Option<maestro_protocol::schema::ResponseResult> {
        Some(maestro_protocol::schema::ResponseResult::TabCreated {
            tab: self.tab_info(workspace, tab)?,
            root_pane: self.root_pane_info(workspace, tab).await?,
        })
    }

    /// The record of a tab's root pane.
    async fn root_pane_info(
        &self,
        workspace: usize,
        tab: usize,
    ) -> Option<maestro_protocol::schema::PaneInfo> {
        let root = self
            .state
            .topology
            .workspaces
            .get(workspace)?
            .tabs
            .get(tab)?
            .root_pane;
        self.pane_info(workspace, root).await
    }
}
