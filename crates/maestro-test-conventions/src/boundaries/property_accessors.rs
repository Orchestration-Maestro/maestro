//! Single-owner rule for element property installation.
use proc_macro2::{TokenStream, TokenTree};

use crate::source::{Member, Source, test_scope};

/// Crate whose module owns installing property accessors.
const OWNER: &str = "maestro-web";
/// Relative path of the owning module.
const OWNER_FILE: &str = "src/dom.rs";
/// Last two path segments of a descriptor installation.
const FRAGMENT: [&str; 2] = ["Reflect", "define_property"];

/// Reject a property descriptor installed outside the owning module.
pub(crate) fn check(member: &Member, source: &Source) -> Result<(), String> {
    if owns(member, source) || test_scope(member, source) {
        return Ok(());
    }
    let installed = installed(source);
    if installed.is_empty() {
        return Ok(());
    }
    Err(installed
        .into_iter()
        .map(|line| {
            format!(
                "{}:{line}: only maestro-web::dom::install installs element property accessors; call it instead of building a descriptor",
                source.path.display()
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Whether `source` is the module that owns installation.
fn owns(member: &Member, source: &Source) -> bool {
    member.name == OWNER
        && source
            .path
            .strip_prefix(&member.directory)
            .is_ok_and(|relative| relative == std::path::Path::new(OWNER_FILE))
}

/// Lines of `source` that install a property descriptor, macro arguments included.
fn installed(source: &Source) -> Vec<usize> {
    let Ok(tokens) = source.contents.parse::<TokenStream>() else {
        return Vec::new();
    };
    let mut flat = Vec::new();
    flatten(tokens, &mut flat);
    let mut lines: Vec<usize> = flat
        .windows(FRAGMENT.len() + 2)
        .filter_map(installs)
        .collect();
    lines.sort_unstable();
    lines.dedup();
    lines
}

/// Collect the tokens of `tokens` in source order, including macro and block arguments.
fn flatten(tokens: TokenStream, flat: &mut Vec<TokenTree>) {
    for token in tokens {
        match token {
            TokenTree::Group(group) => flatten(group.stream(), flat),
            other => flat.push(other),
        }
    }
}

/// Line of an installation when one token window is a call of the fragment.
fn installs(window: &[TokenTree]) -> Option<usize> {
    let [
        TokenTree::Ident(first),
        TokenTree::Punct(first_colon),
        TokenTree::Punct(second_colon),
        TokenTree::Ident(last),
    ] = window
    else {
        return None;
    };
    (first == FRAGMENT[0]
        && first_colon.as_char() == ':'
        && second_colon.as_char() == ':'
        && last == FRAGMENT[1])
        .then(|| last.span().start().line)
}

#[cfg(test)]
mod tests;
