//! Single-owner rule for the shared helpers of the shipped extension examples.

use std::str::FromStr;

use proc_macro2::{Delimiter, TokenStream, TokenTree};

use crate::source::{Member, Source};

/// Crate whose example tests use the shared helpers.
const GUEST: &str = "maestro-extensions-wasm";
/// Example-test directory inside the guest crate.
const TESTS: &str = "src/tests";
/// File name suffix of one example's tests.
const SUFFIX: &str = "_example.rs";
/// Operation that decodes a document member.
const DECODED: &str = "example_support::decoded";
/// Operation that releases every registration and asserts the expected names.
const RELEASED: &str = "example_support::release_all";

/// Reject example tests that write a shared example operation inline.
///
/// The shared decoding and registration release live once in the example support
/// module; a per-example test that repeats one of them is reported with its file
/// and line. Each rule matches the operation's token fragment, so a copy inside a
/// macro argument, a closure or a method is rejected like a bare call. Only one call
/// expression at a time is judged: a decode split over statements through an
/// intermediate binding repeats no fragment and is not reported.
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
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    for (index, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Ident(ident) if ident == "next_to_release" => found.push((
                ident.span().start().line,
                format!("inline registration release walk; use {RELEASED}"),
            )),
            TokenTree::Ident(ident)
                if ident == "from_str" && arguments(&tokens, index).is_some_and(decodes_member) =>
            {
                found.push((
                    ident.span().start().line,
                    format!("inline member decode; use {DECODED}"),
                ));
            }
            _ => {}
        }
        if let TokenTree::Group(group) = token {
            inline_copies(group.stream(), found);
        }
    }
}

/// The argument tokens of the call whose callee starts at `index`.
///
/// Everything between the callee and its first parenthesised group is skipped, so a
/// turbofish naming a qualified or nested type still reaches the call's arguments.
fn arguments(tokens: &[TokenTree], index: usize) -> Option<TokenStream> {
    tokens[index + 1..].iter().find_map(|token| match token {
        TokenTree::Group(group) if group.delimiter() == Delimiter::Parenthesis => {
            Some(group.stream())
        }
        _ => None,
    })
}

/// Whether these argument tokens take an indexed member as a string.
fn decodes_member(tokens: TokenStream) -> bool {
    let (mut indexed, mut text) = (false, false);
    marks(tokens, &mut indexed, &mut text);
    indexed && text
}

/// Flag whether the flattened `tokens` hold an index group and an `as_str` ident.
fn marks(tokens: TokenStream, indexed: &mut bool, text: &mut bool) {
    for token in tokens {
        match token {
            TokenTree::Group(group) => {
                *indexed |= group.delimiter() == Delimiter::Bracket;
                marks(group.stream(), indexed, text);
            }
            TokenTree::Ident(ident) => *text |= ident == "as_str",
            _ => {}
        }
    }
}
