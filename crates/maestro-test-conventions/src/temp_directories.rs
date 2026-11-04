//! Test-owned temporary directory checks.

use crate::source::Member;

/// Crates whose test directories have moved onto the disposable-directory owner.
///
/// A later migration batch adds the crates it converts; the batch that finishes the
/// migration removes this scope so the check covers every crate.
const MIGRATED_CRATES: &[&str] = &[
    "maestro-daemon",
    "maestro-session",
    "maestro-watch",
    "maestro-detection",
    "maestro-extensions-wasmtime",
    "maestro-terminal",
    "maestro-models",
    "maestro-app",
    "maestro-packages",
    "maestro-catalog",
    "maestro-process",
    "maestro-tooling",
    "maestro-tui-crossterm",
    "maestro-ipc",
    "maestro-lock",
    "maestro-listing",
    "maestro-realpath",
    "maestro-tui",
    "maestro-workspace",
    "maestro-credentials",
    "maestro-tools",
    "maestro-resources",
    "maestro-chat",
];

/// Spelling that names the platform temporary directory.
const PLATFORM_DIRECTORY: &str = "temp_dir";

/// Whether the member's test code is checked for the platform temporary directory.
pub(crate) fn migrated(member: &Member) -> bool {
    MIGRATED_CRATES.contains(&member.name.as_str())
}

/// Reject a platform temporary directory that migrated-crate test code names itself.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = members
        .iter()
        .filter(|member| migrated(member))
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
                .walk(member)
                .temp_directories
                .iter()
                .map(|line| (source.path.display().to_string(), *line))
        })
        .collect()
}

/// Platform-directory references of one file, gathered by the shared syntax walk.
///
/// The walk passes identifiers and macros of test-owned code and of `use` imports; it skips
/// method names, the receiver's own API rather than a platform-directory spelling: the
/// standard function is always a call through a path. It also skips a method call's
/// attributes, which never name a call.
#[derive(Default)]
pub(crate) struct References {
    /// Located reference lines.
    lines: Vec<usize>,
}

impl References {
    /// Record one identifier that names the platform temporary directory.
    pub(crate) fn ident(&mut self, ident: &proc_macro2::Ident) {
        if ident == PLATFORM_DIRECTORY {
            self.lines.push(ident.span().start().line);
        }
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
