//! Messages beyond the model conversation.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

pub use crate::bindings::maestro::extension::models::{
    BashExecutionMessage, BranchSummaryMessage, CompactionSummaryMessage, CustomMessage,
    CustomMessageInput,
};
