//! Single owners of browser element connection checks and copy-control creation.
use proc_macro2::{Delimiter, TokenStream, TokenTree};

use crate::source::{Member, test_scope};

/// Crate whose built-in elements share these owners.
const MEMBER: &str = "maestro-web";
/// The one element-side file that may ask a node whether it is connected.
const CONNECTION_OWNER: &str = "src/dom.rs";
/// The copy control's own module tree.
const COPY_TREE: &str = "src/components/copy_button/";
/// Files that reach the copy control's page module: the listing owner and the element registration.
const COPY_CALLERS: &[&str] = &[
    "src/components/code_block/dom.rs",
    "src/contributions/loader.rs",
];

/// Reject inline connection checks in element files and copy controls created outside the listing owner.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut found = Vec::new();
    for member in members.iter().filter(|member| member.name == MEMBER) {
        for source in &member.sources {
            let (Ok(relative), Ok(tokens)) = (
                source.path.strip_prefix(&member.directory),
                source.contents.parse::<TokenStream>(),
            ) else {
                continue;
            };
            if test_scope(member, source) {
                continue;
            }
            let relative = relative.to_string_lossy().replace('\\', "/");
            let mut scan = Scan::default();
            scan.walk(tokens);
            let file = source.path.display();
            if let (true, Some(line)) =
                (scan.element && relative != CONNECTION_OWNER, scan.connected)
            {
                found.push(format!(
                    "{file}:{line}: element connection belongs to crate::dom::connected"
                ));
            }
            let copy_caller =
                relative.starts_with(COPY_TREE) || COPY_CALLERS.contains(&relative.as_str());
            if let (false, Some(line)) = (copy_caller, scan.copy) {
                found.push(format!(
                    "{file}:{line}: copy controls are created by code_block::dom::Listing"
                ));
            }
        }
    }
    if found.is_empty() {
        Ok(())
    } else {
        Err(found.join("\n"))
    }
}

/// What one file's whole token stream, macro arguments included, shows.
#[derive(Default)]
struct Scan {
    /// The file names the element lifecycle trait.
    element: bool,
    /// First line asking a node whether it is connected.
    connected: Option<usize>,
    /// First line reaching the copy control's page module or spelling its tag.
    copy: Option<usize>,
}

impl Scan {
    /// Record the fragments in `tokens` and every nested group.
    fn walk(&mut self, tokens: TokenStream) {
        let tokens: Vec<_> = tokens.into_iter().collect();
        for (index, token) in tokens.iter().enumerate() {
            let line = token.span().start().line;
            match token {
                TokenTree::Ident(ident) => {
                    self.ident(&ident.to_string(), &tokens[index + 1..], line);
                }
                TokenTree::Literal(literal) if literal.to_string() == "\"copy-button\"" => {
                    self.copy.get_or_insert(line);
                }
                TokenTree::Group(group) => self.walk(group.stream()),
                _ => {}
            }
        }
    }
    /// Record an identifier at `line` followed by `following`.
    fn ident(&mut self, name: &str, following: &[TokenTree], line: usize) {
        self.element |= name == "CustomElement";
        if name == "is_connected" {
            self.connected.get_or_insert(line);
        }
        if name == "copy_button" && reaches_page_module(following) {
            self.copy.get_or_insert(line);
        }
    }
}

/// Whether the path after `copy_button` renames the module or reaches its `dom` child, directly,
/// through a glob or through a `{..}` import group.
fn reaches_page_module(following: &[TokenTree]) -> bool {
    let child = match following {
        [TokenTree::Ident(rename), ..] => return rename == "as",
        [TokenTree::Punct(first), TokenTree::Punct(second), child, ..]
            if first.as_char() == ':' && second.as_char() == ':' =>
        {
            child
        }
        _ => return false,
    };
    match child {
        TokenTree::Ident(name) => name == "dom",
        TokenTree::Punct(glob) => glob.as_char() == '*',
        TokenTree::Group(group) if group.delimiter() == Delimiter::Brace => {
            group.stream().into_iter().any(|token| match token {
                TokenTree::Ident(name) => name == "dom" || name == "self",
                TokenTree::Punct(glob) => glob.as_char() == '*',
                _ => false,
            })
        }
        _ => false,
    }
}
