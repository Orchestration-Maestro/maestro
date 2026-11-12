//! File-pattern matching and directory-scoped ignore decisions.

mod glob;
/// Ordered ignore rules with literal directory scopes.
pub mod ignore;
pub use glob::{
    Minimatch, MinimatchOptions, MmRegExp, ParseReturnFiltered, PatternError, Platform, minimatch,
    minimatch_with_nocase,
};

use maestro_text::is_whitespace_scalar as whitespace;
