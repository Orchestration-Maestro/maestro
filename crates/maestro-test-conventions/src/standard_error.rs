//! Standard error diagnostics outside tests.
//!
//! The standard print macros panic when the stream behind standard error is closed, so product
//! code writes its diagnostics through the shared fallible writer instead. The scan reads
//! macro names, not types: every invocation of a standard error print macro is rejected,
//! whatever path qualifies it and however deeply other macros' arguments nest it. Development
//! tooling runs in the developer's terminal and keeps the macros.

use crate::source::Member;
use proc_macro2::{TokenStream, TokenTree};

/// The development tooling crate, whose terminal output belongs to the developer.
const TOOLING: &str = "maestro-tooling";

/// Print macros that write to standard error and panic when it is closed.
const MACROS: &[&str] = &["eprint", "eprintln"];

/// Whether the member's production code is checked for standard error print macros.
pub(crate) fn checked(member: &Member) -> bool {
    member.name != TOOLING
}

/// Reject standard error print macros in production code.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = members
        .iter()
        .filter(|member| checked(member))
        .flat_map(|member| {
            member.sources.iter().flat_map(|source| {
                source
                    .walk
                    .iter()
                    .flat_map(|walk| &walk.standard_error)
                    .map(|line| (source.path.display().to_string(), *line))
            })
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        return Ok(());
    }
    diagnostics.sort();
    Err(diagnostics
        .into_iter()
        .map(|(file, line)| {
            format!(
                "{file}:{line}: production code prints to standard error with a print macro, which panics when the stream is closed; call maestro_agent::diagnostics::write_stderr or write_stderr_line"
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Production print macro invocations of one file, gathered by the shared syntax walk.
#[derive(Default)]
pub(crate) struct Prints {
    /// Located lines.
    lines: Vec<usize>,
}

impl Prints {
    /// Record an invocation of a print macro, and any other inside its unparsed arguments.
    pub(crate) fn invocation(&mut self, invocation: &syn::Macro) {
        if let Some(last) = invocation.path.segments.last()
            && MACROS.iter().any(|name| last.ident == name)
        {
            self.lines.push(last.ident.span().start().line);
        }
        token_prints(&invocation.tokens, &mut self.lines);
    }

    /// The located lines in order, each once.
    pub(crate) fn finish(mut self) -> Vec<usize> {
        self.lines.sort_unstable();
        self.lines.dedup();
        self.lines
    }
}

/// Record print macro invocations anywhere inside macro tokens, nesting included.
fn token_prints(tokens: &TokenStream, lines: &mut Vec<usize>) {
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
    for (index, tree) in trees.iter().enumerate() {
        match tree {
            TokenTree::Ident(ident)
                if MACROS.iter().any(|name| ident == name)
                    && matches!(trees.get(index + 1), Some(TokenTree::Punct(bang)) if bang.as_char() == '!') =>
            {
                lines.push(ident.span().start().line);
            }
            TokenTree::Group(group) => token_prints(&group.stream(), lines),
            _ => {}
        }
    }
}
