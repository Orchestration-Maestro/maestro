//! Mutable daemon data with one topology allocator.

use crate::{settings::model::Config, terminal::state::TerminalState};
use maestro_workspace::terminal_id::TerminalId;
use maestro_workspace::{
    layout::{PaneId, PaneInfo},
    workspace::WorkspaceTopology,
};
use std::collections::HashMap;

/// Mutable workspace data retained independently of attached clients.
pub struct DaemonState<C> {
    /// Sole owner of workspace topology and public identity allocation.
    pub topology: WorkspaceTopology<C>,
    /// Retained terminal metadata keyed by the attached terminal identity.
    pub terminals: HashMap<TerminalId, TerminalState>,
    /// Active workspace display position, absent when no workspace is active.
    pub active: Option<usize>,
    /// Workspace selection position for navigation.
    pub selected: usize,
    /// Historical numeric spellings mapped to their retained pane identity.
    pub pane_id_aliases: HashMap<u32, PaneId>,
    /// Historical public spellings mapped to their retained pane identity.
    pub public_pane_id_aliases: HashMap<String, PaneId>,
    /// Virtual terminal dimensions in columns and rows before a view attaches.
    pub headless_size: (u16, u16),
    /// Most recently supplied pane geometry, in view order.
    pub pane_infos: Vec<PaneInfo>,
}

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Creates empty daemon state using accepted settings and an identity clock.
    #[must_use]
    pub fn new(config: &Config, clock: C) -> Self {
        Self {
            topology: WorkspaceTopology::new(clock),
            terminals: HashMap::new(),
            active: None,
            selected: 0,
            pane_id_aliases: HashMap::new(),
            public_pane_id_aliases: HashMap::new(),
            headless_size: config.headless_size(),
            pane_infos: Vec::new(),
        }
    }

    /// Estimates rows and columns from the first pane, or unattached dimensions.
    #[must_use]
    pub fn estimate_pane_size(&self) -> (u16, u16) {
        self.pane_infos
            .first()
            .map_or((self.headless_size.1, self.headless_size.0), |pane| {
                (pane.rect.height, pane.rect.width)
            })
    }
}
