//! Test-owned temporary directory checks.

use crate::graph::policy;
use crate::source::Member;

/// Spelling that names the platform temporary directory.
const PLATFORM_DIRECTORY: &str = "temp_dir";

/// Whether the member's test code is checked for the platform temporary directory.
///
/// The walk sees every source of a `maestro-test-` member as test code, so the owner alone is
/// outside the check: its `temporary_root` accessor is the one place that reads the platform
/// directory.
pub(crate) fn checked(member: &Member) -> bool {
    member.name != policy::TEST_DIRECTORY
}

/// Reject a platform temporary directory that test code names itself.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = members
        .iter()
        .filter(|member| checked(member))
        .flat_map(member_diagnostics)
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        return Ok(());
    }
    diagnostics.sort();
    Err(diagnostics
        .into_iter()
        .map(|(file, line)| {
            format!(
                "{file}:{line}: test-owned code names the platform temporary directory; use maestro_test_directory::Scratch or maestro_test_directory::temporary_root, and add a new crate to TEST_DIRECTORY_CALLERS"
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Report the located platform-directory references of one member.
fn member_diagnostics(member: &Member) -> Vec<(String, usize)> {
    member
        .sources
        .iter()
        .flat_map(|source| {
            source
                .walk
                .iter()
                .flat_map(|walk| &walk.temp_directories)
                .map(|line| (source.path.display().to_string(), *line))
        })
        .collect()
}

/// Platform-directory references of one file, gathered by the shared syntax walk.
///
/// The walk passes identifiers and macros of test-owned code and of `use` imports; it skips
/// method names and a declared function's own name, its builder API rather than a
/// platform-directory spelling: the standard function is always a call through a path. It
/// also skips a method call's attributes, which never name a call.
#[derive(Default)]
pub(crate) struct References {
    /// Byte ranges of the declarations whose own name never names the platform function.
    declared: Vec<std::ops::Range<usize>>,
    /// Located reference lines.
    lines: Vec<usize>,
}

impl References {
    /// Record the byte range of one declaration's own name so `ident` skips it.
    pub(crate) fn skip(&mut self, ident: &proc_macro2::Ident) {
        self.declared.push(ident.span().byte_range());
    }

    /// Record one identifier that names the platform temporary directory.
    pub(crate) fn ident(&mut self, ident: &proc_macro2::Ident) {
        let range = ident.span().byte_range();
        if ident != PLATFORM_DIRECTORY
            || self
                .declared
                .iter()
                .any(|skipped| skipped.start <= range.start && range.end <= skipped.end)
        {
            return;
        }
        self.lines.push(ident.span().start().line);
    }

    /// Record the platform-directory identifiers inside a macro's tokens.
    pub(crate) fn macro_tokens(&mut self, invocation: &syn::Macro) {
        token_references(&invocation.tokens, &mut self.lines);
    }

    /// The located lines in order, each once.
    pub(crate) fn finish(mut self) -> Vec<usize> {
        self.lines.sort_unstable();
        self.lines.dedup();
        self.lines
    }
}

/// Record platform-directory identifiers anywhere inside macro tokens, nesting included.
fn token_references(tokens: &proc_macro2::TokenStream, lines: &mut Vec<usize>) {
    for tree in tokens.clone() {
        match tree {
            proc_macro2::TokenTree::Ident(ident) if ident == PLATFORM_DIRECTORY => {
                lines.push(ident.span().start().line);
            }
            proc_macro2::TokenTree::Group(group) => token_references(&group.stream(), lines),
            _ => {}
        }
    }
}
