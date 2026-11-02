use std::collections::HashMap;

use ra_ap_rustc_lexer::TokenKind;
use syn::{spanned::Spanned, visit::Visit};

use crate::source::{Member, Source, cfg_test, inherit_test_context, test_layout};

/// Exact lexical identity, with nested delimiters represented by interned children.
#[derive(PartialEq, Eq, Hash)]
enum Key<'a> {
    /// Original spelling of one non-comment token.
    Token(&'a str),
    /// Delimiter spelling and the exact ordered child identities.
    Group(&'a str, Vec<usize>),
}

/// One open delimiter in the iterative lexical walk.
struct Frame<'a> {
    /// Original opening delimiter.
    opening: &'a str,
    /// Byte location of that delimiter.
    start: usize,
    /// Already interned children.
    children: Vec<usize>,
    /// Interior lexical token count, including nested delimiters.
    count: usize,
}

impl Frame<'_> {
    /// Append one child identity with its flat lexical width.
    fn push(&mut self, id: usize, count: usize) {
        self.children.push(id);
        self.count += count;
    }
}

/// Canonical token identities shared only for one workspace check.
#[derive(Default)]
struct Tokens<'a>(HashMap<Key<'a>, usize>);

impl<'a> Tokens<'a> {
    /// Resolve exact equality through the map, never through a hash alone.
    fn intern(&mut self, key: Key<'a>) -> usize {
        #[cfg(test)]
        WORK.set(
            WORK.get()
                + match &key {
                    Key::Token(text) => text.len(),
                    Key::Group(_, children) => children.len(),
                },
        );
        let next = self.0.len();
        *self.0.entry(key).or_insert(next)
    }

    /// Index delimiter bodies once, without rescanning nested function source.
    fn blocks(&mut self, source: &'a str) -> HashMap<usize, (usize, usize)> {
        let mut blocks = HashMap::new();
        let mut stack: Vec<Frame<'a>> = Vec::new();
        for (kind, range) in crate::source::tokens(source) {
            let offset = range.start;
            let end = range.end;
            let text = &source[offset..end];
            if matches!(
                kind,
                TokenKind::Whitespace
                    | TokenKind::LineComment { .. }
                    | TokenKind::BlockComment { .. }
            ) {
                continue;
            }
            match text {
                "(" | "[" | "{" => stack.push(Frame {
                    opening: text,
                    start: offset,
                    children: Vec::new(),
                    count: 0,
                }),
                ")" | "]" | "}" => self.close(&mut stack, &mut blocks),
                _ => self.atom(text, &mut stack),
            }
        }
        blocks
    }
    /// Intern and attach one complete delimiter group.
    fn close(&mut self, stack: &mut Vec<Frame<'a>>, blocks: &mut HashMap<usize, (usize, usize)>) {
        let Some(frame) = stack.pop() else { return };
        let id = self.intern(Key::Group(frame.opening, frame.children));
        if frame.opening == "{" {
            blocks.insert(frame.start, (id, frame.count));
        }
        if let Some(parent) = stack.last_mut() {
            parent.push(id, frame.count + 2);
        }
    }

    /// Intern and attach one non-delimiter token.
    fn atom(&mut self, text: &'a str, stack: &mut [Frame<'a>]) {
        if let Some(parent) = stack.last_mut() {
            parent.push(self.intern(Key::Token(text)), 1);
        }
    }
}

/// Collect eligible function bodies using the source owner's parsed test context.
struct Bodies<'a> {
    /// Source path used in diagnostics.
    source: &'a Source,
    /// Constant-time identity lookup at a parsed block's byte offset.
    blocks: HashMap<usize, (usize, usize)>,
    /// All eligible authored locations, grouped by exact token identity and count.
    sites: &'a mut HashMap<usize, (usize, Vec<String>)>,
    /// Inherited explicit test configuration.
    testing: bool,
}

impl Bodies<'_> {
    /// Retain a production function's source location once it reaches the threshold.
    fn body(&mut self, signature: &syn::Signature, block: &syn::Block) {
        if !self.testing
            && let Some(&(id, count)) = self.blocks.get(&block.span().byte_range().start)
            && count >= 30
        {
            self.sites
                .entry(id)
                .or_insert_with(|| (count, Vec::new()))
                .1
                .push(format!(
                    "{}:{}",
                    self.source.path.display(),
                    signature.span().start().line,
                ));
        }
    }
}

impl<'ast> Visit<'ast> for Bodies<'_> {
    inherit_test_context!(visit_item, Item, item_attributes);
    inherit_test_context!(visit_impl_item, ImplItem, impl_item_attributes);
    inherit_test_context!(visit_trait_item, TraitItem, trait_item_attributes);

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.body(&item.sig, &item.block);
        syn::visit::visit_item_fn(self, item);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.body(&item.sig, &item.block);
        syn::visit::visit_impl_item_fn(self, item);
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        if let Some(block) = &item.default {
            self.body(&item.sig, block);
        }
        syn::visit::visit_trait_item_fn(self, item);
    }
}

/// Reject every exact duplicate group in parsed, hand-written production sources.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut tokens = Tokens::default();
    let mut sites = HashMap::new();
    for member in members {
        for source in &member.sources {
            let Some(syntax) = &source.syntax else {
                continue;
            };
            if test_layout(&source.path, &member.directory)
                || cfg_test(&syntax.attrs)
                || crate::generated::catalog_source(member, source)
                || crate::generated::bindings_source(member, source)
            {
                continue;
            }
            Bodies {
                source,
                blocks: tokens.blocks(&source.contents),
                sites: &mut sites,
                testing: false,
            }
            .visit_file(syntax);
        }
    }
    let mut diagnostics = Vec::new();
    for id in 0..tokens.0.len() {
        if let Some((count, locations)) = sites.remove(&id)
            && locations.len() > 1
        {
            diagnostics.push(format!(
                "exact duplicate body ({count} tokens): {}",
                locations.join(", ")
            ));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics.join("\n"))
    }
}

#[cfg(test)]
thread_local! {
    /// Test-side observation of bytes and child IDs presented to exact interning.
    static WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests;
