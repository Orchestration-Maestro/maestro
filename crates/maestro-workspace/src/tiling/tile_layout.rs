//! Layout edits and focus history.
use super::{
    geometry::{Borders, Rect},
    navigation::{nearest_resize_split, opposite_direction},
    tree::{
        collect_ids, collect_panes, collect_splits, count_panes, get_ratio_at, remove_pane,
        set_ratio_at, split_at, split_ratios, swap_pane_ids, valid_split_ratio,
    },
};
use maestro_protocol::schema::SplitDirection;
/// Allocator-assigned pane identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PaneId(u32);
impl PaneId {
    /// Return the numeric identity.
    #[must_use]
    pub fn raw(self) -> u32 {
        self.0
    }
    /// Reconstruct a saved identity for admission by the topology owner.
    #[must_use]
    pub fn from_raw(id: u32) -> Self {
        Self(id)
    }
}
/// A pane's cell geometry and focus state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneInfo {
    /// Pane identity.
    pub id: PaneId,
    /// Outer cell rectangle.
    pub rect: Rect,
    /// Content cell rectangle.
    pub inner_rect: Rect,
    /// Optional scrollbar lane.
    pub scrollbar_rect: Option<Rect>,
    /// Decorations applied by the caller.
    pub borders: Borders,
    /// Whether this pane is selected.
    pub is_focused: bool,
}
/// A binary split tree in leaf order.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// A pane leaf.
    Pane(PaneId),
    /// Two adjacent child regions.
    Split {
        /// Split axis.
        direction: SplitDirection,
        /// First child's fraction of the region.
        ratio: f32,
        /// First child in layout order.
        first: Box<Node>,
        /// Second child in layout order.
        second: Box<Node>,
    },
}
/// A split tree and its selected pane.
#[derive(Debug, Clone, PartialEq)]
pub struct TileLayout {
    /// Tree root.
    root: Node,
    /// Selected identity.
    focus: PaneId,
    /// Last selected live identity, consumed on focused close.
    prev_focus: Option<PaneId>,
}

/// Requested target, axis and independent ratio/focus controls.
#[derive(Debug, Clone)]
pub struct SplitRequest {
    /// Target pane.
    pub target: PaneId,
    /// Split axis.
    pub direction: SplitDirection,
    /// First-child ratio, defaulting to one half.
    pub ratio: Option<f32>,
    /// Select the new pane after a successful insertion.
    pub focus_new_pane: bool,
}
/// A split divider and its path from the root.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitBorder {
    /// Divider coordinate on the split axis.
    pub pos: u16,
    /// Split axis.
    pub direction: SplitDirection,
    /// First-child fraction.
    pub ratio: f32,
    /// Total node area.
    pub area: Rect,
    /// False selects the first child, true the second.
    pub path: Vec<bool>,
}
/// Cardinal direction for navigation and resizing.
#[derive(Debug, Clone, Copy)]
pub enum NavDirection {
    /// Toward the left edge.
    Left,
    /// Toward the right edge.
    Right,
    /// Toward the top edge.
    Up,
    /// Toward the bottom edge.
    Down,
}

impl TileLayout {
    /// Reconstruct a saved tree without focus history.
    #[must_use]
    pub fn from_saved(root: Node, focus: PaneId) -> Self {
        Self {
            root,
            focus,
            prev_focus: None,
        }
    }
    /// Return the selected pane.
    #[must_use]
    pub fn focused(&self) -> PaneId {
        self.focus
    }
    /// Count tree leaves.
    #[must_use]
    pub fn pane_count(&self) -> usize {
        count_panes(&self.root)
    }
    /// Collect pane rectangles in tree order.
    #[must_use]
    pub fn panes(&self, area: Rect) -> Vec<PaneInfo> {
        let mut result = Vec::new();
        collect_panes(&self.root, area, self.focus, &mut result);
        result
    }
    /// Collect pane identities in tree order.
    #[must_use]
    pub fn pane_ids(&self) -> Vec<PaneId> {
        let mut ids = Vec::new();
        collect_ids(&self.root, &mut ids);
        ids
    }
    /// Borrow the split tree for persistence.
    #[must_use]
    pub fn root(&self) -> &Node {
        &self.root
    }
    /// Exchange leaf identities without changing geometry or focus.
    pub fn swap_panes(&mut self, first: PaneId, second: PaneId) -> bool {
        if first == second {
            return false;
        }
        let ids = self.pane_ids();
        if !ids.contains(&first) || !ids.contains(&second) {
            return false;
        }
        swap_pane_ids(&mut self.root, first, second);
        true
    }

    /// Construct a single-pane layout.
    #[must_use]
    pub fn new(root_id: PaneId) -> Self {
        Self {
            root: Node::Pane(root_id),
            focus: root_id,
            prev_focus: None,
        }
    }
    /// Record a real focus move.
    fn set_focus(&mut self, id: PaneId) {
        if id != self.focus {
            self.prev_focus = Some(self.focus);
            self.focus = id;
        }
    }
    /// Insert an existing identity beside a live target.
    pub fn insert_pane_near(&mut self, moved: PaneId, request: &SplitRequest) -> bool {
        let ids = self.pane_ids();
        if request.target == moved || !ids.contains(&request.target) || ids.contains(&moved) {
            return false;
        }
        let old = std::mem::replace(&mut self.root, Node::Pane(PaneId::from_raw(0)));
        self.root = split_at(
            old,
            request.target,
            &request.direction,
            moved,
            valid_split_ratio(request.ratio.unwrap_or(0.5)),
        );
        if request.focus_new_pane {
            self.set_focus(moved);
        }
        true
    }

    /// Insert an allocated identity without moving focus.
    pub fn split_pane(&mut self, new_id: PaneId, request: SplitRequest) -> Option<PaneId> {
        self.insert_pane_near(
            new_id,
            &SplitRequest {
                focus_new_pane: false,
                ..request
            },
        )
        .then_some(new_id)
    }

    /// Select a live pane, ignoring missing identities.
    pub fn focus_pane(&mut self, id: PaneId) {
        if self.pane_ids().contains(&id) {
            self.set_focus(id);
        }
    }

    /// Collect divider geometry and root-relative paths.
    #[must_use]
    pub fn splits(&self, area: Rect) -> Vec<SplitBorder> {
        let mut result = Vec::new();
        collect_splits(&self.root, area, vec![], &mut result);
        result
    }
    /// Clamp and set the ratio at a split path.
    pub fn set_ratio_at(&mut self, path: &[bool], ratio: f32) -> bool {
        set_ratio_at(&mut self.root, path, ratio.clamp(0.1, 0.9))
    }
    /// Adjust the nearest eligible divider, using the opposite edge as fallback.
    pub fn resize_focused(&mut self, nav: NavDirection, delta: f32, area: Rect) {
        let panes = self.panes(area);
        let Some(focused) = panes.iter().find(|p| p.is_focused) else {
            return;
        };
        let focused_rect = focused.rect;
        let splits = self.splits(area);

        let target_dir = match nav {
            NavDirection::Left | NavDirection::Right => SplitDirection::Right,
            NavDirection::Up | NavDirection::Down => SplitDirection::Down,
        };
        let grows = matches!(nav, NavDirection::Right | NavDirection::Down);

        let best = nearest_resize_split(&splits, &target_dir, focused_rect, nav).or_else(|| {
            nearest_resize_split(&splits, &target_dir, focused_rect, opposite_direction(nav))
        });

        if let Some(split) = best {
            let path = &split.path;
            let current_ratio = get_ratio_at(&self.root, path).unwrap_or(0.5);
            let adj = if grows { delta } else { -delta };
            self.set_ratio_at(path, current_ratio + adj);
        }
    }
    /// Resize a live pane while preserving selection and its return history.
    pub fn resize_pane(
        &mut self,
        pane_id: PaneId,
        nav: NavDirection,
        delta: f32,
        area: Rect,
    ) -> bool {
        if !self.pane_ids().contains(&pane_id) {
            return false;
        }
        let before = split_ratios(&self.root);
        let previous_focus = self.focus;
        self.focus = pane_id;
        self.resize_focused(nav, delta, area);
        self.focus = previous_focus;
        split_ratios(&self.root) != before
    }

    /// Close the selection, consuming its live return target or using tree order.
    pub fn close_focused(&mut self) -> bool {
        if self.pane_count() <= 1 {
            return false;
        }
        let target = self.focus;
        let ids = self.pane_ids();
        let Some(pos) = ids.iter().position(|id| *id == target) else {
            return false;
        };
        let ordered = if pos + 1 < ids.len() {
            ids[pos + 1]
        } else {
            ids[pos - 1]
        };
        let new_focus = match self.prev_focus {
            Some(prev) if prev != target && ids.contains(&prev) => prev,
            _ => ordered,
        };
        let placeholder = PaneId::from_raw(0);
        let old = std::mem::replace(&mut self.root, Node::Pane(placeholder));
        if let Some(new_root) = remove_pane(old, target) {
            self.root = new_root;
            self.focus = new_focus;
            self.prev_focus = None;
            true
        } else {
            false
        }
    }
    /// Close a live leaf, leaving background focus history intact.
    pub fn close_pane(&mut self, id: PaneId) -> bool {
        if self.focus == id {
            return self.close_focused();
        }
        if self.pane_count() <= 1 || !self.pane_ids().contains(&id) {
            return false;
        }
        let placeholder = PaneId::from_raw(0);
        let old = std::mem::replace(&mut self.root, Node::Pane(placeholder));
        let Some(new_root) = remove_pane(old, id) else {
            return false;
        };
        self.root = new_root;
        if self.prev_focus == Some(id) {
            self.prev_focus = None;
        }
        true
    }
}
