//! Single-owner rule for terminal detection on the standard streams.
use proc_macro2::{TokenStream, TokenTree};

use std::ops::Range;

use crate::source::{Member, covered, test_scope};

/// The files whose production code asks a stream whether it is a terminal, as
/// (crate, path inside the crate): the colour decision, print output cleaning, and input
/// routing with the component stdout route.
///
/// Colour follows `maestro_theme::supports_color` alone, so no other file may decide it from
/// the stream.
const OWNERS: &[(&str, &str)] = &[
    ("maestro-theme", "src/palette/loading.rs"),
    ("maestro-cli", "src/print/print_mode/mod.rs"),
    ("maestro", "src/host.rs"),
];

/// Reject a `.is_terminal()` call in production code outside the owners of terminal detection.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut found = Vec::new();
    for member in members {
        for source in &member.sources {
            let owned = OWNERS.iter().any(|(name, path)| {
                member.name == *name && source.path == member.directory.join(path)
            });
            if owned || test_scope(member, source) {
                continue;
            }
            let Ok(tokens) = source.contents.parse::<TokenStream>() else {
                continue;
            };
            let mut lines = Vec::new();
            calls(tokens, source.test_spans(), &mut lines);
            found.extend(lines.into_iter().map(|line| {
                format!(
                    "{}:{line}: terminal detection outside its owners; colour comes from \
                     maestro_theme::supports_color",
                    source.path.display()
                )
            }));
        }
    }
    if found.is_empty() {
        Ok(())
    } else {
        Err(found.join("\n"))
    }
}

/// Records the line of each `.is_terminal()` call in `tokens` outside the test-gated `testing`
/// spans, macro arguments included.
fn calls(tokens: TokenStream, testing: &[Range<(usize, usize)>], lines: &mut Vec<usize>) {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    for (index, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Ident(name)
                if *name == "is_terminal"
                    && method_call(&tokens[..index], tokens.get(index + 1)) =>
            {
                let start = name.span().start();
                if !covered(testing, (start.line, start.column)) {
                    lines.push(start.line);
                }
            }
            TokenTree::Group(group) => calls(group.stream(), testing, lines),
            _ => {}
        }
    }
}

/// Whether the name is called as a method: a dot before it and an argument list after it.
fn method_call(previous: &[TokenTree], next: Option<&TokenTree>) -> bool {
    matches!(previous.last(), Some(TokenTree::Punct(dot)) if dot.as_char() == '.')
        && matches!(next, Some(TokenTree::Group(group)) if group.delimiter() == proc_macro2::Delimiter::Parenthesis)
}
