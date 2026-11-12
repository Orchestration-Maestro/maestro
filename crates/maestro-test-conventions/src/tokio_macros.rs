//! Tokio macros in production code.
//!
//! Tokio gates `select!`, `join!`, `try_join!` and the `main` and `test` attributes behind its
//! `macros` feature. A member whose production code spells one declares that feature itself, so
//! its build never leans on another member to supply it. The scan reads names, not types: a macro
//! path or an import that starts at `tokio`, however deeply other macros' arguments nest it.

use cargo_metadata::{DependencyKind, Metadata};
use proc_macro2::{Spacing, TokenStream, TokenTree};

use crate::source::{Member, imports};

/// The Tokio macros that need the `macros` feature.
const MACROS: &[&str] = &["select", "join", "try_join"];

/// The Tokio attributes that need the `macros` feature.
const ATTRIBUTES: &[&str] = &["main", "test"];

/// Reject production use of a Tokio macro in a member that does not declare the feature.
pub(crate) fn check(metadata: &Metadata, members: &[Member]) -> Result<(), String> {
    let mut diagnostics = Vec::new();
    for package in crate::graph::metadata::members(metadata) {
        let mut declarations = package
            .dependencies
            .iter()
            .filter(|dependency| {
                dependency.name == "tokio" && dependency.kind == DependencyKind::Normal
            })
            .peekable();
        if declarations.peek().is_none()
            || declarations.any(|dependency| {
                dependency
                    .features
                    .iter()
                    .any(|feature| feature == "macros")
            })
        {
            continue;
        }
        for member in members.iter().filter(|member| member.name == *package.name) {
            for source in &member.sources {
                let lines = source.walk.iter().flat_map(|walk| &walk.tokio_macros);
                diagnostics.extend(
                    lines.map(|line| (source.path.display().to_string(), *line, &member.name)),
                );
            }
        }
    }
    if diagnostics.is_empty() {
        return Ok(());
    }
    diagnostics.sort();
    Err(diagnostics
        .into_iter()
        .map(|(file, line, member)| {
            format!(
                "{file}:{line}: production code uses a tokio macro, but {member} does not declare tokio's `macros` feature"
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Production spellings of Tokio macros in one file, gathered by the shared syntax walk.
#[derive(Default)]
pub(crate) struct Spellings {
    /// Located lines.
    lines: Vec<usize>,
}

impl Spellings {
    /// Record a macro invocation qualified by `tokio`, and any other inside its arguments.
    pub(crate) fn invocation(&mut self, invocation: &syn::Macro) {
        if let Some(line) = qualified(
            invocation
                .path
                .segments
                .iter()
                .map(|segment| &segment.ident),
            MACROS,
        ) {
            self.lines.push(line);
        }
        token_spellings(&invocation.tokens, &mut self.lines);
    }

    /// Record an attribute qualified by `tokio`.
    pub(crate) fn attribute(&mut self, attribute: &syn::Attribute) {
        let path = attribute
            .path()
            .segments
            .iter()
            .map(|segment| &segment.ident);
        if let Some(line) = qualified(path, ATTRIBUTES) {
            self.lines.push(line);
        }
    }

    /// Record every import of a Tokio macro, whatever it is renamed to.
    pub(crate) fn import(&mut self, tree: &syn::UseTree) {
        for import in imports::flatten(tree) {
            if let [first, last] = import.path.as_slice()
                && *first == "tokio"
                && MACROS.iter().any(|name| last == name)
            {
                self.lines.push(last.span().start().line);
            }
        }
    }

    /// The located lines in order, each once.
    pub(crate) fn finish(mut self) -> Vec<usize> {
        self.lines.sort_unstable();
        self.lines.dedup();
        self.lines
    }
}

/// The line of a two-segment path that starts at `tokio` and ends in one of the names.
fn qualified<'a>(
    mut path: impl Iterator<Item = &'a proc_macro2::Ident>,
    names: &[&str],
) -> Option<usize> {
    let (first, second) = (path.next()?, path.next()?);
    (first == "tokio" && path.next().is_none() && names.iter().any(|name| second == name))
        .then(|| first.span().start().line)
}

/// Record `tokio::<macro>!` anywhere inside macro tokens, nesting included.
fn token_spellings(tokens: &TokenStream, lines: &mut Vec<usize>) {
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
    for (index, tree) in trees.iter().enumerate() {
        match tree {
            TokenTree::Ident(ident)
                if MACROS.iter().any(|name| ident == name)
                    && matches!(trees.get(index + 1), Some(TokenTree::Punct(bang)) if bang.as_char() == '!' && bang.spacing() == Spacing::Alone) =>
            {
                if let [
                    ..,
                    TokenTree::Ident(owner),
                    TokenTree::Punct(first),
                    TokenTree::Punct(second),
                ] = &trees[..index]
                    && owner == "tokio"
                    && first.as_char() == ':'
                    && second.as_char() == ':'
                {
                    lines.push(owner.span().start().line);
                }
            }
            TokenTree::Group(group) => token_spellings(&group.stream(), lines),
            _ => {}
        }
    }
}
