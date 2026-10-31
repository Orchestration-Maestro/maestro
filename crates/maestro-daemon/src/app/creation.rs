//! Workspace creation through the topology allocator and shared native launch inputs.

use super::{App, state::Mode};
use crate::{
    pane::{launch_env::PaneLaunchEnv, runtime::PaneRuntime, shell::PaneShellConfig},
    paths::expand_home_path,
    settings::model::NewTerminalCwdConfig,
    terminal::state::TerminalState,
};
use maestro_workspace::workspace::{public_pane_id_for_number, public_tab_id_for_number};
use std::{io, path::PathBuf};

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
        NewTerminalCwdConfig::Path(path) => expand_home_path(path, home),
    }
}

impl<C: FnMut() -> u128> App<C> {
    /// Retains creation intent or creates immediately when no name prompt is configured.
    ///
    /// # Errors
    /// Returns a native pane-spawn error without adding a workspace.
    pub async fn begin_workspace_create(&mut self) -> io::Result<()> {
        let follow = self.workspace_creation_source();
        let follow = match follow {
            Some(workspace) => self.workspace_creation_cwd(workspace).await,
            None => None,
        };
        let cwd = resolve_new_terminal_cwd(&self.config.terminal.new_cwd, follow);
        if self.state.prompt_new_workspace_name.0 {
            self.state.pending_workspace_create_cwd = Some(cwd);
            return Ok(());
        }
        let result = self.create_workspace_with_options(cwd, true).map(|_| ());
        self.state.mode = if self.state.active.is_some() {
            Mode::Terminal
        } else {
            Mode::Navigate
        };
        result
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

    /// Creates the native root inside the topology's consuming spawn transaction.
    ///
    /// # Errors
    /// Returns the native spawn error without publishing topology or runtime ownership.
    pub fn create_workspace_with_options(
        &mut self,
        initial_cwd: PathBuf,
        focus: bool,
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
                    let environment = PaneLaunchEnv::default().with_identity(
                        request.workspace_id.clone(),
                        public_tab_id_for_number(&request.workspace_id, request.tab_number),
                        public_pane_id_for_number(&request.workspace_id, request.pane_number),
                    );
                    let runtime = PaneRuntime::spawn(
                        launch.runtime_config(config, pane, size),
                        PaneShellConfig::new(
                            &config.terminal.default_shell,
                            config.terminal.shell_mode,
                        ),
                        cwd.clone(),
                        &environment,
                    )?;
                    Ok((TerminalState::new(request.terminal_id, cwd), runtime))
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
        let workspace = self.state.topology.workspaces.get(index)?;
        let focused = workspace
            .tabs
            .get(workspace.active_tab)
            .and_then(|tab| tab.terminal_id(tab.layout.focused()));
        if let Some(terminal) = focused {
            if let Some(runtime) = self.terminal_runtimes.get(terminal)
                && let Some(cwd) = runtime.follow_cwd().await
            {
                return Some(cwd);
            }
            if let Some(terminal) = self.state.terminals.get(terminal) {
                return Some(terminal.cwd.clone());
            }
        }
        crate::workspace::resolved_identity_cwd_from(
            workspace,
            &self.state.terminals,
            &self.terminal_runtimes,
        )
    }
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
