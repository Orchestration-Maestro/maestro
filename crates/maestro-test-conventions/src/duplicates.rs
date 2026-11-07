use std::collections::HashMap;

use ra_ap_rustc_lexer::TokenKind;
use syn::spanned::Spanned;

use crate::source::{Member, Source};

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
    fn blocks(&mut self, source: &'a Source) -> HashMap<usize, (usize, usize)> {
        let mut blocks = HashMap::new();
        let mut stack: Vec<Frame<'a>> = Vec::new();
        for (kind, range) in &source.tokens {
            let offset = range.start;
            let end = range.end;
            let text = &source.contents[offset..end];
            if matches!(
                kind,
                TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
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

/// Function bodies of one source, production and test code alike, gathered by the shared
/// syntax walk.
///
/// Each entry holds the body's opening-brace byte offset and its signature's line.
#[derive(Default)]
pub(crate) struct Bodies(pub(crate) Vec<(usize, usize)>);

impl Bodies {
    /// Record one function body.
    pub(crate) fn function(&mut self, signature: &syn::Signature, block: &syn::Block) {
        self.0
            .push((block.span().byte_range().start, signature_line(signature)));
    }
}

/// Line where a signature starts: its first qualifier, or else its `fn` keyword.
///
/// The line comes from that token's own span, so no signature text is printed to find it.
fn signature_line(signature: &syn::Signature) -> usize {
    let safety = match &signature.safety {
        syn::Safety::Safe(token) => Some(token.span),
        syn::Safety::Unsafe(token) => Some(token.span),
        syn::Safety::Default => None,
    };
    signature
        .constness
        .as_ref()
        .map(|token| token.span)
        .or_else(|| signature.asyncness.as_ref().map(|token| token.span))
        .or(safety)
        .or_else(|| signature.abi.as_ref().map(|abi| abi.extern_token.span))
        .unwrap_or(signature.fn_token.span)
        .start()
        .line
}

/// Reject every exact duplicate group in parsed, hand-written sources, test code included.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut tokens = Tokens::default();
    let mut sites = HashMap::new();
    for member in members {
        for source in &member.sources {
            let Some(walk) = &source.walk else {
                continue;
            };
            if crate::generated::catalog_source(member, source)
                || crate::generated::bindings_source(member, source)
            {
                continue;
            }
            let blocks = tokens.blocks(source);
            let bodies = walk.bodies.iter().filter_map(|(start, line)| {
                let &(id, count) = blocks.get(start)?;
                (count >= 30).then_some((id, count, line))
            });
            for (id, count, line) in bodies {
                sites
                    .entry(id)
                    .or_insert_with(|| (count, Vec::new()))
                    .1
                    .push(format!("{}:{line}", source.path.display()));
            }
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
