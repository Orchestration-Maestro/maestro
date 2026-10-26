//! Application-owned conversation messages.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
pub use maestro_request::messages::{
    BashExecutionMessage, BranchSummaryMessage, CompactionSummaryMessage,
};
/// An extension-injected conversation message with opaque details text.
pub type CustomMessage = maestro_request::messages::CustomMessage<crate::JsonText>;
