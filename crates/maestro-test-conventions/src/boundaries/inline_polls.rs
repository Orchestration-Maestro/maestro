//! Single-owner rule for work polled once before a task facility takes it.
use proc_macro2::{TokenStream, TokenTree};

use crate::source::{Member, test_scope};

/// The crate whose production code starts work only through the owner.
const MEMBER: &str = "maestro-app";

/// The file that owns the inline first poll.
const OWNER: &str = "src/utils/task.rs";

/// Reject a no-op waker in production code outside the owner of the inline first poll.
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
            noop_wakers(tokens, &mut lines);
            found.extend(lines.into_iter().map(|line| {
                format!(
                    "{}:{line}: inline poll outside utils::task::start_now",
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

/// Records the line of each `Waker::noop` path in `tokens`, macro arguments included.
fn noop_wakers(tokens: TokenStream, lines: &mut Vec<usize>) {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    for (index, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Ident(name) if *name == "noop" && after_waker(&tokens[..index]) => {
                lines.push(name.span().start().line);
            }
            TokenTree::Group(group) => noop_wakers(group.stream(), lines),
            _ => {}
        }
    }
}

/// Whether `previous` ends with the path `Waker::`.
fn after_waker(previous: &[TokenTree]) -> bool {
    matches!(
        previous,
        [.., TokenTree::Ident(waker), TokenTree::Punct(first), TokenTree::Punct(second)]
            if *waker == "Waker" && first.as_char() == ':' && second.as_char() == ':'
    )
}
