//! Tool argument and observation records.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod bash;
pub use bash::*;
mod read;
pub use read::*;
mod edit;
pub use edit::*;
mod write;
pub use write::*;
mod grep;
pub use grep::*;
mod find;
pub use find::*;
mod ls;
pub use ls::*;
pub mod truncate;
pub use truncate::*;

/// Native authored-path operations.
pub mod path_utils;
