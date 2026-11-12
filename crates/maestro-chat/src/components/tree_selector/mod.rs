//! Retained session-tree navigation, literal search and folding.

mod entry_text;
mod navigation;
mod panel;
mod render;
mod tool_call;
mod tree_list;

pub use maestro_session::export_html::template::SessionTreeNode;
pub use panel::{TreeSelectorCallbacks, TreeSelectorComponent, TreeSelectorOptions};
pub use tree_list::TreeList;

/// Entry categories included in the navigation view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FilterMode {
    /// Conversation entries without bookkeeping records.
    #[default]
    Default,
    /// Conversation entries without tool results.
    NoTools,
    /// User messages.
    UserOnly,
    /// Entries with a resolved label.
    LabeledOnly,
    /// Entries of any kind after assistant content eligibility.
    All,
}
