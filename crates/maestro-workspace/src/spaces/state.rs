//! Workspace topology records.
use crate::{tab::Tab, tiling::PaneId};
use std::{collections::HashMap, path::PathBuf};
/// A named collection of tabs with stable public numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    /// Stable workspace handle.
    pub id: String,
    /// Explicit workspace label.
    pub custom_name: Option<String>,
    /// Supplied fallback identity directory.
    pub identity_cwd: PathBuf,
    /// Directory to which the admitted automatic label belongs.
    pub cached_identity_cwd: PathBuf,
    /// Automatic label admitted outside display operations.
    pub cached_auto_label: String,
    /// Cache key for the admitted repository identity.
    pub cached_git_status_key: PathBuf,
    /// Admitted branch name.
    pub cached_git_branch: Option<String>,
    /// Admitted upstream counts.
    pub cached_git_ahead_behind: Option<(usize, usize)>,
    /// Admitted repository grouping.
    pub cached_git_space: Option<super::GitSpaceMetadata>,
    /// Explicit managed-worktree provenance.
    pub worktree_space: Option<super::WorktreeSpaceMembership>,
    /// Current metadata values and explicit expiry state.
    pub metadata_tokens: crate::metadata_tokens::MetadataTokens,
    /// Last accepted sequence per reporting source.
    pub metadata_token_sequences: HashMap<String, u64>,
    /// Public numbers, including detached panes awaiting explicit unregister.
    pub public_pane_numbers: HashMap<PaneId, usize>,
    /// Next pane number; removals do not decrease it.
    pub next_public_pane_number: usize,
    /// Next tab number; failed creation also reserves its number.
    pub next_public_tab_number: usize,
    /// Tabs in display order.
    pub tabs: Vec<Tab>,
    /// Selected tab index, zero when the transfer source is empty.
    pub active_tab: usize,
}
impl Workspace {
    /// Construct the initial topology from a successfully spawned tab.
    pub(super) fn new(id: String, identity_cwd: PathBuf, label: String, tab: Tab) -> Self {
        let root = tab.root_pane;
        Self {
            cached_git_status_key: identity_cwd.clone(),
            cached_git_branch: None,
            cached_git_ahead_behind: None,
            cached_git_space: None,
            worktree_space: None,
            metadata_tokens: crate::metadata_tokens::MetadataTokens::default(),
            metadata_token_sequences: HashMap::new(),
            id,
            custom_name: None,
            cached_identity_cwd: identity_cwd.clone(),
            identity_cwd,
            cached_auto_label: label,
            public_pane_numbers: HashMap::from([(root, 1)]),
            next_public_pane_number: 2,
            next_public_tab_number: 2,
            tabs: vec![tab],
            active_tab: 0,
        }
    }
    /// Commit a public pane number after successful insertion.
    pub(super) fn register_new_pane_with_number(&mut self, pane: PaneId, number: usize) {
        self.public_pane_numbers.insert(pane, number);
        self.next_public_pane_number = self.next_public_pane_number.max(number + 1);
    }
}
/// Result of taking a pane, including whether its source became empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakenPane {
    /// Recoverable transferred record.
    pub moved: crate::tab::MovedPane,
    /// Index of a source tab removed during transfer.
    pub removed_tab_idx: Option<usize>,
    /// Whether the caller should remove or refill the source workspace.
    pub workspace_empty: bool,
}
