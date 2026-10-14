//! Navigable views over a borrowed session tree.

mod entry_text;
mod navigation;
mod render;
mod structure;
mod tool_call;
mod tree_list;

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
