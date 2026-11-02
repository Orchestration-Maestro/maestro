//! Pane geometry as the daemon serves it: tile cells composed with the view area and pane chrome.

use super::App;
use crate::settings::model::Config;
use maestro_protocol::{
    events::EventData,
    schema::{PaneDirection, PaneLayoutPane, PaneLayoutRect, PaneLayoutSnapshot, PaneLayoutSplit},
};
use maestro_workspace::layout::{
    Borders, NavDirection, PaneId, PaneInfo, Rect, find_in_direction, ranges_overlap,
};

/// Chrome switches that decide the cell each pane occupies and the borders it keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneChrome {
    /// Draw borders around split panes.
    pub borders: bool,
    /// Keep split panes apart instead of sharing divider borders.
    pub gaps: bool,
    /// Draw borders along the outside edge of the pane area.
    pub outer_borders: bool,
}

impl PaneChrome {
    /// Reads the accepted interface preferences.
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self {
            borders: config.ui.pane_borders.0,
            gaps: config.ui.pane_gaps.0,
            outer_borders: config.ui.pane_outer_borders.0,
        }
    }
}

/// Which sides of a pane touch another pane.
#[derive(Clone, Copy)]
struct Neighbours {
    /// Another pane starts where this one ends horizontally.
    right: bool,
    /// Another pane starts where this one ends vertically.
    below: bool,
}

/// Smallest rectangle that covers every pane cell.
#[derive(Clone, Copy)]
struct Extent {
    /// Leftmost column.
    left: u16,
    /// Topmost row.
    top: u16,
    /// Column after the rightmost cell.
    right: u16,
    /// Row after the bottom cell.
    bottom: u16,
}

impl Extent {
    /// Measures a set of pane cells.
    fn of(panes: &[PaneInfo]) -> Self {
        Self {
            left: panes.iter().map(|pane| pane.rect.x).min().unwrap_or(0),
            top: panes.iter().map(|pane| pane.rect.y).min().unwrap_or(0),
            right: panes
                .iter()
                .map(|pane| pane.rect.right())
                .max()
                .unwrap_or(0),
            bottom: panes
                .iter()
                .map(|pane| pane.rect.bottom())
                .max()
                .unwrap_or(0),
        }
    }
}

/// Whether another cell begins at this cell's right edge across a shared span of rows.
fn has_cell_to_right(pane: &PaneInfo, panes: &[PaneInfo]) -> bool {
    panes.iter().any(|other| {
        other.id != pane.id
            && other.rect.x == pane.rect.right()
            && ranges_overlap(
                pane.rect.y,
                pane.rect.height,
                other.rect.y,
                other.rect.height,
            )
    })
}

/// Whether another cell begins at this cell's bottom edge across a shared span of columns.
fn has_cell_below(pane: &PaneInfo, panes: &[PaneInfo]) -> bool {
    panes.iter().any(|other| {
        other.id != pane.id
            && other.rect.y == pane.rect.bottom()
            && ranges_overlap(pane.rect.x, pane.rect.width, other.rect.x, other.rect.width)
    })
}

/// Gives up one cell to the gap when the extent exceeds one cell.
fn shrink_for_one_cell_gap(size: u16) -> u16 {
    if size > 1 { size - 1 } else { size }
}

/// Leaves one empty cell before each neighbouring pane.
fn shrink_for_gap(rect: Rect, neighbours: Neighbours) -> Rect {
    Rect::new(
        rect.x,
        rect.y,
        if neighbours.right {
            shrink_for_one_cell_gap(rect.width)
        } else {
            rect.width
        },
        if neighbours.below {
            shrink_for_one_cell_gap(rect.height)
        } else {
            rect.height
        },
    )
}

/// Edges of a bordered pane that remain drawn.
fn drawn_borders(
    rect: Rect,
    chrome: PaneChrome,
    neighbours: Neighbours,
    extent: Extent,
) -> Borders {
    let mut borders = Borders::ALL;
    if !chrome.gaps {
        if neighbours.right {
            borders = borders.without(Borders::RIGHT);
        }
        if neighbours.below {
            borders = borders.without(Borders::BOTTOM);
        }
    }
    if !chrome.outer_borders {
        for (outer, edge) in [
            (rect.x == extent.left, Borders::LEFT),
            (rect.y == extent.top, Borders::TOP),
            (rect.right() == extent.right, Borders::RIGHT),
            (rect.bottom() == extent.bottom, Borders::BOTTOM),
        ] {
            if outer {
                borders = borders.without(edge);
            }
        }
    }
    borders
}

/// Applies the chrome switches to raw tile cells, adding borders, gaps and inner rectangles.
#[must_use]
pub fn apply_pane_chrome(panes: Vec<PaneInfo>, chrome: PaneChrome) -> Vec<PaneInfo> {
    let multi_pane = panes.len() > 1;
    let extent = Extent::of(&panes);
    let neighbours: Vec<_> = panes
        .iter()
        .map(|pane| Neighbours {
            right: multi_pane && has_cell_to_right(pane, &panes),
            below: multi_pane && has_cell_below(pane, &panes),
        })
        .collect();
    panes
        .into_iter()
        .zip(neighbours)
        .map(|(mut pane, neighbours)| {
            if chrome.gaps && !chrome.borders {
                pane.rect = shrink_for_gap(pane.rect, neighbours);
            }
            pane.borders = if multi_pane && chrome.borders {
                drawn_borders(pane.rect, chrome, neighbours, extent)
            } else {
                Borders::NONE
            };
            pane.inner_rect = pane_inner_rect(pane.rect, pane.borders);
            pane
        })
        .collect()
}

/// The cells inside a pane after its drawn borders, never inverted in a tiny cell.
#[must_use]
pub fn pane_inner_rect(area: Rect, borders: Borders) -> Rect {
    let left = u16::from(borders.contains(Borders::LEFT));
    let top = u16::from(borders.contains(Borders::TOP));
    let right = u16::from(borders.contains(Borders::RIGHT));
    let bottom = u16::from(borders.contains(Borders::BOTTOM));
    Rect::new(
        area.x.saturating_add(left).min(area.right()),
        area.y.saturating_add(top).min(area.bottom()),
        area.width.saturating_sub(left + right),
        area.height.saturating_sub(top + bottom),
    )
}

/// Names a divider by its position in the traversal and its path from the root.
fn split_path_id(index: usize, path: &[bool]) -> String {
    if path.is_empty() {
        return format!("split_{index}_root");
    }
    let path: String = path
        .iter()
        .map(|second| if *second { '1' } else { '0' })
        .collect();
    format!("split_{index}_{path}")
}

/// Converts a cell rectangle to its protocol record.
fn layout_rect(rect: Rect) -> PaneLayoutRect {
    PaneLayoutRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

/// Converts a protocol direction to the navigation direction.
pub(crate) fn navigation_direction(direction: PaneDirection) -> NavDirection {
    match direction {
        PaneDirection::Left => NavDirection::Left,
        PaneDirection::Right => NavDirection::Right,
        PaneDirection::Up => NavDirection::Up,
        PaneDirection::Down => NavDirection::Down,
    }
}

impl<C: FnMut() -> u128> App<C> {
    /// Projects one tab's cells and dividers as laid out in the current view area.
    #[must_use]
    pub fn pane_layout_snapshot(&self, workspace: usize, tab: usize) -> Option<PaneLayoutSnapshot> {
        let target = self.state.topology.workspaces.get(workspace)?;
        let record = target.tabs.get(tab)?;
        let area = self.state.view_area();
        let focused_pane_id = self
            .state
            .public_pane_id(workspace, record.layout.focused())?;
        let panes = apply_pane_chrome(
            record.layout.panes(area),
            PaneChrome::from_config(&self.config),
        )
        .into_iter()
        .filter_map(|pane| {
            Some(PaneLayoutPane {
                pane_id: self.state.public_pane_id(workspace, pane.id)?,
                focused: pane.is_focused,
                rect: layout_rect(pane.rect),
            })
        })
        .collect();
        let splits = record
            .layout
            .splits(area)
            .into_iter()
            .enumerate()
            .map(|(index, split)| PaneLayoutSplit {
                id: split_path_id(index, &split.path),
                direction: split.direction,
                ratio: split.ratio,
                rect: layout_rect(split.area),
            })
            .collect();
        Some(PaneLayoutSnapshot {
            workspace_id: target.id.clone(),
            tab_id: self.state.public_tab_id(workspace, tab)?,
            zoomed: record.zoomed,
            area: layout_rect(area),
            focused_pane_id,
            panes,
            splits,
        })
    }

    /// Publishes a tab's current layout, doing nothing when the tab is gone.
    pub fn emit_layout_updated_event(&self, workspace: usize, tab: usize) {
        if let Some(layout) = self.pane_layout_snapshot(workspace, tab) {
            self.emit_layout_updated_snapshot(layout);
        }
    }

    /// Publishes a layout already projected by the caller.
    pub(super) fn emit_layout_updated_snapshot(&self, layout: PaneLayoutSnapshot) {
        self.event_hub.push(EventData::LayoutUpdated { layout });
    }

    /// Finds the tab whose layout changes when `pane` is removed, if other panes remain in it.
    #[must_use]
    pub fn layout_update_target_after_pane_removal(
        &self,
        workspace: usize,
        pane: PaneId,
    ) -> Option<(usize, usize)> {
        let target = self.state.topology.workspaces.get(workspace)?;
        let tab = target.find_tab_index_for_pane(pane)?;
        (target.tabs.get(tab)?.layout.pane_count() > 1).then_some((workspace, tab))
    }

    /// Finds the neighbouring pane in a direction over the tab's tile cells.
    #[must_use]
    pub fn directional_pane_target(
        &self,
        workspace: usize,
        tab: usize,
        source: PaneId,
        direction: PaneDirection,
    ) -> Option<PaneId> {
        let record = self
            .state
            .topology
            .workspaces
            .get(workspace)?
            .tabs
            .get(tab)?;
        let panes = record.layout.panes(self.state.view_area());
        let source = panes.iter().find(|pane| pane.id == source)?;
        find_in_direction(source, navigation_direction(direction), &panes)
    }
}
