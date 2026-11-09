//! Single-owner rule for start-anchored recognizer sources.
use proc_macro2::{TokenStream, TokenTree};

use crate::source::{Member, test_scope};

/// The crate whose recognizer sources compile through the start-anchored owner.
const MEMBER: &str = "maestro-markdown";

/// The file that owns the start-anchored matcher.
const OWNER: &str = "src/rules/anchored.rs";

/// Source openers that start with `^` yet stay on a plain `Regex`, each with the reason the
/// owner does not apply. An entry is the literal's first characters.
const PLAIN_REGEX: &[(&str, &str)] = &[
    (
        "^([`~]+|[^`~])(?:",
        "inline text spelling compiled by the owner",
    ),
    (
        "^(`+|[^`])(?:",
        "inline text spelling compiled by the owner",
    ),
    (
        "^( {0,3}> ?(",
        "block quote source consumed as a plain regex",
    ),
    (
        "^<!--(?:-?>",
        "inline tag source compiled by the anchored tag rule",
    ),
    (
        "^[^_*]*?__[^_*]*?",
        "right-delimiter runs scanned with find_iter",
    ),
    (
        "^[^_*]*?\\*\\*[^_*]*?",
        "right-delimiter runs scanned with find_iter",
    ),
    (
        "^((?![*_])[\\s\\p{P}\\p{S}])",
        "single-scalar punctuation test",
    ),
    ("^ {{0,{prefix}}}", "item stops matched on one line"),
    (
        "^ {0,3}(?:<(script|pre|style|textarea)",
        "html opener matched on one line",
    ),
    (
        "^ {0,3}</(?!script|pre|style|textarea)",
        "html closing matched on one line",
    ),
];

/// Reject a recognizer source opening with `^` outside the owner's constructor and the allow-list.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut found = Vec::new();
    for member in members.iter().filter(|member| member.name == MEMBER) {
        for source in &member.sources {
            let production = source.path.starts_with(member.directory.join("src"));
            if !production
                || test_scope(member, source)
                || source.path == member.directory.join(OWNER)
            {
                continue;
            }
            let Ok(tokens) = source.contents.parse::<TokenStream>() else {
                continue;
            };
            let mut lines = Vec::new();
            bypasses(tokens, &mut lines);
            found.extend(lines.into_iter().map(|line| {
                format!(
                    "{}:{line}: start-anchored source outside Anchored::new",
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

/// Records the line of each string literal in `tokens` that opens with `^` and is neither an
/// argument of `Anchored::new` nor allow-listed, macro arguments included.
fn bypasses(tokens: TokenStream, lines: &mut Vec<usize>) {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    for (index, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Group(_) if after_owner(&tokens[..index]) => {}
            TokenTree::Group(group) => bypasses(group.stream(), lines),
            TokenTree::Literal(literal) if opens_with_caret(literal) => {
                lines.push(literal.span().start().line);
            }
            _ => {}
        }
    }
}

/// Whether `previous` ends with the path `Anchored::new`, so the next group holds its arguments.
fn after_owner(previous: &[TokenTree]) -> bool {
    matches!(
        previous,
        [
            ..,
            TokenTree::Ident(owner),
            TokenTree::Punct(first),
            TokenTree::Punct(second),
            TokenTree::Ident(constructor)
        ] if *owner == "Anchored"
            && *constructor == "new"
            && first.as_char() == ':'
            && second.as_char() == ':'
    )
}

/// Whether `literal` is a string whose value starts with `^` and is not allow-listed.
fn opens_with_caret(literal: &proc_macro2::Literal) -> bool {
    let Ok(string) = syn::parse2::<syn::LitStr>(TokenTree::Literal(literal.clone()).into()) else {
        return false;
    };
    let value = string.value();
    value.starts_with('^')
        && !PLAIN_REGEX
            .iter()
            .any(|(opener, _)| value.starts_with(opener))
}
