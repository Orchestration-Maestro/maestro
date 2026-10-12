//! File-pattern matching and directory-scoped ignore decisions.

/// Ordered ignore rules with literal directory scopes.
pub mod ignore;
mod minimatch;
pub use minimatch::{
    Minimatch, MinimatchOptions, MmRegExp, ParseReturnFiltered, PatternError, Platform, minimatch,
    minimatch_with_nocase,
};

use maestro_text::is_whitespace_scalar as whitespace;
