//! Tab topology and transactional caller-owned spawning.
use crate::{
    layout::{PaneId, SplitRequest, TileLayout},
    pane_state::PaneState,
    terminal_id::TerminalId,
};
use std::{collections::HashMap, io};

/// Owned identity inputs for the caller's spawn operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnRequest {
    /// Workspace handle.
    pub workspace_id: String,
    /// Stable public tab number.
    pub tab_number: usize,
    /// Stable public pane number.
    pub pane_number: usize,
    /// Newly allocated terminal identity.
    pub terminal_id: TerminalId,
}

/// One split tree and its pane viewport records.
#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    /// Explicit label, including an explicitly empty label.
    pub custom_name: Option<String>,
    /// Stable public number.
    pub number: usize,
    /// Pane supplying the tab's identity.
    pub root_pane: PaneId,
    /// Split geometry and focus history.
    pub layout: TileLayout,
    /// Viewport records for live panes.
    pub panes: HashMap<PaneId, PaneState>,
    /// Whether the selected pane occupies the viewport.
    pub zoomed: bool,
}
impl Tab {
    /// Create a tab while returning the spawn result directly to its caller.
    ///
    /// # Errors
    /// Propagates the spawn error without retaining a tab.
    pub fn new_with_runtime<T>(
        pane_id: PaneId,
        request: SpawnRequest,
        spawn: impl FnOnce(PaneId, SpawnRequest) -> io::Result<T>,
    ) -> io::Result<(Self, T)> {
        let terminal = request.terminal_id.clone();
        let number = request.tab_number;
        let result = spawn(pane_id, request)?;
        Ok((
            Self {
                custom_name: None,
                number,
                root_pane: pane_id,
                layout: TileLayout::new(pane_id),
                panes: HashMap::from([(pane_id, PaneState::new(terminal))]),
                zoomed: false,
            },
            result,
        ))
    }

    /// Split without publishing viewport or focus changes until spawn succeeds.
    ///
    /// # Errors
    /// Returns `NotFound` for a missing target, or the original spawn error.
    pub fn split_pane_with_runtime<T>(
        &mut self,
        pane_id: PaneId,
        request: SplitRequest,
        spawn_request: SpawnRequest,
        spawn: impl FnOnce(PaneId, SpawnRequest) -> io::Result<T>,
    ) -> io::Result<(PaneId, T)> {
        let focus = request.focus_new_pane;
        self.layout.split_pane(pane_id, request).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "split target pane is not in the layout",
            )
        })?;
        let terminal = spawn_request.terminal_id.clone();
        let result = match spawn(pane_id, spawn_request) {
            Ok(result) => result,
            Err(error) => {
                self.layout.close_pane(pane_id);
                return Err(error);
            }
        };
        if focus {
            self.layout.focus_pane(pane_id);
        }
        self.panes.insert(pane_id, PaneState::new(terminal));
        self.zoomed = false;
        Ok((pane_id, result))
    }
}

/// Removed pane and terminal identities for runtime release by the caller.
pub type DetachedPane = (PaneId, TerminalId);
impl Tab {
    /// Detach a pane unless it is the last leaf.
    pub fn close_pane(&mut self, pane_id: PaneId) -> Option<DetachedPane> {
        self.detach_pane(pane_id)
    }
    /// Remove a pane without retaining a runtime.
    pub fn remove_pane(&mut self, pane_id: PaneId) -> Option<DetachedPane> {
        self.detach_pane(pane_id)
    }
    /// Remove viewport state and promote a surviving root when needed.
    fn detach_pane(&mut self, pane_id: PaneId) -> Option<DetachedPane> {
        if self.layout.pane_count() <= 1 {
            return None;
        }
        let next_root = self.promoted_root_if_needed(pane_id);
        self.layout.close_pane(pane_id);
        let pane = self.panes.remove(&pane_id)?;
        self.zoomed = false;
        if let Some(next_root) = next_root {
            self.root_pane = next_root;
        }
        Some((pane_id, pane.attached_terminal_id))
    }
    /// Select the first surviving leaf when removing the identity root.
    fn promoted_root_if_needed(&self, closing: PaneId) -> Option<PaneId> {
        if self.root_pane != closing {
            return None;
        }
        self.layout.pane_ids().into_iter().find(|id| *id != closing)
    }
    /// Borrow a pane's attached terminal identity.
    #[must_use]
    pub fn terminal_id(&self, pane_id: PaneId) -> Option<&TerminalId> {
        self.panes
            .get(&pane_id)
            .map(|pane| &pane.attached_terminal_id)
    }
}

/// A transferred pane retains its viewport and terminal identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedPane {
    /// Moving pane identity.
    pub pane_id: PaneId,
    /// Original viewport state.
    pub pane_state: PaneState,
}
impl Tab {
    /// Build a single-pane tab from transferred state.
    #[must_use]
    pub fn from_existing_pane(
        number: usize,
        custom_name: Option<String>,
        moved: MovedPane,
    ) -> Self {
        let pane = moved.pane_id;
        Self {
            custom_name,
            number,
            root_pane: pane,
            layout: TileLayout::new(pane),
            panes: HashMap::from([(pane, moved.pane_state)]),
            zoomed: false,
        }
    }
    /// Take a pane, permitting the sole pane to leave a transient empty source.
    pub fn take_pane_for_move(&mut self, pane_id: PaneId) -> Option<MovedPane> {
        if !self.panes.contains_key(&pane_id) {
            return None;
        }
        if self.layout.pane_count() > 1 {
            let next_root = self.promoted_root_if_needed(pane_id);
            self.layout.close_pane(pane_id);
            if let Some(next_root) = next_root {
                self.root_pane = next_root;
            }
        }
        let pane_state = self.panes.remove(&pane_id)?;
        self.zoomed = false;
        Some(MovedPane {
            pane_id,
            pane_state,
        })
    }
    /// Insert transferred viewport state, returning it unchanged on rejection.
    ///
    /// # Errors
    /// Returns the moved record when the target is missing or the identity already exists.
    pub fn insert_existing_pane(
        &mut self,
        moved: MovedPane,
        request: &SplitRequest,
    ) -> Result<PaneId, MovedPane> {
        if !self.layout.insert_pane_near(moved.pane_id, request) {
            return Err(moved);
        }
        let pane = moved.pane_id;
        self.panes.insert(pane, moved.pane_state);
        self.zoomed = false;
        Ok(pane)
    }

    /// Whether the displayed name follows the tab's position.
    #[must_use]
    pub fn is_auto_named(&self) -> bool {
        self.custom_name.is_none()
    }
    /// Set an explicit name, including an empty label.
    pub fn set_custom_name(&mut self, name: String) {
        self.custom_name = Some(name);
    }
}
