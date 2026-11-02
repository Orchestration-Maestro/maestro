//! Layout commands: export a tab as a portable tree, set a split ratio and, in the child
//! modules, apply a tree as a new or replacement tab.

mod apply;
mod describe;
mod tree;

use super::{super::App, responses::api_error};
use maestro_protocol::schema::{
    ErrorBody, LayoutDescription, LayoutExportParams, LayoutSetSplitRatioParams, ResponseResult,
};

impl<C: FnMut() -> u128> App<C> {
    /// Describes the tab named by `tab_id`, or holding `pane_id`, or the active tab.
    ///
    /// # Errors
    /// Returns `layout_not_found` for an unknown or ambiguous target.
    pub(super) fn handle_layout_export(
        &self,
        params: &LayoutExportParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let (workspace, tab) =
            self.layout_tab(params.tab_id.as_deref(), params.pane_id.as_deref())?;
        Ok(ResponseResult::LayoutExport {
            layout: self.described_layout(workspace, tab)?,
        })
    }

    /// Sets the ratio of the split at `path`, keeping it within the range a split allows.
    ///
    /// # Errors
    /// Returns `invalid_ratio` for a ratio that is not finite, `layout_not_found` for an
    /// unknown target and `split_not_found` when the path names no split.
    pub(super) fn handle_layout_set_split_ratio(
        &mut self,
        params: &LayoutSetSplitRatioParams,
    ) -> Result<ResponseResult, ErrorBody> {
        if !params.ratio.is_finite() {
            return Err(api_error("invalid_ratio", "ratio must be finite"));
        }
        let (workspace, tab) =
            self.layout_tab(params.tab_id.as_deref(), params.pane_id.as_deref())?;
        let layout = &mut self.state.topology.workspaces[workspace].tabs[tab].layout;
        if !layout.set_ratio_at(&params.path, params.ratio) {
            return Err(api_error("split_not_found", "split path not found"));
        }
        self.state.session_dirty = true;
        let layout = self.described_layout(workspace, tab)?;
        self.emit_layout_updated_event(workspace, tab);
        Ok(ResponseResult::LayoutSplitRatioSet { layout })
    }

    /// Resolves the tab named by `tab_id` or holding `pane_id`, or the active tab of the
    /// active workspace when neither is named.
    fn layout_tab(
        &self,
        tab_id: Option<&str>,
        pane_id: Option<&str>,
    ) -> Result<(usize, usize), ErrorBody> {
        self.find_layout_tab(tab_id, pane_id)
            .ok_or_else(|| api_error("layout_not_found", "layout target not found"))
    }

    /// The tab a layout target names; `None` when it names none or names two.
    fn find_layout_tab(
        &self,
        tab_id: Option<&str>,
        pane_id: Option<&str>,
    ) -> Option<(usize, usize)> {
        match (tab_id, pane_id) {
            (Some(_), Some(_)) => None,
            (Some(id), None) => self.state.parse_tab_id(id),
            (None, Some(id)) => self.locate_by_handle(id).map(|at| (at.workspace, at.tab)),
            (None, None) => {
                let workspace = self.state.active?;
                Some((
                    workspace,
                    self.state.topology.workspaces[workspace].active_tab,
                ))
            }
        }
    }

    /// Describes a live tab for a command result.
    fn described_layout(
        &self,
        workspace: usize,
        tab: usize,
    ) -> Result<LayoutDescription, ErrorBody> {
        self.layout_description(workspace, tab)
            .ok_or_else(|| api_error("layout_not_found", "layout unavailable"))
    }
}
