//! Active public identity and live directory supplied to status commands.

use crate::app::App;
use std::path::PathBuf;

impl<C: FnMut() -> u128> App<C> {
    /// Resolves command context once per launch pass, preferring the live process cwd.
    pub(crate) fn custom_command_env(&self) -> (Vec<(String, String)>, Option<PathBuf>) {
        let mut environment = vec![(
            "MAESTRO_SOCKET_PATH".into(),
            self.launch.socket.display().to_string(),
        )];
        if let Some(executable) = self
            .launch
            .executable
            .clone()
            .or_else(|| std::env::current_exe().ok())
        {
            environment.push(("MAESTRO_BIN_PATH".into(), executable.display().to_string()));
        }
        let Some(index) = self.state.active else {
            return (environment, None);
        };
        let Some(workspace) = self.state.topology.workspaces.get(index) else {
            return (environment, None);
        };
        environment.push(("MAESTRO_ACTIVE_WORKSPACE_ID".into(), workspace.id.clone()));
        let tab_index = workspace.active_tab;
        if let Some(id) = self.state.public_tab_id(index, tab_index) {
            environment.push(("MAESTRO_ACTIVE_TAB_ID".into(), id));
        }
        let Some(tab) = workspace.tabs.get(tab_index) else {
            return (environment, None);
        };
        let pane = tab.layout.focused();
        if let Some(id) = self.state.public_pane_id(index, pane) {
            environment.push(("MAESTRO_ACTIVE_PANE_ID".into(), id));
        }
        let cwd = tab.terminal_id(pane).and_then(|terminal| {
            self.terminal_runtimes
                .get(terminal)
                .and_then(crate::panes::lifecycle::PaneRuntime::cwd)
                .or_else(|| {
                    self.state
                        .terminals
                        .get(terminal)
                        .map(|terminal| terminal.cwd.clone())
                })
        });
        if let Some(cwd) = &cwd {
            environment.push(("MAESTRO_ACTIVE_PANE_CWD".into(), cwd.display().to_string()));
        }
        (environment, cwd.filter(|cwd| cwd.is_dir()))
    }
}
