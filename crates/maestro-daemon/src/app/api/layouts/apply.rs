//! `layout.apply`: builds a requested tree as a new tab, replacing a tab when one is named.

use super::tree::{first_leaf, leaf_launch, validate_layout_tree};
use crate::app::{App, api::responses::api_error, creation::RootLaunch, state::Mode};
use maestro_protocol::{
    events::EventData,
    schema::{ErrorBody, LayoutApplyParams, LayoutNode, LayoutPane, ResponseResult},
};
use maestro_workspace::layout::{PaneId, SplitRequest};
use std::path::PathBuf;

impl<C: FnMut() -> u128> App<C> {
    /// Starts one process per leaf of the requested tree in a new tab of the named, owning or
    /// active workspace. A named tab is closed once the new one stands, and the new tab is
    /// activated when asked or when it replaces the active tab.
    ///
    /// # Errors
    /// Returns `tab_not_found`, `invalid_target` or `workspace_not_found` for a bad target,
    /// `invalid_layout` for a tree outside the limits and `layout_apply_failed` when a
    /// process cannot start; the new tab and its processes are then removed again.
    pub(in crate::app::api) async fn handle_layout_apply(
        &mut self,
        params: LayoutApplyParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let (workspace, replaced) = self.layout_apply_scope(&params)?;
        validate_layout_tree(&params.root)?;
        let target = &self.state.topology.workspaces[workspace];
        let name = params
            .tab_label
            .clone()
            .or_else(|| replaced.and_then(|tab| target.tabs[tab].custom_name.clone()));
        let replaced_was_active = replaced
            .is_some_and(|tab| self.state.active == Some(workspace) && target.active_tab == tab);
        let root_pane = self
            .spawn_layout_root(workspace, replaced, &params.root)
            .await?;
        if let Err(message) = self.build_layout(workspace, root_pane, &params.root).await {
            self.discard_layout_tab(workspace, root_pane).await;
            return Err(api_error("layout_apply_failed", message));
        }
        if let Some(tab) = replaced {
            self.close_replaced_tab(workspace, tab).await;
        }
        let tab = self
            .tab_of_root(workspace, root_pane)
            .ok_or_else(|| api_error("layout_apply_failed", "new layout tab disappeared"))?;
        if let Some(name) = name {
            self.state.topology.workspaces[workspace].tabs[tab].set_custom_name(name);
        }
        if params.focus || replaced_was_active {
            self.state.switch_workspace_tab(workspace, tab);
            self.state.mode = Mode::Terminal;
        }
        self.state.session_dirty = true;
        self.emit_tab_created_events(workspace, tab).await;
        let layout = self
            .layout_description(workspace, tab)
            .ok_or_else(|| api_error("layout_apply_failed", "new layout unavailable"))?;
        Ok(ResponseResult::LayoutApply { layout })
    }

    /// Resolves the workspace a layout is built in and the tab it replaces, if any.
    fn layout_apply_scope(
        &self,
        params: &LayoutApplyParams,
    ) -> Result<(usize, Option<usize>), ErrorBody> {
        let replaced = match params.tab_id.as_deref() {
            Some(id) => Some(self.target_tab(id)?),
            None => None,
        };
        if replaced.is_some() && params.workspace_id.is_some() {
            return Err(api_error(
                "invalid_target",
                "use either tab_id or workspace_id, not both",
            ));
        }
        let workspace = match (&replaced, params.workspace_id.as_deref()) {
            (Some((workspace, _)), _) => *workspace,
            (None, Some(id)) => self.target_workspace(id)?,
            (None, None) => self
                .state
                .active
                .ok_or_else(|| api_error("workspace_not_found", "no active workspace"))?,
        };
        Ok((workspace, replaced.map(|(_, tab)| tab)))
    }

    /// Adds the new tab, whose root pane runs the tree's first leaf, and returns that pane.
    async fn spawn_layout_root(
        &mut self,
        workspace: usize,
        replaced: Option<usize>,
        root: &LayoutNode,
    ) -> Result<PaneId, ErrorBody> {
        let leaf = first_leaf(root);
        let launch = leaf_launch(leaf)?;
        let cwd = match leaf.cwd.as_deref() {
            Some(cwd) => PathBuf::from(cwd),
            None => self.layout_root_cwd(workspace, replaced).await,
        };
        let root_launch = RootLaunch {
            cwd,
            extra_env: launch.extra_env,
            argv: launch.argv,
        };
        let tab = self
            .create_tab_with_launch(workspace, root_launch)
            .map_err(|error| api_error("layout_apply_failed", error.to_string()))?;
        let pane = self.state.topology.workspaces[workspace].tabs[tab].root_pane;
        self.label_layout_pane(workspace, pane, leaf);
        Ok(pane)
    }

    /// The directory the new tab starts in when its first leaf names none: the replaced tab's
    /// focused pane's, else the workspace's focused pane's, under the configured policy.
    async fn layout_root_cwd(&self, workspace: usize, replaced: Option<usize>) -> PathBuf {
        let Some(tab) = replaced else {
            return self.new_tab_cwd(workspace).await;
        };
        let focused = self.state.topology.workspaces[workspace].tabs[tab]
            .layout
            .focused();
        self.new_pane_cwd(workspace, focused).await
    }

    /// Splits the tab's root pane into the tree, starting the leaves in layout order.
    async fn build_layout(
        &mut self,
        workspace: usize,
        root_pane: PaneId,
        root: &LayoutNode,
    ) -> Result<(), String> {
        let mut pending = vec![(root_pane, root)];
        while let Some((pane, node)) = pending.pop() {
            match node {
                LayoutNode::Pane { pane: leaf } => self.label_layout_pane(workspace, pane, leaf),
                LayoutNode::Split {
                    direction,
                    ratio,
                    first,
                    second,
                } => {
                    let request = SplitRequest {
                        target: pane,
                        direction: direction.clone(),
                        ratio: Some(*ratio),
                        focus_new_pane: false,
                    };
                    let created = self
                        .split_layout_pane(workspace, request, first_leaf(second))
                        .await?;
                    pending.push((created, second.as_ref()));
                    pending.push((pane, first.as_ref()));
                }
            }
        }
        Ok(())
    }

    /// Splits `request.target` and starts `leaf` in the new pane.
    async fn split_layout_pane(
        &mut self,
        workspace: usize,
        request: SplitRequest,
        leaf: &LayoutPane,
    ) -> Result<PaneId, String> {
        let launch = leaf_launch(leaf).map_err(|error| error.message)?;
        let cwd = match leaf.cwd.as_deref() {
            Some(cwd) => PathBuf::from(cwd),
            None => self
                .pane_cwd_in_workspace(workspace, request.target)
                .await
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| PathBuf::from("/")),
        };
        let root_launch = RootLaunch {
            cwd,
            extra_env: launch.extra_env,
            argv: launch.argv,
        };
        let pane = self
            .spawn_split(workspace, request, root_launch)
            .ok_or("pane not found")?
            .map_err(|error| error.to_string())?;
        self.label_layout_pane(workspace, pane, leaf);
        Ok(pane)
    }

    /// Gives a new pane the label its leaf asks for.
    fn label_layout_pane(&mut self, workspace: usize, pane: PaneId, leaf: &LayoutPane) {
        let Some(label) = &leaf.label else {
            return;
        };
        let terminal = self
            .state
            .topology
            .workspaces
            .get(workspace)
            .and_then(|target| target.terminal_id(pane))
            .and_then(|terminal| self.state.terminals.get_mut(terminal));
        if let Some(terminal) = terminal {
            terminal.set_manual_label(label);
        }
    }

    /// The display position of the tab whose root pane is `root_pane`.
    fn tab_of_root(&self, workspace: usize, root_pane: PaneId) -> Option<usize> {
        self.state
            .topology
            .workspaces
            .get(workspace)?
            .tabs
            .iter()
            .position(|tab| tab.root_pane == root_pane)
    }

    /// Removes a tab built for a layout that failed, with its processes.
    async fn discard_layout_tab(&mut self, workspace: usize, root_pane: PaneId) {
        if let Some(tab) = self.tab_of_root(workspace, root_pane) {
            self.retire_tab_and_stop_panes(workspace, tab).await;
        }
    }

    /// Closes the tab a finished layout replaces and publishes its closing.
    async fn close_replaced_tab(&mut self, workspace: usize, tab: usize) {
        let closed = self.state.public_tab_id(workspace, tab);
        if !self.retire_tab_and_stop_panes(workspace, tab).await {
            return;
        }
        if let Some(tab_id) = closed {
            let workspace_id = self.state.topology.workspaces[workspace].id.clone();
            self.event_hub.push(EventData::TabClosed {
                tab_id,
                workspace_id,
            });
        }
    }
}
