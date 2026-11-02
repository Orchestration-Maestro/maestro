//! Single-owner rule for the shared helpers of the shipped extension examples.

use std::str::FromStr;

use proc_macro2::{TokenStream, TokenTree};

use crate::source::{Member, Source};

/// Crate whose example tests use the shared helpers.
const GUEST: &str = "maestro-extensions-wasm";
/// Example-test directory inside the guest crate.
const TESTS: &str = "src/tests";
/// File name suffix of one example's tests.
const SUFFIX: &str = "_example.rs";
/// Operation that releases every registration and asserts the expected names.
const RELEASED: &str = "example_support::release_all";

/// Reject example tests that write the shared registration release walk inline.
///
/// The registration release lives once in the example support module; a
/// per-example test that repeats it is reported with its file and line.
/// Macro arguments, closures and methods are inspected too.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = Vec::new();
    for member in members.iter().filter(|member| member.name == GUEST) {
        let sources = member
            .sources
            .iter()
            .filter(|source| example_test(member, source));
        for source in sources {
            let Ok(tokens) = TokenStream::from_str(&source.contents) else {
                continue;
            };
            let file = source.path.display().to_string();
            let mut found = Vec::new();
            inline_copies(tokens, &mut found);
            diagnostics.extend(
                found
                    .into_iter()
                    .map(|(line, message)| (file.clone(), line, message)),
            );
        }
    }
    if diagnostics.is_empty() {
        return Ok(());
    }
    diagnostics.sort();
    diagnostics.dedup();
    Err(diagnostics
        .into_iter()
        .map(|(file, line, message)| format!("{file}:{line}: {message}"))
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Whether `source` is one example's test file in the guest crate.
fn example_test(member: &Member, source: &Source) -> bool {
    source
        .path
        .strip_prefix(&member.directory)
        .is_ok_and(|relative| {
            relative.starts_with(TESTS)
                && relative
                    .file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .is_some_and(|name| name.ends_with(SUFFIX))
        })
}

/// Collect the inline copies in one token stream, descending into every group.
fn inline_copies(tokens: TokenStream, found: &mut Vec<(usize, String)>) {
    for token in tokens {
        if let TokenTree::Ident(ident) = &token
            && ident == "next_to_release"
        {
            found.push((
                ident.span().start().line,
                format!("inline registration release walk; use {RELEASED}"),
            ));
        }
        if let TokenTree::Group(group) = token {
            inline_copies(group.stream(), found);
        }
    }
}
