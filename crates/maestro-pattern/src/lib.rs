//! File-pattern matching and directory-scoped ignore decisions.

/// Ordered ignore rules with literal directory scopes.
pub mod ignore;
mod minimatch;
pub use minimatch::{
    Minimatch, MinimatchOptions, MmRegExp, ParseReturnFiltered, PatternError, Platform, minimatch,
};

/// Recognizes authored whitespace, including BOM but not the next-line control.
fn whitespace(c: char) -> bool {
    c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}')
}
