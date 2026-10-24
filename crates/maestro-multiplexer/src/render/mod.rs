//! Pane cells into frame buffers, frame records and terminal bytes.

pub mod ansi;
pub mod conversion;
pub mod pane;

/// Whether a symbol is one halfwidth katakana followed by its voiced or
/// semi-voiced mark, which host terminals draw two columns wide.
fn is_halfwidth_katakana_voiced_grapheme(symbol: &str) -> bool {
    let mut chars = symbol.chars();
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (
            Some('\u{ff66}'..='\u{ff9d}'),
            Some('\u{ff9e}' | '\u{ff9f}'),
            None
        )
    )
}
