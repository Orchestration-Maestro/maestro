//! Replaces a completed launch command with the configured interactive shell.

use crate::{
    app::App,
    events::AppEvent,
    pane::{runtime::PaneRuntime, shell::PaneShellConfig},
};
use maestro_workspace::layout::PaneId;

impl<C: FnMut() -> u128> App<C> {
    /// Attempts replacement before pane removal; failure leaves ordinary close policy intact.
    pub(super) fn respawn_shell_after_exit(&mut self, event: &AppEvent) -> bool {
        if self.state.should_quit.0 {
            return false;
        }
        let AppEvent::PaneDied { pane_id } = event else {
            return false;
        };
        let Some((workspace, pane)) = self.state.find_pane(*pane_id) else {
            return false;
        };
        let terminal_id = pane.attached_terminal_id.clone();
        let Some(terminal) = self.state.terminals.get(&terminal_id) else {
            return false;
        };
        if !terminal.respawn_shell_on_exit {
            return false;
        }
        let cwd = terminal.cwd.clone();
        let size = self.terminal_runtimes.get(&terminal_id).map_or_else(
            || self.state.estimate_pane_size(),
            PaneRuntime::current_size,
        );
        let Some(env) = self.pane_launch_env(workspace, *pane_id, Vec::new()) else {
            return false;
        };
        let runtime = match PaneRuntime::spawn(
            self.launch.runtime_config(&self.config, *pane_id, size),
            PaneShellConfig::new(
                &self.config.terminal.default_shell,
                self.config.terminal.shell_mode,
            ),
            cwd,
            &env,
        ) {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::warn!(pane = pane_id.raw(), terminal = %terminal_id, %error, "failed to respawn shell after launch command exited");
                return false;
            }
        };
        self.terminal_runtimes.insert(terminal_id.clone(), runtime);
        if let Some(terminal) = self.state.terminals.get_mut(&terminal_id) {
            terminal.clear_agent_runtime_identity_after_respawn();
        }
        self.state.focus_pane_in_workspace(workspace, *pane_id);
        self.state.session_dirty = true;
        self.overlay_panes.remove(pane_id);
        self.launch.render_dirty.request_generic();
        self.launch.render_notify.notify_one();
        true
    }

    /// Supplies public workspace, tab and pane identities to every native launch.
    pub(crate) fn pane_launch_env(
        &self,
        workspace: usize,
        pane: PaneId,
        extra: Vec<(String, String)>,
    ) -> Option<crate::pane::launch_env::PaneLaunchEnv> {
        let target = self.state.topology.workspaces.get(workspace)?;
        let tab = target.find_tab_index_for_pane(pane)?;
        Some(
            crate::pane::launch_env::PaneLaunchEnv::from_extra(extra).with_identity(
                target.id.clone(),
                self.state.public_tab_id(workspace, tab)?,
                self.state.public_pane_id(workspace, pane)?,
            ),
        )
    }
}
