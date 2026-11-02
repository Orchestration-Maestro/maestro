use std::path::Path;

use maestro_markdown::{Kind, MarkedOptions, Node, parse_with_options};
use ra_ap_rustc_lexer::TokenKind;
use syn::visit::Visit;

use crate::source::{Member, Source, cfg_test, test_layout};

#[cfg(test)]
thread_local! {
    /// Markdown parses performed by the documentation check on this thread.
    pub(crate) static MARKDOWN_PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Reject repeated documentation and invalid test source layouts.
pub(super) fn check(members: &[Member]) -> Result<(), String> {
    for member in members {
        for source in &member.sources {
            let generated_bindings = crate::generated::bindings_source(member, source);
            documentation(source, generated_bindings)?;
            check_test_layout(source, &member.directory)?;
        }
    }
    Ok(())
}

/// Check documentation tokens without interpreting comment-like text in literals.
fn documentation(source: &Source, generated_bindings: bool) -> Result<(), String> {
    let mut line = 1;
    let mut block = Documentation::default();
    let mut style = None;
    for (kind, range) in crate::source::tokens(&source.contents) {
        let offset = range.start;
        let end = range.end;
        let body = match kind {
            TokenKind::LineComment {
                doc_style: Some(owner),
            } => Some((&source.contents[offset + 3..end], owner)),
            TokenKind::BlockComment {
                doc_style: Some(_), ..
            } if !generated_bindings => {
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
            block.comment(body, line);
        }
        line += crate::source::count_newlines(&source.contents[offset..end]);
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
        let mut previous = None;
        if !self
            .markdown
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .any(|line| previous.replace(line) == Some(line))
        {
            return Ok(());
        }
        #[cfg(test)]
        MARKDOWN_PARSES.set(MARKDOWN_PARSES.get() + 1);
        let document = parse_with_options(self.markdown.clone(), MarkedOptions::Documentation);
        let mut code = Vec::new();
        code_ranges(&document.nodes, &mut code);
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

/// Collect original code-block ranges, including those inside list and quote containers.
fn code_ranges(nodes: &[Node], ranges: &mut Vec<std::ops::Range<usize>>) {
    let mut pending = vec![nodes];
    while let Some(nodes) = pending.pop() {
        for node in nodes {
            match &node.kind {
                Kind::CodeBlock(..) => ranges.push(node.range.clone()),
                Kind::List(_, children) | Kind::Item(children) | Kind::Quote(children, _) => {
                    pending.push(children);
                }
                _ => {}
            }
        }
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

#[cfg(test)]
mod tests {
    use super::code_ranges;
    use maestro_markdown::{Kind, MappedText, Node};

    #[test]
    fn deep_markdown_containers_keep_checks_on_a_small_stack() {
        let ranges = std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(|| {
                let mut node = Node {
                    kind: Kind::CodeBlock(String::new(), None),
                    authored: MappedText::default(),
                    range: 4..8,
                };
                for level in 0..100_000 {
                    let kind = match level % 3 {
                        0 => Kind::Quote(vec![node], MappedText::default()),
                        1 => Kind::Item(vec![node]),
                        _ => Kind::List(None, vec![node]),
                    };
                    node = Node {
                        kind,
                        authored: MappedText::default(),
                        range: 0..0,
                    };
                }
                let mut ranges = Vec::new();
                code_ranges(&[node], &mut ranges);
                ranges
            })
            .expect("worker thread")
            .join()
            .expect("worker panicked");
        assert_eq!(ranges, vec![4..8]);
    }
}
