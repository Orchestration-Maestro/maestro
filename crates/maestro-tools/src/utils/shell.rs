//! Cleaning of captured process output for display.

/// Drop C0 controls other than TAB, LF and CR, and the interlinear annotation marks
/// U+FFF9 to U+FFFB; DEL, C1 controls, ZWJ and every other scalar stay.
#[must_use]
pub fn sanitize_binary_output(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            matches!(c, '\t' | '\n' | '\r')
                || !(c <= '\u{1f}' || ('\u{fff9}'..='\u{fffb}').contains(&c))
        })
        .collect()
}
