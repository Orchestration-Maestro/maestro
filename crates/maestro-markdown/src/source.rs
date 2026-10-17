//! Container-local source framing.
use std::ops::Range;
/// Authored source and the native containers enclosing the current events.
#[derive(Clone, Copy)]
pub(crate) struct Source<'a> {
    /// Normalized original input.
    pub(crate) text: &'a str,
    /// Container prefixes removed by the native parser on continuation lines.
    pub(crate) frame: Option<&'a Frame<'a>>,
    /// Whether the events belong to a table cell, where `\|` spells a literal pipe.
    pub(crate) cell: bool,
}
/// One enclosing native block's continuation prefix.
pub(crate) struct Frame<'a> {
    /// Outer block, stripped before this block.
    pub(crate) parent: Option<&'a Frame<'a>>,
    /// Quote marker or list continuation indentation.
    pub(crate) kind: Prefix,
}
/// Native block framing relevant to authored inline spans.
pub(crate) enum Prefix {
    /// One quote marker with its optional following space.
    Quote,
    /// List marker width on the item's first line.
    Item(usize),
}
impl Frame<'_> {
    /// Removes only prefixes belonging to the captured container path.
    pub(crate) fn strip<'a>(&self, line: &'a str) -> &'a str {
        let line = self.parent.map_or(line, |parent| parent.strip(line));
        match self.kind {
            Prefix::Quote => strip_quote(line),
            Prefix::Item(width) => {
                let spaces = line.bytes().take_while(|byte| *byte == b' ').count();
                &line[spaces.min(width)..]
            }
        }
    }
}
impl Source<'_> {
    /// Retains authored bytes while removing enclosing continuation prefixes.
    pub(crate) fn authored(&self, range: Range<usize>) -> String {
        let authored = self.text[range]
            .split('\n')
            .enumerate()
            .map(|(index, line)| {
                if index == 0 {
                    line
                } else {
                    self.frame.map_or(line, |frame| frame.strip(line))
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.spelling(authored)
    }

    /// Spells a table cell's `\|` as a literal pipe; other sources pass through unchanged.
    pub(crate) fn spelling(&self, text: String) -> String {
        if self.cell {
            text.replace("\\|", "|")
        } else {
            text
        }
    }
}
/// Removes a native quote prefix, leaving lazy continuation text intact.
pub(crate) fn strip_quote(line: &str) -> &str {
    let spaces = line.bytes().take_while(|byte| *byte == b' ').count().min(3);
    line[spaces..]
        .strip_prefix('>')
        .map_or(line, |line| line.strip_prefix(' ').unwrap_or(line))
}
