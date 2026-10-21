//! Owned Markdown parsing with cursor-time extended links.
mod autolinks;
mod helpers;
mod inline;
mod lexer;
mod parse;
mod rules;
mod source;
mod tokenizer;
mod types;

pub use inline::{InlineAtom, InlineContext, InlineRun, InlineScope, InlineToken, InlineTokenizer};
pub use parse::{
    BlockTokenizer, InvalidBlockToken, parse, parse_with_options, parse_with_tokenizer,
};
pub use types::*;

#[cfg(test)]
mod tests;
