//! `pane.swap`: exchanges the places of two panes of one tab.

use super::{super::responses::api_error, Located};
use crate::app::App;
use maestro_protocol::schema::{
    ErrorBody, PaneDirection, PaneSwapParams, PaneSwapReason, PaneSwapResult, ResponseResult,
};
use maestro_workspace::layout::PaneId;

/// What a swap request resolved to: the tab it describes and, when the panes may be
/// exchanged, which ones.
struct SwapPlan {
    /// Workspace and tab positions of the layout the answer describes.
    tab: (usize, usize),
    /// Handle reported for the source pane.
    source_id: String,
    /// Handle reported for the target pane, when the request had one.
    target_id: Option<String>,
    /// The source and target to exchange; absent when `reason` explains why not.
    exchange: Option<(PaneId, PaneId)>,
    /// Why nothing moves.
    reason: Option<PaneSwapReason>,
}

/// The failure for a request that mixes or omits the two ways to name a swap.
fn invalid_swap(message: &str) -> ErrorBody {
    api_error("invalid_pane_swap", message)
}

impl<C: FnMut() -> u128> App<C> {
    /// Exchanges two panes of one tab, or the pane and its neighbour in a direction.
    ///
    /// The source ends up focused. A swap that cannot happen is answered, not refused.
    pub(super) fn handle_pane_swap(
        &mut self,
        params: &PaneSwapParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let plan = self.plan_swap(params)?;
        let changed = plan
            .exchange
            .is_some_and(|(source, target)| self.exchange_panes(plan.tab, source, target));
        let layout = self.layout_at(plan.tab.0, plan.tab.1)?;
        if changed {
            self.emit_layout_updated_snapshot(layout.clone());
        }
        Ok(ResponseResult::PaneSwap {
            swap: PaneSwapResult {
                changed,
                reason: plan.reason,
                source_pane_id: plan.source_id,
                target_pane_id: plan.target_id,
                focused_pane_id: layout.focused_pane_id.clone(),
                layout,
            },
        })
    }

    /// Resolves a request into the panes to exchange, or the reason it cannot be done.
    fn plan_swap(&self, params: &PaneSwapParams) -> Result<SwapPlan, ErrorBody> {
        let explicit = params.source_pane_id.is_some() || params.target_pane_id.is_some();
        match params.direction {
            Some(direction) if !explicit => {
                self.plan_directional_swap(params.pane_id.as_deref(), direction)
            }
            None if explicit => self.plan_explicit_swap(params),
            _ => Err(invalid_swap(
                "provide either direction with optional pane_id, or source_pane_id and target_pane_id",
            )),
        }
    }

    /// Plans a swap of the named or focused pane with its neighbour in a direction.
    fn plan_directional_swap(
        &self,
        pane_id: Option<&str>,
        direction: PaneDirection,
    ) -> Result<SwapPlan, ErrorBody> {
        let at = self
            .located_pane(pane_id)
            .map_err(|_| api_error("pane_not_found", "source pane not found"))?;
        let target = self.directional_pane_target(at.workspace, at.tab, at.pane, direction);
        Ok(SwapPlan {
            tab: (at.workspace, at.tab),
            source_id: self.public_pane(at.workspace, at.pane)?,
            target_id: target.and_then(|pane| self.state.public_pane_id(at.workspace, pane)),
            exchange: target.map(|target| (at.pane, target)),
            reason: target.is_none().then_some(PaneSwapReason::NoNeighbor),
        })
    }

    /// Plans a swap of two named panes, describing a tab even when one of them is missing.
    fn plan_explicit_swap(&self, params: &PaneSwapParams) -> Result<SwapPlan, ErrorBody> {
        let source_raw = params
            .source_pane_id
            .as_deref()
            .ok_or_else(|| invalid_swap("missing source_pane_id"))?;
        let target_raw = params
            .target_pane_id
            .as_deref()
            .ok_or_else(|| invalid_swap("missing target_pane_id"))?;
        let source = self.locate_by_handle(source_raw);
        let target = self.locate_by_handle(target_raw);
        let tab = source
            .or(target)
            .map(|at| (at.workspace, at.tab))
            .or_else(|| {
                let workspace = self.state.active?;
                Some((
                    workspace,
                    self.state.topology.workspaces.get(workspace)?.active_tab,
                ))
            })
            .ok_or_else(|| api_error("pane_layout_unavailable", "pane layout unavailable"))?;
        let reason = match (source, target) {
            (None, _) | (_, None) => Some(PaneSwapReason::NotFound),
            (Some(source), Some(target)) if source.pane == target.pane => {
                Some(PaneSwapReason::SamePane)
            }
            (Some(source), Some(target))
                if (source.workspace, source.tab) != (target.workspace, target.tab) =>
            {
                Some(PaneSwapReason::CrossTab)
            }
            _ => None,
        };
        Ok(SwapPlan {
            tab,
            source_id: self.handle_or(source, source_raw),
            target_id: Some(self.handle_or(target, target_raw)),
            exchange: source
                .zip(target)
                .filter(|_| reason.is_none())
                .map(|(source, target)| (source.pane, target.pane)),
            reason,
        })
    }

    /// The current handle of a located pane, or the text the request used.
    fn handle_or(&self, at: Option<Located>, raw: &str) -> String {
        at.and_then(|at| self.state.public_pane_id(at.workspace, at.pane))
            .unwrap_or_else(|| raw.to_owned())
    }

    /// Exchanges two panes of the tab and focuses the source, reporting whether anything moved.
    fn exchange_panes(&mut self, tab: (usize, usize), source: PaneId, target: PaneId) -> bool {
        let (workspace, tab) = tab;
        let previous_focus = self.state.current_pane_focus_target();
        let Some(record) = self
            .state
            .topology
            .workspaces
            .get_mut(workspace)
            .and_then(|target| target.tabs.get_mut(tab))
        else {
            return false;
        };
        let changed = record.layout.swap_panes(source, target);
        record.layout.focus_pane(source);
        if changed {
            self.state.switch_workspace_tab(workspace, tab);
            self.state
                .record_pane_focus_change(previous_focus, workspace, source);
        }
        changed
    }
}
