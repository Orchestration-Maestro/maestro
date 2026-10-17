//! Literal and folded scalar indentation and final-newline policy.

use super::{FrontmatterError, parser::Parser};

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

impl Parser<'_> {
    /// Read a complete block scalar, stopping before the next collection entry.
    pub fn block_scalar(&mut self, parent: usize) -> Result<String, FrontmatterError> {
        let header = self.block_header()?;
        let mut indent = header.indent.map(|n| parent + n);
        let mut leading = 0;
        let mut lines = Vec::new();
        while self.cursor.peek().is_some() {
            let start = self.cursor.offset;
            while self.cursor.eat(' ') {}
            let spaces = self.cursor.offset - start;
            let empty = matches!(self.cursor.peek(), None | Some('\n'));
            if !empty && spaces <= parent {
                self.cursor.offset = start;
                break;
            }
            if !empty && indent.is_none() && leading > spaces {
                return Err(self.cursor.error("More-indented leading empty line"));
            }
            if !empty && indent.is_none() {
                indent = Some(spaces);
            }
            if !empty && spaces < indent.unwrap_or(parent + 1) {
                return Err(self.cursor.error("Under-indented block scalar"));
            }
            if indent.is_none() {
                leading = leading.max(spaces);
            }
            while !matches!(self.cursor.peek(), None | Some('\n')) {
                self.cursor.next();
            }
            let end = self.cursor.offset;
            let newline = self.cursor.eat('\n');
            lines.push((&self.cursor.source[start..end], newline));
        }
        Ok(render(&lines, indent.unwrap_or(parent + 1), &header))
    }

    /// Parse indentation/chomping in either legal order and a separated comment.
    fn block_header(&mut self) -> Result<Header, FrontmatterError> {
        let style = if self.cursor.eat('|') {
            BlockStyle::Literal
        } else {
            self.cursor.next();
            BlockStyle::Folded
        };
        let mut header = Header {
            style,
            indent: None,
            chomp: Chomp::Clip,
        };
        while let Some(ch) = self.cursor.peek() {
            match ch {
                '1'..='9' if header.indent.is_none() => {
                    header.indent = ch.to_digit(10).map(|n| n as usize);
                }
                '+' if header.chomp == Chomp::Clip => header.chomp = Chomp::Keep,
                '-' if header.chomp == Chomp::Clip => header.chomp = Chomp::Strip,
                ' ' | '\t' | '\n' => break,
                _ => return Err(self.cursor.error("Invalid block scalar header")),
            }
            self.cursor.next();
        }
        self.cursor.spaces();
        self.cursor.comment();
        if self.cursor.peek().is_some() && !self.cursor.eat('\n') {
            return Err(self.cursor.error("Unexpected block header content"));
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
    let mut value = String::new();
    let mut previous = None;
    for (index, line) in content[..end].iter().enumerate() {
        if let Some(prev) = previous {
            value.push_str(separator(prev, line, header.style == BlockStyle::Folded));
        }
        value.push_str(line);
        previous = Some(*line);
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

/// Fold ordinary lines, preserving blank and more-indented line boundaries.
fn separator(previous: &str, current: &str, folded: bool) -> &'static str {
    if !folded || previous.starts_with([' ', '\t']) || current.starts_with([' ', '\t']) {
        "\n"
    } else if current.is_empty() {
        ""
    } else if previous.is_empty() {
        "\n"
    } else {
        " "
    }
}
