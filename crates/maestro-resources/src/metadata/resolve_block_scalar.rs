//! Literal and folded scalar indentation and final-newline policy.

use super::{
    FrontmatterError,
    composer::Composer,
    cst::{Node, SourceToken, TokenKind as K},
};

/// Final-newline handling selected by a block header.
#[derive(Default, PartialEq, Eq)]
enum Chomp {
    /// Keep exactly one newline after nonempty content.
    #[default]
    Clip,
    /// Remove trailing newlines.
    Strip,
    /// Keep all authored trailing newlines.
    Keep,
}

/// Parsed block header policy.
struct Header {
    /// Literal versus folded representation.
    style: BlockStyle,
    /// Optional explicit indentation relative to the parent.
    indent: Option<usize>,
    /// Final-newline handling.
    chomp: Chomp,
}

/// How nonempty content lines join.
#[derive(PartialEq, Eq)]
enum BlockStyle {
    /// Preserve every line break.
    Literal,
    /// Fold ordinary adjacent lines.
    Folded,
}

impl Composer<'_> {
    /// Resolve a complete block scalar header and its lexer-delimited content.
    pub fn resolve_block_scalar(
        &self,
        node: &Node,
        props: &[SourceToken],
        source: Option<&SourceToken>,
    ) -> Result<String, FrontmatterError> {
        let first = props
            .first()
            .ok_or_else(|| self.error_at(node.offset, "Missing block scalar header"))?;
        let header = self.block_header(first)?;
        self.resolve_end(&props[1..], false)?;
        let text = source.map_or("", |token| self.text(token));
        let lines: Vec<_> = text
            .split_inclusive('\n')
            .map(|line| {
                (
                    line.strip_suffix('\n').unwrap_or(line),
                    line.ends_with('\n'),
                )
            })
            .collect();
        if lines
            .iter()
            .all(|(line, _)| line.bytes().all(|byte| byte == b' '))
        {
            return Ok(if header.chomp == Chomp::Keep && !text.is_empty() {
                "\n".repeat(text.bytes().filter(|byte| *byte == b'\n').count().max(1))
            } else {
                String::new()
            });
        }
        let indent = self.block_indent(&lines, node, &header)?;
        if indent == 0 && node.indent > 0 {
            return Err(self.error_at(node.offset, "Block scalar values must be indented"));
        }
        if lines.iter().any(|(line, _)| {
            !line.bytes().all(|byte| byte == b' ')
                && line.len() - line.trim_start_matches(' ').len() < indent
        }) {
            return Err(self.error_at(node.offset, "Under-indented block scalar"));
        }
        Ok(render(&lines, indent, &header))
    }

    /// Determine indentation from the first content line and leading blanks.
    fn block_indent(
        &self,
        lines: &[(&str, bool)],
        node: &Node,
        header: &Header,
    ) -> Result<usize, FrontmatterError> {
        let mut indent = node.indent + header.indent.unwrap_or_default();
        for (line, _) in lines {
            let spaces = line.len() - line.trim_start_matches(' ').len();
            let blank = line.bytes().all(|byte| byte == b' ');
            if blank && header.indent.is_none() {
                indent = indent.max(spaces);
            }
            if blank {
                continue;
            }
            if spaces < indent {
                return Err(self.error_at(
                    node.offset,
                    "More-indented leading empty line or under-indented content",
                ));
            }
            if header.indent.is_none() {
                indent = spaces;
            }
            break;
        }
        Ok(indent)
    }

    /// Decode indentation and chomping from the complete header token.
    fn block_header(&self, token: &SourceToken) -> Result<Header, FrontmatterError> {
        if token.kind != K::BlockScalarHeader {
            return Err(self.error_at(token.offset, "Invalid block scalar header"));
        }
        let mut chars = self.text(token).chars();
        let style = if chars.next() == Some('|') {
            BlockStyle::Literal
        } else {
            BlockStyle::Folded
        };
        let mut header = Header {
            style,
            indent: None,
            chomp: Chomp::Clip,
        };
        for ch in chars {
            match ch {
                '1'..='9' if header.indent.is_none() => {
                    header.indent = ch.to_digit(10).map(|n| n as usize);
                }
                '+' if header.chomp == Chomp::Clip => header.chomp = Chomp::Keep,
                '-' if header.chomp == Chomp::Clip => header.chomp = Chomp::Strip,
                _ => return Err(self.error_at(token.offset, "Invalid block scalar header")),
            }
        }
        Ok(header)
    }
}

/// Join content lines and apply the selected final-newline policy.
fn render(lines: &[(&str, bool)], indent: usize, header: &Header) -> String {
    let content: Vec<&str> = lines
        .iter()
        .map(|(line, _)| line.get(indent..).unwrap_or_default())
        .collect();
    let end = content
        .iter()
        .rposition(|s| !s.is_empty())
        .map_or(0, |n| n + 1);
    let lead = lines[..end]
        .iter()
        .position(|(line, _)| !line.bytes().all(|b| b == b' '))
        .unwrap_or(0);
    let mut value = String::new();
    for line in &content[..lead] {
        value.push_str(line);
        value.push('\n');
    }
    let mut fold = Fold::default();
    for (index, line) in content[..end].iter().enumerate().skip(lead) {
        match header.style {
            BlockStyle::Literal => {
                value.push_str(fold.sep);
                value.push_str(line);
                fold.sep = "\n";
            }
            BlockStyle::Folded => fold.push(&mut value, line),
        }
        if index + 1 == end && header.chomp != Chomp::Strip {
            value.push('\n');
        }
    }
    if header.chomp == Chomp::Keep {
        for (_, newline) in &lines[end..] {
            if *newline {
                value.push('\n');
            }
        }
    }
    value
}

/// Line separator state shared by literal and folded content.
#[derive(Default)]
struct Fold {
    /// Text inserted before the next content line.
    sep: &'static str,
    /// Whether the previous content line was more indented.
    prev_more_indented: bool,
}

impl Fold {
    /// Append a folded line, preserving blank and more-indented boundaries.
    fn push(&mut self, value: &mut String, line: &str) {
        if line.starts_with([' ', '\t']) {
            if self.sep == " " {
                self.sep = "\n";
            } else if !self.prev_more_indented && self.sep == "\n" {
                self.sep = "\n\n";
            }
            value.push_str(self.sep);
            value.push_str(line);
            self.sep = "\n";
            self.prev_more_indented = true;
        } else if line.is_empty() {
            if self.sep == "\n" {
                value.push('\n');
            } else {
                self.sep = "\n";
            }
        } else {
            value.push_str(self.sep);
            value.push_str(line);
            self.sep = " ";
            self.prev_more_indented = false;
        }
    }
}
