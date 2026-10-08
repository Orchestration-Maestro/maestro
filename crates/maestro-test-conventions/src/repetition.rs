use std::path::Path;

use pulldown_cmark::{Event, Parser, Tag};
use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind};
use syn::visit::Visit;

use crate::source::{Member, Source, cfg_test, test_layout};

/// Reject repeated documentation and invalid test source layouts.
pub(super) fn check(members: &[Member]) -> Result<(), String> {
    for member in members {
        for source in &member.sources {
            let generated_bindings = member.name == "maestro-extensions-wasm"
                && source.path == member.directory.join("src/bindings.rs");
            documentation(source, generated_bindings)?;
            check_test_layout(source, &member.directory)?;
        }
    }
    Ok(())
}

/// Check documentation tokens without interpreting comment-like text in literals.
fn documentation(source: &Source, generated_bindings: bool) -> Result<(), String> {
    let mut offset = ra_ap_rustc_lexer::strip_shebang(&source.contents).unwrap_or(0);
    let mut block = Documentation::default();
    let mut style = None;
    for token in ra_ap_rustc_lexer::tokenize(&source.contents[offset..], FrontmatterAllowed::No) {
        let end = offset + token.len as usize;
        let body = match token.kind {
            TokenKind::LineComment {
                doc_style: Some(owner),
            } => Some((&source.contents[offset + 3..end], owner)),
            TokenKind::BlockComment {
                doc_style: Some(_), ..
            } if !generated_bindings => {
                let line = crate::source::line(&source.contents, offset);
                return Err(format!(
                    "{}:{line}: block documentation: use /// or //!",
                    source.path.display()
                ));
            }
            TokenKind::Whitespace => None,
            _ => {
                block.check(source)?;
                block = Documentation::default();
                style = None;
                None
            }
        };
        if let Some((body, owner)) = body {
            if style != Some(owner) {
                block.check(source)?;
                block = Documentation::default();
                style = Some(owner);
            }
            let start = crate::source::line(&source.contents, offset);
            block.comment(body, start);
        }
        offset = end;
    }
    block.check(source)
}

#[derive(Default)]
/// Documentation state for a contiguous comment block.
struct Documentation {
    /// Markdown source retaining indentation for code-block parsing.
    markdown: String,
    /// Physical source locations of the Markdown lines.
    lines: Vec<usize>,
}

impl Documentation {
    /// Retain a documentation line, removing one separator space.
    fn comment(&mut self, body: &str, start: usize) {
        self.markdown
            .push_str(body.strip_prefix(' ').unwrap_or(body));
        self.markdown.push('\n');
        self.lines.push(start);
    }

    /// Compare physical lines, excluding parser-selected code block ranges.
    fn check(&self, source: &Source) -> Result<(), String> {
        let code: Vec<_> = Parser::new(&self.markdown)
            .into_offset_iter()
            .filter_map(|(event, range)| {
                matches!(event, Event::Start(Tag::CodeBlock(_))).then_some(range)
            })
            .collect();
        let mut previous = None;
        let mut offset = 0;
        for (index, text) in self.markdown.lines().enumerate() {
            let end = offset + text.len();
            let inside_code = code
                .iter()
                .any(|range| range.start < end && offset < range.end);
            offset = end + 1;
            if inside_code {
                previous = None;
                continue;
            }
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            let line = self.lines[index];
            if let Some((prior, first)) = previous
                && prior == text
            {
                return Err(format!(
                    "{}:{line}: repeated documentation; first at line {first}",
                    source.path.display()
                ));
            }
            previous = Some((text, line));
        }
        Ok(())
    }
}

/// Validate the layout used to exclude test sources from production line counts.
fn check_test_layout(source: &Source, directory: &Path) -> Result<(), String> {
    let mut visitor = TestLayout {
        source,
        in_src: source.path.starts_with(directory.join("src")),
        error: None,
    };
    if let Some(syntax) = &source.syntax {
        visitor.visit_file(syntax);
        visitor.error.take().map_or(Ok(()), Err)?;
        if visitor.in_src && cfg_test(&syntax.attrs) && !test_layout(&source.path, directory) {
            return Err(format!(
                "{}:1: test file convention: use tests.rs or a tests directory for #![cfg(test)]",
                source.path.display()
            ));
        }
    }
    visitor.error.map_or(Ok(()), Err)
}

/// Validate source-local test module declarations throughout the syntax tree.
struct TestLayout<'a> {
    /// File used to attach locations to diagnostics.
    source: &'a Source,
    /// Whether source-local module declarations must follow the convention.
    in_src: bool,
    /// First convention violation, if any.
    error: Option<String>,
}

impl<'ast> Visit<'ast> for TestLayout<'_> {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let configured = cfg_test(&item.attrs);
        let named = item.ident == "tests";
        if self.in_src
            && (configured != named
                || ((configured || named)
                    && item.attrs.iter().any(|attr| attr.path().is_ident("path"))))
            && self.error.is_none()
        {
            self.error = Some(format!(
                "{}:{}: test module convention: use #[cfg(test)] mod tests without #[path]",
                self.source.path.display(),
                item.ident.span().start().line,
            ));
        }
        syn::visit::visit_item_mod(self, item);
    }
}
