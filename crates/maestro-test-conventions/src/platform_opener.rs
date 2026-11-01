//! Platform viewer executables belong to the application opener.

use proc_macro2::{TokenStream, TokenTree};

use crate::source::{Member, test_scope};

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
            strings(tokens, &mut literals);
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

/// Collect decoded literals through every group, recognizing `open` only as a command operand.
fn strings(tokens: TokenStream, found: &mut Vec<(String, usize)>) {
    let mut command = false;
    for token in tokens {
        match token {
            TokenTree::Ident(name) => {
                command = name == "Command" || name == "command" || (command && name == "new");
            }
            TokenTree::Punct(punct) => command &= punct.as_char() == ':',
            TokenTree::Literal(literal) => {
                if let Ok(text) = syn::parse_str::<syn::LitStr>(&literal.to_string())
                    && text.value() != "open"
                {
                    found.push((text.value(), literal.span().start().line));
                }
                command = false;
            }
            TokenTree::Group(group) => {
                if command
                    && let Some(TokenTree::Literal(literal)) = group.stream().into_iter().next()
                    && let Ok(text) = syn::parse_str::<syn::LitStr>(&literal.to_string())
                    && text.value() == "open"
                {
                    found.push((text.value(), literal.span().start().line));
                }
                strings(group.stream(), found);
                command = false;
            }
        }
    }
}
