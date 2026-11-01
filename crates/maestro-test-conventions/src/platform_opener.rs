//! Platform viewer executables belong to the application opener.

use std::collections::BTreeSet;
use std::ops::Range;

use proc_macro2::{TokenStream, TokenTree};

use crate::source::{Member, test_scope, test_spans};

/// Application source that owns native viewer selection.
const OWNER: &str = "src/utils/platform_opener.rs";

/// Reject viewer executable literals and shell-start selection outside their owner.
///
/// The whole-file token walk includes fragments, macro arguments, closures and methods.
/// Editor and clipboard commands have different operands and remain caller-owned.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut found = Vec::new();
    for member in members {
        for source in &member.sources {
            if test_scope(member, source)
                || (member.name == "maestro-app" && source.path == member.directory.join(OWNER))
            {
                continue;
            }
            let Ok(tokens) = source.contents.parse::<TokenStream>() else {
                continue;
            };
            let mut literals = Vec::new();
            strings(
                tokens,
                &BTreeSet::new(),
                &test_spans(source.syntax.as_ref()),
                &mut literals,
            );
            let shell_start = ["cmd", "/c", "start"]
                .iter()
                .all(|expected| literals.iter().any(|(text, _)| text == expected));
            found.extend(
                literals
                    .into_iter()
                    .filter(|(text, _)| {
                        matches!(text.as_str(), "open" | "xdg-open" | "explorer.exe")
                            || (shell_start && text == "cmd")
                    })
                    .map(|(_, line)| {
                        format!(
                            "{}:{line}: platform viewer outside utils::platform_opener",
                            source.path.display()
                        )
                    }),
            );
        }
    }
    if found.is_empty() {
        Ok(())
    } else {
        Err(found.join("\n"))
    }
}

/// Collect production literals, tracking local `open` bindings through nested groups.
fn strings(
    tokens: TokenStream,
    inherited: &BTreeSet<String>,
    testing: &[Range<(usize, usize)>],
    found: &mut Vec<(String, usize)>,
) {
    let tokens: Vec<_> = tokens
        .into_iter()
        .filter(|token| {
            let start = token.span().start();
            !testing
                .iter()
                .any(|span| span.contains(&(start.line, start.column)))
        })
        .collect();
    let mut bindings = inherited.clone();
    let mut command = false;
    for (index, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Ident(name) => {
                if name == "let" {
                    local_binding(&tokens[index + 1..], &mut bindings);
                }
                command = name == "Command" || name == "command" || (command && name == "new");
            }
            TokenTree::Punct(punct) => command &= punct.as_char() == ':',
            TokenTree::Literal(_) => {
                if let Some(text) = literal_text(token)
                    && text != "open"
                {
                    found.push((text, token.span().start().line));
                }
                command = false;
            }
            TokenTree::Group(group) => {
                if command && let Some(line) = open_operand(group.stream(), &bindings) {
                    found.push(("open".into(), line));
                }
                strings(group.stream(), &bindings, testing, found);
                command = false;
            }
        }
    }
}

/// Decode a string token without treating other literal kinds as executable names.
fn literal_text(token: &TokenTree) -> Option<String> {
    let TokenTree::Literal(literal) = token else {
        return None;
    };
    syn::parse_str::<syn::LitStr>(&literal.to_string())
        .ok()
        .map(|text| text.value())
}

/// Track a simple local literal initializer, replacing a shadowed binding in this group.
fn local_binding(tokens: &[TokenTree], bindings: &mut BTreeSet<String>) {
    let mut tokens = tokens.iter();
    let name = tokens.next();
    let name = match name {
        Some(TokenTree::Ident(name)) if name == "mut" => tokens.next(),
        name => name,
    };
    let Some(TokenTree::Ident(name)) = name else {
        return;
    };
    let name = name.to_string();
    bindings.remove(&name);
    let initializer = tokens
        .take_while(|token| !matches!(token, TokenTree::Punct(punct) if punct.as_char() == ';'))
        .skip_while(|token| !matches!(token, TokenTree::Punct(punct) if punct.as_char() == '='))
        .nth(1);
    if initializer.and_then(literal_text).as_deref() == Some("open") {
        bindings.insert(name);
    }
}

/// Recognize a literal or locally bound macOS executable passed to command construction.
fn open_operand(tokens: TokenStream, bindings: &BTreeSet<String>) -> Option<usize> {
    let operand = tokens.into_iter().next()?;
    let bound = matches!(&operand, TokenTree::Ident(name) if bindings.contains(&name.to_string()));
    (bound || literal_text(&operand).as_deref() == Some("open"))
        .then(|| operand.span().start().line)
}
