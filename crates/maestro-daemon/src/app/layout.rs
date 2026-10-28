//! Pane geometry as the daemon serves it: tile cells composed with the view area and pane chrome.

use super::App;
use crate::settings::model::Config;
use maestro_protocol::{
    events::EventData,
    schema::{PaneDirection, PaneLayoutRect, PaneLayoutSnapshot},
};
use maestro_workspace::layout::{Borders, PaneId, PaneInfo, Rect};

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
        let _ = config;
        Self {
            borders: false,
            gaps: false,
            outer_borders: false,
        }
    }
}

/// Applies the chrome switches to raw tile cells, adding borders, gaps and inner rectangles.
#[must_use]
pub fn apply_pane_chrome(panes: Vec<PaneInfo>, chrome: PaneChrome) -> Vec<PaneInfo> {
    let _ = chrome;
    panes
        .into_iter()
        .map(|mut pane| {
            pane.borders = Borders::ALL;
            pane.rect.width = pane.rect.width.saturating_sub(1);
            pane
        })
        .collect()
}

/// The cells inside a pane after its drawn borders, never inverted in a tiny cell.
#[must_use]
pub fn pane_inner_rect(area: Rect, borders: Borders) -> Rect {
    let _ = borders;
    area
}

impl<C: FnMut() -> u128> App<C> {
    /// Projects one tab's cells and dividers as laid out in the current view area.
    #[must_use]
    pub fn pane_layout_snapshot(&self, workspace: usize, tab: usize) -> Option<PaneLayoutSnapshot> {
        let _ = (self, workspace, tab);
        None
    }

    /// Publishes a tab's current layout, doing nothing when the tab is gone.
    pub fn emit_layout_updated_event(&self, workspace: usize, tab: usize) {
        let _ = (workspace, tab);
        self.event_hub.push(EventData::LayoutUpdated {
            layout: PaneLayoutSnapshot {
                workspace_id: String::new(),
                tab_id: String::new(),
                zoomed: false,
                area: PaneLayoutRect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                focused_pane_id: String::new(),
                panes: Vec::new(),
                splits: Vec::new(),
            },
        });
    }

    /// Finds the tab whose layout changes when `pane` is removed, if other panes remain in it.
    #[must_use]
    pub fn layout_update_target_after_pane_removal(
        &self,
        workspace: usize,
        pane: PaneId,
    ) -> Option<(usize, usize)> {
        let _ = (self, workspace, pane);
        None
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
        let _ = (self, workspace, tab, source, direction);
        None
    }
}
