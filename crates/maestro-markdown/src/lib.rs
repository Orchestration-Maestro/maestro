//! Owned Markdown parsing with a separate mapped extended-link pass.
mod autolinks;
mod helpers;
mod inline;
mod lexer;
mod parse;
mod rules;
mod source;
mod tokenizer;
mod types;

pub use autolinks::autolinks;
pub use inline::{InlineAtom, InlineContext, InlineRun, InlineScope};
pub use parse::{
    BlockTokenizer, InvalidBlockToken, parse, parse_with_options, parse_with_tokenizer,
};
pub use types::*;

#[cfg(test)]
mod tests;
