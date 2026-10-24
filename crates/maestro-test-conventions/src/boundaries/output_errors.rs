//! Single-owner rule for native output-failure registration.
use proc_macro2::{Delimiter, TokenStream, TokenTree};

use crate::source::{Member, test_scope};

/// The crate whose terminals and host report native output failures.
const MEMBER: &str = "maestro-tui-crossterm";

/// The file that owns the registered callbacks.
const OWNER: &str = "src/output_errors.rs";

/// The method each native terminal and host exposes.
const METHOD: &str = "on_output_error";

/// The owner's operation whose result the method must return.
const DELEGATION: &str = "register";

/// Reject a native output-error registration that does not return the owner's removal, and
/// callback storage outside the owner.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut found = Vec::new();
    for member in members.iter().filter(|member| member.name == MEMBER) {
        for source in &member.sources {
            let production = source.path.starts_with(member.directory.join("src"));
            if !production || test_scope(member, source) {
                continue;
            }
            let Ok(tokens) = source.contents.parse::<TokenStream>() else {
                continue;
            };
            let mut scan = Scan {
                owner: source.path == member.directory.join(OWNER),
                found: Vec::new(),
            };
            scan.walk(tokens);
            let path = source.path.display();
            found.extend(
                scan.found
                    .into_iter()
                    .map(|(line, message)| format!("{path}:{line}: {message}")),
            );
        }
    }
    if found.is_empty() {
        Ok(())
    } else {
        Err(found.join("\n"))
    }
}

/// Findings in one adapter source.
struct Scan {
    /// Whether the source owns the registered callbacks.
    owner: bool,
    /// Each finding's line and message.
    found: Vec<(usize, &'static str)>,
}

impl Scan {
    /// Walks `tokens`, macro arguments included.
    fn walk(&mut self, tokens: TokenStream) {
        let tokens: Vec<TokenTree> = tokens.into_iter().collect();
        let mut index = 0;
        while let Some(token) = tokens.get(index) {
            index += 1;
            let line = token.span().start().line;
            match token {
                TokenTree::Ident(name) if *name == "fn" => {
                    index += self.declaration(&tokens[index..], line);
                }
                TokenTree::Ident(name) if *name == "use" => {
                    index += tokens[index..]
                        .iter()
                        .position(
                            |token| matches!(token, TokenTree::Punct(end) if end.as_char() == ';'),
                        )
                        .map_or(0, |end| end + 1);
                }
                TokenTree::Ident(name) if *name == "OutputErrorHandler" && !self.owner => {
                    self.found
                        .push((line, "output-error callback storage outside output_errors"));
                }
                TokenTree::Group(group) => self.walk(group.stream()),
                _ => {}
            }
        }
    }

    /// Checks the shape of an output-error registration and returns how many tokens it spans,
    /// so its parameters are not read as storage.
    fn declaration(&mut self, following: &[TokenTree], line: usize) -> usize {
        if !matches!(following.first(), Some(TokenTree::Ident(name)) if *name == METHOD) {
            return 0;
        }
        let Some(end) = following
            .iter()
            .position(|token| matches!(token, TokenTree::Group(group) if group.delimiter() == Delimiter::Brace))
        else {
            return 0;
        };
        if !delegates(&following[end]) {
            self.found.push((
                line,
                "output-error registration must return the owner's removal",
            ));
        }
        end + 1
    }
}

/// Whether `body` is the single tail expression `self.<fields>.register(<parameter>)`.
fn delegates(body: &TokenTree) -> bool {
    let TokenTree::Group(body) = body else {
        return false;
    };
    let tokens: Vec<TokenTree> = body.stream().into_iter().collect();
    let [
        TokenTree::Ident(receiver),
        between @ ..,
        TokenTree::Punct(dot),
        TokenTree::Ident(call),
        argument,
    ] = tokens.as_slice()
    else {
        return false;
    };
    receiver == "self"
        && dot.as_char() == '.'
        && call == DELEGATION
        && is_single_ident(argument)
        && fields(between)
}

/// Whether `tokens` is a nonempty run of `.` and field names.
fn fields(tokens: &[TokenTree]) -> bool {
    !tokens.is_empty()
        && matches!(
            tokens,
            [TokenTree::Punct(dot), TokenTree::Ident(_), between @ ..]
                if dot.as_char() == '.' && (between.is_empty() || fields(between))
        )
}

/// Whether `argument` is a parenthesis group holding one identifier.
fn is_single_ident(argument: &TokenTree) -> bool {
    let TokenTree::Group(argument) = argument else {
        return false;
    };
    argument.delimiter() == Delimiter::Parenthesis
        && matches!(
            argument
                .stream()
                .into_iter()
                .collect::<Vec<TokenTree>>()
                .as_slice(),
            [TokenTree::Ident(_)]
        )
}
