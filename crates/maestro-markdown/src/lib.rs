//! Owned Markdown parsing with a separate mapped extended-link pass.
mod autolinks;
mod inline;
mod parse;
mod source;
mod types;

pub use autolinks::autolinks;
pub use inline::{InlineAtom, InlineContext, InlineRun, InlineScope};
pub use parse::parse;
pub use types::*;

#[cfg(test)]
mod tests;
