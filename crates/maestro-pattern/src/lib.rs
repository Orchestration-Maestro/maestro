//! File-pattern matching and directory-scoped ignore decisions.

mod glob;
/// Ordered ignore rules with literal directory scopes.
pub mod ignore;
pub use glob::{
    GlobMatcher, GlobOptions, MmRegExp, ParseReturnFiltered, PatternError, Platform, matches_glob,
    matches_glob_with_nocase,
};

use maestro_text::is_whitespace_scalar as whitespace;
