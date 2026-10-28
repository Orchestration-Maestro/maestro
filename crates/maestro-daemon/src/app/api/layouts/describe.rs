//! A tab's split tree as the portable description, without geometry.

use crate::app::App;
use maestro_protocol::schema::{LayoutDescription, LayoutNode, LayoutPane};
use maestro_workspace::layout::{Node, PaneId};
use std::collections::HashMap;

impl<C: FnMut() -> u128> App<C> {
    /// Describes a tab's tree with the launch facts of each pane; `None` when the tab is gone.
    pub(super) fn layout_description(
        &self,
        workspace: usize,
        tab: usize,
    ) -> Option<LayoutDescription> {
        let target = self.state.topology.workspaces.get(workspace)?;
        let record = target.tabs.get(tab)?;
        Some(LayoutDescription {
            workspace_id: target.id.clone(),
            tab_id: self.state.public_tab_id(workspace, tab)?,
            zoomed: record.zoomed,
            focused_pane_id: self
                .state
                .public_pane_id(workspace, record.layout.focused())?,
            root: self.layout_node(workspace, tab, record.layout.root())?,
        })
    }

    /// Describes one node and everything below it.
    fn layout_node(&self, workspace: usize, tab: usize, node: &Node) -> Option<LayoutNode> {
        Some(match node {
            Node::Pane(pane) => LayoutNode::Pane {
                pane: self.layout_pane(workspace, tab, *pane)?,
            },
            Node::Split {
                direction,
                ratio,
                first,
                second,
            } => LayoutNode::Split {
                direction: direction.clone(),
                ratio: *ratio,
                first: Box::new(self.layout_node(workspace, tab, first)?),
                second: Box::new(self.layout_node(workspace, tab, second)?),
            },
        })
    }

    /// Describes one pane: its handle, label, directory and launch command.
    fn layout_pane(&self, workspace: usize, tab: usize, pane: PaneId) -> Option<LayoutPane> {
        let record = self
            .state
            .topology
            .workspaces
            .get(workspace)?
            .tabs
            .get(tab)?;
        let terminal = self.state.terminals.get(record.terminal_id(pane)?);
        Some(LayoutPane {
            pane_id: Some(self.state.public_pane_id(workspace, pane)?),
            label: terminal.and_then(|terminal| terminal.manual_label.clone()),
            cwd: crate::tab::cwd_for_pane(
                record,
                pane,
                &self.state.terminals,
                &self.terminal_runtimes,
            )
            .map(|cwd| cwd.display().to_string()),
            command: terminal.and_then(|terminal| terminal.launch_argv.clone()),
            env: HashMap::new(),
        })
    }
}
