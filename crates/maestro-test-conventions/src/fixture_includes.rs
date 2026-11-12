//! Large fixtures are read when the test runs, not compiled into its executable.

use std::collections::BTreeSet;

use proc_macro2::TokenTree;

use crate::source::{Member, Source};
use crate::test_input_routes::string_value;

/// The largest fixture, in bytes, that test code may compile into its executable.
const LIMIT: u64 = 1_048_576;

/// The includes that copy a file's contents into the compiled test executable.
const NAMES: [&str; 2] = ["include_str", "include_bytes"];

/// Reject test code that compiles a fixture larger than the limit into its executable.
///
/// A large fixture copied into the binary lengthens every link and rebuilds its test unit
/// whenever the fixture changes. Read it when the test runs instead, by joining the
/// manifest directory and the fixture path.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let diagnostics: BTreeSet<String> = members
        .iter()
        .flat_map(|member| member.sources.iter())
        .flat_map(sites)
        .collect();
    if diagnostics.is_empty() {
        return Ok(());
    }
    Err(diagnostics.into_iter().collect::<Vec<_>>().join("\n"))
}

/// The over-limit include diagnostics of one source.
fn sites(source: &Source) -> impl Iterator<Item = String> {
    source
        .walk
        .iter()
        .flat_map(|walk| &walk.includes)
        .filter_map(move |(line, literal)| {
            let directory = source.path.parent()?;
            let size = std::fs::metadata(directory.join(literal)).ok()?.len();
            (size > LIMIT).then(|| {
                format!(
                    "{}:{line}: tests compile {literal} ({size} bytes) into their executable, over the {LIMIT}-byte limit; read it when the test runs instead",
                    source.path.display()
                )
            })
        })
}

/// Compile-time includes found in one file's test code.
#[derive(Default)]
pub(crate) struct Includes {
    /// The line and spelled path of each include target, in source order.
    pub(crate) targets: Vec<(usize, String)>,
}

impl Includes {
    /// Record the target of an include in test code.
    ///
    /// A target the compiler resolves or reproduces is the include's own business; this
    /// scan reads the spelled path alone.
    pub(crate) fn macro_tokens(&mut self, invocation: &syn::Macro) {
        let compiled = NAMES.iter().any(|name| invocation.path.is_ident(*name));
        if let Some(TokenTree::Literal(literal)) = invocation.tokens.clone().into_iter().next()
            && compiled
            && let Some(value) = string_value(&literal)
        {
            self.targets.push((literal.span().start().line, value));
        }
    }
}
