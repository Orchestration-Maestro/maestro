//! Tab commands: list, get, create, focus, rename, move and close.

use super::{
    super::{App, state::Mode},
    env::normalize_launch_env,
    responses::{api_error, tab_not_found, workspace_not_found},
};
use maestro_protocol::{
    events::EventData,
    schema::{ErrorBody, ResponseResult, TabCreateParams, TabInfo, TabMoveParams, TabRenameParams},
};
use std::{collections::HashSet, path::PathBuf};

impl<C: FnMut() -> u128> App<C> {
    /// Lists the tabs of one workspace, or of every workspace in display order.
    pub(super) fn handle_tab_list(
        &self,
        workspace_id: Option<&str>,
    ) -> Result<ResponseResult, ErrorBody> {
        let tabs = match workspace_id {
            Some(id) => self.tab_list_info(self.target_workspace(id)?),
            None => (0..self.state.topology.workspaces.len())
                .flat_map(|workspace| self.tab_list_info(workspace))
                .collect(),
        };
        Ok(ResponseResult::TabList { tabs })
    }

    /// Reads one tab.
    pub(super) fn handle_tab_get(&self, id: &str) -> Result<ResponseResult, ErrorBody> {
        let (workspace, tab) = self.target_tab(id)?;
        self.tab_info_result(workspace, tab, id)
    }

    /// Adds a tab to the supplied or active workspace and publishes its records.
    pub(super) async fn handle_tab_create(
        &mut self,
        params: TabCreateParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let workspace = match params.workspace_id.as_deref() {
            Some(id) => self
                .state
                .parse_workspace_id(id)
                .ok_or_else(|| workspace_not_found(id))?,
            None => self
                .state
                .active
                .ok_or_else(|| api_error("workspace_not_found", "no active workspace"))?,
        };
        let cwd = match params.cwd {
            Some(cwd) => PathBuf::from(cwd),
            None => self.new_tab_cwd(workspace).await,
        };
        let extra_env = normalize_launch_env(params.env)?;
        let tab = self
            .create_tab_with_launch_env(workspace, cwd, extra_env)
            .map_err(|error| api_error("tab_create_failed", error.to_string()))?;
        if let Some(label) = params.label
            && let Some(tab_id) = self.state.public_tab_id(workspace, tab)
        {
            let target = &mut self.state.topology.workspaces[workspace];
            target.tabs[tab].set_custom_name(label);
            crate::logging::tab_renamed(&target.id, &tab_id);
        }
        if params.focus {
            self.state.switch_workspace_tab(workspace, tab);
            self.state.mode = Mode::Terminal;
        }
        self.state.session_dirty = true;
        self.emit_tab_created_events(workspace, tab).await;
        self.tab_created_result(workspace, tab)
            .await
            .ok_or_else(|| api_error("tab_create_failed", "workspace disappeared"))
    }

    /// Activates one tab and its workspace.
    pub(super) fn handle_tab_focus(&mut self, id: &str) -> Result<ResponseResult, ErrorBody> {
        let (workspace, tab) = self.target_tab(id)?;
        self.state.switch_workspace_tab(workspace, tab);
        self.tab_info_result(workspace, tab, id)
    }

    /// Names one tab and publishes the new label.
    pub(super) fn handle_tab_rename(
        &mut self,
        params: TabRenameParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let (workspace, tab) = self.target_tab(&params.tab_id)?;
        let tab_id = self.public_tab_id(workspace, tab, &params.tab_id)?;
        let target = &mut self.state.topology.workspaces[workspace];
        target.tabs[tab].set_custom_name(params.label.clone());
        crate::logging::tab_renamed(&target.id, &tab_id);
        let workspace_id = target.id.clone();
        self.state.session_dirty = true;
        self.event_hub.push(EventData::TabRenamed {
            tab_id,
            workspace_id,
            label: params.label,
        });
        self.tab_info_result(workspace, tab, &params.tab_id)
    }

    /// Moves one tab within its workspace; only a real change is published.
    pub(super) fn handle_tab_move(
        &mut self,
        params: &TabMoveParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let (workspace, tab) = self.target_tab(&params.tab_id)?;
        if params.insert_index > self.state.topology.workspaces[workspace].tabs.len() {
            return Err(api_error(
                "tab_move_failed",
                format!("insert_index {} is out of bounds", params.insert_index),
            ));
        }
        let tab_id = self.public_tab_id(workspace, tab, &params.tab_id)?;
        let target = &mut self.state.topology.workspaces[workspace];
        let moved = target.move_tab(tab, params.insert_index);
        let workspace_id = target.id.clone();
        let tabs = self.tab_list_info(workspace);
        if moved {
            self.state.session_dirty = true;
            self.event_hub.push(EventData::TabMoved {
                tab_id,
                workspace_id,
                insert_index: params.insert_index,
                tabs: tabs.clone(),
            });
        }
        Ok(ResponseResult::TabList { tabs })
    }

    /// Closes one tab with its owned terminals; the last tab closes its workspace too.
    pub(super) async fn handle_tab_close(&mut self, id: &str) -> Result<ResponseResult, ErrorBody> {
        let (workspace, tab) = self.target_tab(id)?;
        let tab_id = self.public_tab_id(workspace, tab, id)?;
        let target = &self.state.topology.workspaces[workspace];
        let workspace_id = target.id.clone();
        if target.tabs.len() <= 1 {
            if self.state.confirm_implicit_worktree_group_close(workspace) {
                return Err(api_error(
                    "confirmation_required",
                    "closing this tab would close a worktree group",
                ));
            }
            let record = self.workspace_info(workspace);
            self.state.selected = workspace;
            self.state.close_selected_workspace();
            self.shutdown_detached_terminal_runtimes().await;
            self.event_hub.push(EventData::TabClosed {
                tab_id,
                workspace_id: workspace_id.clone(),
            });
            self.event_hub.push(EventData::WorkspaceClosed {
                workspace_id,
                workspace: record,
            });
            return Ok(ResponseResult::Ok {});
        }
        let closing = &target.tabs[tab];
        let panes: HashSet<_> = closing.panes.keys().copied().collect();
        let terminals = closing
            .panes
            .values()
            .map(|pane| pane.attached_terminal_id.clone())
            .collect();
        if !self.state.topology.workspaces[workspace].close_tab(tab) {
            return Err(api_error(
                "tab_close_failed",
                format!("tab {id} could not be closed"),
            ));
        }
        self.state.remove_plugin_pane_records(&panes);
        self.state.remove_unattached_terminal_ids(terminals);
        self.shutdown_detached_terminal_runtimes().await;
        self.state.session_dirty = true;
        self.event_hub.push(EventData::TabClosed {
            tab_id,
            workspace_id,
        });
        Ok(ResponseResult::Ok {})
    }

    /// Resolves a tab target to its live workspace and tab display positions.
    fn target_tab(&self, id: &str) -> Result<(usize, usize), ErrorBody> {
        self.state.parse_tab_id(id).ok_or_else(|| tab_not_found(id))
    }

    /// The stable public handle of a live tab.
    fn public_tab_id(&self, workspace: usize, tab: usize, id: &str) -> Result<String, ErrorBody> {
        self.state
            .public_tab_id(workspace, tab)
            .ok_or_else(|| tab_not_found(id))
    }

    /// The record of a live tab as a command result.
    fn tab_info_result(
        &self,
        workspace: usize,
        tab: usize,
        id: &str,
    ) -> Result<ResponseResult, ErrorBody> {
        self.tab_info(workspace, tab)
            .map(|tab| ResponseResult::TabInfo { tab })
            .ok_or_else(|| tab_not_found(id))
    }

    /// The records of a workspace's tabs in display order.
    fn tab_list_info(&self, workspace: usize) -> Vec<TabInfo> {
        let count = self
            .state
            .topology
            .workspaces
            .get(workspace)
            .map_or(0, |target| target.tabs.len());
        (0..count)
            .filter_map(|tab| self.tab_info(workspace, tab))
            .collect()
    }
}
