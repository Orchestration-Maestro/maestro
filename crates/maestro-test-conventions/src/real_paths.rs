//! Real-path resolution outside its owner.
//!
//! Production code resolves real paths only through `maestro_realpath::canonicalize`, which
//! returns the ordinary Windows form. The standard and Tokio functions, the path method and
//! any import of the name return the verbatim form there, so every other spelling of the name
//! is rejected. The scan reads names, not types: a method named `canonicalize` is rejected
//! whatever its receiver, and the owner's function is always called by its qualified path.

use crate::source::Member;
use proc_macro2::{TokenStream, TokenTree};

/// The one crate that resolves real paths through the file system.
const OWNER: &str = "maestro-realpath";

/// The owner's crate name as code spells it.
const OWNER_PATH: &str = "maestro_realpath";

/// The name the standard library, Tokio and the owner give real-path resolution.
const NAME: &str = "canonicalize";

/// Whether the member's production code is checked for real-path resolution.
pub(crate) fn checked(member: &Member) -> bool {
    member.name != OWNER
}

/// Reject real-path resolution that production code performs outside the owner.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = members
        .iter()
        .filter(|member| checked(member))
        .flat_map(|member| {
            member.sources.iter().flat_map(|source| {
                source
                    .walk
                    .iter()
                    .flat_map(|walk| &walk.real_paths)
                    .map(|line| (source.path.display().to_string(), *line))
            })
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        return Ok(());
    }
    diagnostics.sort();
    Err(diagnostics
        .into_iter()
        .map(|(file, line)| {
            format!(
                "{file}:{line}: production code resolves a real path outside {OWNER}; call {OWNER_PATH}::{NAME}"
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Production spellings of real-path resolution in one file, gathered by the shared syntax walk.
#[derive(Default)]
pub(crate) struct Resolutions {
    /// Located lines.
    lines: Vec<usize>,
}

impl Resolutions {
    /// Record a method call named after real-path resolution, whatever its receiver.
    pub(crate) fn method(&mut self, method: &proc_macro2::Ident) {
        if method == NAME {
            self.lines.push(method.span().start().line);
        }
    }

    /// Record a path that ends in the name, unless it is the owner's qualified function.
    pub(crate) fn path(&mut self, path: &syn::Path) {
        let Some(last) = path.segments.last().filter(|last| last.ident == NAME) else {
            return;
        };
        let owner = path.segments.len() == 2
            && path
                .segments
                .first()
                .is_some_and(|first| first.ident == OWNER_PATH);
        if !owner {
            self.lines.push(last.ident.span().start().line);
        }
    }

    /// Record every import of the name, whatever it is imported from or renamed to.
    pub(crate) fn use_tree(&mut self, tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(path) => self.use_tree(&path.tree),
            syn::UseTree::Name(name) if name.ident == NAME => {
                self.lines.push(name.ident.span().start().line);
            }
            syn::UseTree::Rename(rename) if rename.ident == NAME => {
                self.lines.push(rename.ident.span().start().line);
            }
            syn::UseTree::Group(group) => group.items.iter().for_each(|item| self.use_tree(item)),
            _ => {}
        }
    }

    /// Record the name inside a macro's tokens unless the owner's crate qualifies it.
    pub(crate) fn macro_tokens(&mut self, invocation: &syn::Macro) {
        token_resolutions(&invocation.tokens, &mut self.lines);
    }

    /// The located lines in order, each once.
    pub(crate) fn finish(mut self) -> Vec<usize> {
        self.lines.sort_unstable();
        self.lines.dedup();
        self.lines
    }
}

/// Record unqualified uses of the name anywhere inside macro tokens, nesting included.
fn token_resolutions(tokens: &TokenStream, lines: &mut Vec<usize>) {
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
    for (index, tree) in trees.iter().enumerate() {
        match tree {
            TokenTree::Ident(ident) if ident == NAME && !owner_qualified(&trees[..index]) => {
                lines.push(ident.span().start().line);
            }
            TokenTree::Group(group) => token_resolutions(&group.stream(), lines),
            _ => {}
        }
    }
}

/// Whether the tokens before a name end with the owner's crate and a path separator.
fn owner_qualified(before: &[TokenTree]) -> bool {
    matches!(
        before,
        [.., TokenTree::Ident(owner), TokenTree::Punct(first), TokenTree::Punct(second)]
            if owner == OWNER_PATH && first.as_char() == ':' && second.as_char() == ':'
    )
}
