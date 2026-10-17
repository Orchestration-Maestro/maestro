//! Owned Markdown parsing with a separate mapped extended-link pass.
mod autolinks;
mod parse;
mod references;
mod source;
mod types;

pub use autolinks::autolinks;
pub use parse::{parse, parse_with_references};
pub use pulldown_cmark::{Alignment, LinkType};
pub use types::*;
