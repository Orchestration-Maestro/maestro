//! Single-owner rule for native termination-signal subscription.
use std::collections::BTreeSet;

use proc_macro2::{TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::visit::Visit;

use super::imports::flatten;
use crate::source::{Member, Source, inherit_test_context, test_scope};

/// Crate that owns native termination delivery and its default-action facility.
const OWNER: &str = "maestro-process";

/// The owner's subscription entry point; every other crate delegates to it and returns it.
const ENTRY: &str = "register_signals";

/// Native facility only the owner may name.
const RAW_FACILITY: &str = "signal_hook";

/// Tokio signal kinds that deliver termination events.
const TERMINATION_KINDS: &[&str] = &["terminate", "hangup", "interrupt"];

/// Files outside the owner that re-export the entry point.
const EXPORTS: &[(&str, &str)] = &[("maestro-app", "src/exports/process.rs")];

/// Reject termination subscription outside its owner, and a subscription the caller drops.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = Vec::new();
    for member in members {
        if member.name == OWNER {
            continue;
        }
        for source in &member.sources {
            if test_scope(member, source) || reexport(member, source) || example(member, source) {
                continue;
            }
            let Some(syntax) = &source.syntax else {
                continue;
            };
            let Ok(tokens) = source.contents.parse::<TokenStream>() else {
                continue;
            };
            let mut report = Report {
                file: &source.path.display().to_string(),
                positions: &Positions::of(syntax),
                found: Vec::new(),
            };
            report.scan(&tokens);
            diagnostics.extend(report.found);
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        diagnostics.sort();
        diagnostics.dedup();
        Err(diagnostics.join("\n"))
    }
}

/// Whether `source` is a development example, which never ships.
fn example(member: &Member, source: &Source) -> bool {
    source
        .path
        .strip_prefix(&member.directory)
        .is_ok_and(|relative| relative.starts_with("examples"))
}

/// Whether `source` is the application file that re-exports the owner's entry point.
fn reexport(member: &Member, source: &Source) -> bool {
    source
        .path
        .strip_prefix(&member.directory)
        .is_ok_and(|relative| {
            let relative = relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            EXPORTS.contains(&(member.name.as_str(), relative.as_str()))
        })
}

/// Lines of one file that may hold the entry point's name without calling it, and the
/// local names that stand for it.
#[derive(Default)]
struct Positions {
    /// Lines inside a function's returned expression.
    returned: BTreeSet<usize>,
    /// Lines inside a `use` declaration, which binds the name without calling it.
    imported: BTreeSet<usize>,
    /// Local names the file imports the entry point under.
    aliases: BTreeSet<String>,
    /// Whether the current syntax belongs to tests.
    testing: bool,
}

impl Positions {
    /// Collect the delegating and importing lines of `syntax`.
    fn of(syntax: &syn::File) -> Self {
        let mut positions = Self::default();
        positions.visit_file(syntax);
        positions
    }

    /// Add every line of `span` to `into`.
    fn lines(into: &mut BTreeSet<usize>, span: proc_macro2::Span) {
        into.extend(span.start().line..=span.end().line);
    }

    /// Record the lines of a block's trailing expression.
    fn returned(&mut self, block: &syn::Block) {
        if let Some(syn::Stmt::Expr(expression, None)) = block.stmts.last() {
            Self::lines(&mut self.returned, expression.span());
        }
    }
}

impl<'ast> Visit<'ast> for Positions {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        Self::lines(&mut self.imported, item.span());
        if self.testing {
            return;
        }
        for import in flatten(&item.tree) {
            let Some(binding) = import.binding else {
                continue;
            };
            if import.item().map(ToString::to_string).as_deref() == Some(ENTRY) {
                self.aliases.insert(binding.unraw().to_string());
            }
        }
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.returned(&item.block);
        syn::visit::visit_item_fn(self, item);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.returned(&item.block);
        syn::visit::visit_impl_item_fn(self, item);
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        if let Some(block) = &item.default {
            self.returned(block);
        }
        syn::visit::visit_trait_item_fn(self, item);
    }

    inherit_test_context!(visit_item, Item, item_attributes);
    inherit_test_context!(visit_impl_item, ImplItem, impl_item_attributes);
    inherit_test_context!(visit_trait_item, TraitItem, trait_item_attributes);
}

/// One source's report under construction.
struct Report<'a> {
    /// The file the diagnostics name.
    file: &'a str,
    /// The positions that may hold a restricted name.
    positions: &'a Positions,
    /// Located diagnostics in source order.
    found: Vec<String>,
}

impl Report<'_> {
    /// Report each restricted name `stream` uses outside a position that delegates.
    fn scan(&mut self, stream: &TokenStream) {
        let tokens: Vec<_> = stream.clone().into_iter().collect();
        for (index, token) in tokens.iter().enumerate() {
            match token {
                TokenTree::Group(group) => self.scan(&group.stream()),
                TokenTree::Ident(found) => self.ident(&tokens, index, found),
                _ => {}
            }
        }
    }

    /// Report the restricted meaning of the identifier at `index`, when it has one.
    fn ident(&mut self, tokens: &[TokenTree], index: usize, found: &proc_macro2::Ident) {
        let spelling = found.to_string();
        let spelling = spelling.strip_prefix("r#").unwrap_or(&spelling);
        let line = found.span().start().line;
        let delegates =
            self.positions.imported.contains(&line) || self.positions.returned.contains(&line);
        let subscribes = (spelling == ENTRY || self.positions.aliases.contains(spelling))
            && !port(tokens, index);
        if subscribes && !delegates {
            self.found.push(format!(
                "{}:{line}: termination subscription is {OWNER}'s; delegate to it and return its registration result",
                self.file
            ));
        } else if spelling == RAW_FACILITY {
            self.found.push(format!(
                "{}:{line}: raw {RAW_FACILITY} belongs to {OWNER}",
                self.file
            ));
        } else if TERMINATION_KINDS.contains(&spelling) && kind_of(tokens, index) {
            self.found.push(format!(
                "{}:{line}: native termination delivery belongs to {OWNER}",
                self.file
            ));
        }
    }
}

/// Whether the identifier at `index` names a caller's port method or a declaration, not the
/// owner's free function: `connection.process.register_signals(..)` and `fn register_signals(..)`.
fn port(tokens: &[TokenTree], index: usize) -> bool {
    match tokens.get(index.wrapping_sub(1)) {
        Some(TokenTree::Punct(punctuation)) => punctuation.as_char() == '.',
        Some(above) => name(above).as_deref() == Some("fn"),
        None => false,
    }
}

/// Whether the identifier at `index` follows `SignalKind ::`.
fn kind_of(tokens: &[TokenTree], index: usize) -> bool {
    matches!(
        tokens.get(index.wrapping_sub(3)..index),
        Some([above, first, second])
            if name(above).as_deref() == Some("SignalKind")
                && is_colon(first)
                && is_colon(second)
    )
}

/// Whether `token` is a single `:` separator.
fn is_colon(token: &TokenTree) -> bool {
    matches!(token, TokenTree::Punct(punctuation) if punctuation.as_char() == ':')
}

/// The spelling of `token` when it is an identifier, without a raw prefix.
fn name(token: &TokenTree) -> Option<String> {
    match token {
        TokenTree::Ident(found) => {
            let spelling = found.to_string();
            Some(spelling.strip_prefix("r#").unwrap_or(&spelling).to_owned())
        }
        _ => None,
    }
}
