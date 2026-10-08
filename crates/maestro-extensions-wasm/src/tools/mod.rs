//! Input and details records of the built-in tools.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod bash;
mod edit;
mod find;
mod grep;
mod ls;
mod read;
mod truncate;
mod write;

pub use bash::{BashToolDetails, BashToolInput};
pub use edit::{EditToolDetails, EditToolInput, ReplaceEdit};
pub use find::{FindToolDetails, FindToolInput};
pub use grep::{GrepToolDetails, GrepToolInput};
pub use ls::{LsToolDetails, LsToolInput};
pub use read::{ReadToolDetails, ReadToolInput};
pub use truncate::{TruncatedBy, TruncationResult};
pub use write::WriteToolInput;
