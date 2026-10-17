//! Owned Markdown parsing with a separate mapped extended-link pass.
mod autolinks;
mod parse;
mod source;
mod types;

pub use autolinks::autolinks;
pub use parse::parse;
pub use pulldown_cmark::{Alignment, LinkType};
pub use types::*;
