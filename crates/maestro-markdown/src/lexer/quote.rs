//! Quote-prefix and setext spelling transformations.
use crate::source::{Cursor, ScopeSource, Segmented};

/// A recognized quote prefix and the operand its body admits.
pub(crate) struct QuoteEntry {
    /// Complete recognized raw, projected for the admitted extent.
    pub(crate) raw: ScopeSource,
    /// Body operand without the quote prefix.
    pub(crate) body: Segmented,
}

/// Rewrite unquoted setext spelling within this chunk before removing prefixes.
pub(super) fn quote_source(source: &ScopeSource) -> ScopeSource {
    let mut pending = Cursor::default();
    pending.append(source.clone());
    let mut output = Cursor::default();
    let mut first = true;
    while let Some(line) = pending.line() {
        let line = line.materialize();
        let shown = crate::source::strip_quote(&line.text);
        let mut piece = line.select(std::iter::once(
            line.text.len() - shown.len()..line.text.len(),
        ));
        let spaces = line.text.bytes().take_while(|byte| *byte == b' ').count();
        let label = line.text[spaces..].trim_end_matches([' ', '\n']);
        if !first
            && spaces <= 3
            && !label.is_empty()
            && (label.bytes().all(|byte| byte == b'=') || label.bytes().all(|byte| byte == b'-'))
        {
            piece = piece.replace_prefix(spaces, "    ");
        }
        output.append(piece);
        first = false;
    }
    output.materialize()
}
