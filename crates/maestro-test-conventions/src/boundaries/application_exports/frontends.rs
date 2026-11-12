//! The reach check: a frontend names a lower-crate item through its owner, never through the
//! application.
//!
//! A frontend's row of the dependency table bounds every crate it can name. The application
//! could still hand it a lower-crate item through a re-export, so each path a frontend writes
//! below `maestro_app` is resolved through the application's own bindings and rejected when it
//! lands in another crate.

use std::collections::BTreeSet;

use proc_macro2::{TokenStream, TokenTree};
use syn::ext::IdentExt;

use super::{Aliases, resolve};
use crate::{
    graph::policy::FRONTENDS,
    source::{
        Member,
        imports::{Import, flatten},
    },
};

/// The application crate as frontend code spells it.
const APPLICATION: &str = "maestro_app";

/// Whether the member is a frontend, or the composition root, that the reach check covers.
pub(crate) fn checked(member: &Member) -> bool {
    FRONTENDS.contains(&member.name.as_str())
}

/// One written path below the application crate, with its line.
type Reach = (usize, Vec<String>);

/// Paths one frontend source writes below the application crate, gathered by the shared syntax
/// walk.
#[derive(Default)]
pub(crate) struct Paths {
    /// Located paths, each without the leading crate name.
    found: Vec<Reach>,
}

impl Paths {
    /// Record every item one use tree imports from the application crate.
    pub(crate) fn use_tree(&mut self, tree: &syn::UseTree) {
        self.found.extend(flatten(tree).iter().filter_map(applied));
    }

    /// Record a path written below the application crate.
    pub(crate) fn path(&mut self, path: &syn::Path) {
        if path.segments.len() > 1 && path.segments[0].ident == APPLICATION {
            let tail = path.segments.iter().skip(1);
            self.found.push((
                path.segments[0].ident.span().start().line,
                tail.map(|segment| segment.ident.unraw().to_string())
                    .collect(),
            ));
        }
    }

    /// Record every path written below the application crate inside a macro's tokens.
    pub(crate) fn macro_tokens(&mut self, invocation: &syn::Macro) {
        token_paths(&invocation.tokens, &mut self.found);
    }

    /// The located paths in source order.
    pub(crate) fn finish(self) -> Vec<Reach> {
        self.found
    }
}

/// The path below the application crate that one import names, with the line it starts on.
fn applied(import: &Import<'_>) -> Option<Reach> {
    let (first, rest) = import.path.split_first()?;
    if *first != APPLICATION {
        return None;
    }
    let mut path: Vec<_> = rest.iter().map(|name| name.unraw().to_string()).collect();
    if path.last().is_some_and(|name| name == "self") {
        path.pop();
    }
    Some((first.span().start().line, path))
}

/// Record the `maestro_app::a::b` chains inside macro tokens, nesting included.
fn token_paths(tokens: &TokenStream, found: &mut Vec<Reach>) {
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
    let mut index = 0;
    while index < trees.len() {
        match &trees[index] {
            TokenTree::Ident(ident) if ident == APPLICATION => {
                let mut path = Vec::new();
                let mut next = index + 1;
                while let Some((name, after)) = separator_then_name(&trees, next) {
                    path.push(name);
                    next = after;
                }
                if !path.is_empty() {
                    found.push((ident.span().start().line, path));
                }
                index = next;
            }
            TokenTree::Group(group) => {
                token_paths(&group.stream(), found);
                index += 1;
            }
            _ => index += 1,
        }
    }
}

/// The identifier after a `::` at `index`, with the index after it.
fn separator_then_name(trees: &[TokenTree], index: usize) -> Option<(String, usize)> {
    match trees.get(index..index + 3)? {
        [
            TokenTree::Punct(first),
            TokenTree::Punct(second),
            TokenTree::Ident(name),
        ] if first.as_char() == ':' && second.as_char() == ':' => {
            Some((name.unraw().to_string(), index + 3))
        }
        _ => None,
    }
}

/// Reject a frontend path that the application resolves into another crate.
pub(super) fn check(members: &[Member], aliases: &Aliases) -> Result<(), String> {
    let findings: BTreeSet<_> = members
        .iter()
        .filter(|member| checked(member))
        .flat_map(|member| &member.sources)
        .flat_map(|source| {
            let reach = source.walk.iter().flat_map(|walk| &walk.reach);
            reach.filter_map(move |(line, path)| {
                let (name, owner) = foreign(path, aliases)?;
                Some(format!(
                    "{}:{line}: {name} belongs to {owner}; import it from its owner, not through maestro-app",
                    source.path.display()
                ))
            })
        })
        .collect();
    if findings.is_empty() {
        Ok(())
    } else {
        Err(findings.into_iter().collect::<Vec<_>>().join("\n"))
    }
}

/// The item a written path names and the crate that owns it, when the application hands it
/// over from another crate.
fn foreign(path: &[String], aliases: &Aliases) -> Option<(String, String)> {
    (1..=path.len()).find_map(|length| {
        let resolved = resolve(&path[..length], &[], aliases);
        let first = resolved.first()?;
        let owner = first.strip_prefix("maestro_")?;
        Some((
            path[length - 1].clone(),
            format!("maestro-{}", owner.replace('_', "-")),
        ))
    })
}
