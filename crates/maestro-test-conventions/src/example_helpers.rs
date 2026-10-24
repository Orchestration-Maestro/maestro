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
/// macro argument, a closure or a method is rejected like a bare call.
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

/// The argument tokens of the call whose callee starts at `index`, past a turbofish.
fn arguments(tokens: &[TokenTree], index: usize) -> Option<TokenStream> {
    let mut depth = 0usize;
    for token in &tokens[index + 1..] {
        match token {
            TokenTree::Group(group) if depth == 0 => {
                return (group.delimiter() == Delimiter::Parenthesis).then(|| group.stream());
            }
            TokenTree::Punct(punct) if punct.as_char() == '<' => depth += 1,
            TokenTree::Punct(punct) if punct.as_char() == '>' && depth > 0 => depth -= 1,
            TokenTree::Punct(punct) if depth == 0 && punct.as_char() == ':' => {}
            TokenTree::Group(_) | TokenTree::Ident(_) | TokenTree::Literal(_) if depth > 0 => {}
            _ => return None,
        }
    }
    None
}

/// Whether these argument tokens take an indexed member as a string.
fn decodes_member(tokens: TokenStream) -> bool {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    for (index, token) in tokens.iter().enumerate() {
        if let TokenTree::Group(group) = token {
            if group.delimiter() == Delimiter::Bracket && reads_text(&tokens, index) {
                return true;
            }
            if decodes_member(group.stream()) {
                return true;
            }
        }
    }
    false
}

/// Whether the group at `index` is an index followed by `.as_str`.
fn reads_text(tokens: &[TokenTree], index: usize) -> bool {
    matches!(tokens.get(index + 1), Some(TokenTree::Punct(punct)) if punct.as_char() == '.')
        && matches!(tokens.get(index + 2), Some(TokenTree::Ident(ident)) if ident == "as_str")
}
