//! Sanitization shared by retained status producers.

/// Maximum visible Unicode scalars in a dynamic segment.
const MAX_STATUS_TEXT_CHARS: usize = 80;

/// Removes controls without changing printable spacing.
pub(super) fn sanitize_separator(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}

/// Keeps literal spacing while removing terminal controls.
pub(super) fn sanitize_literal_text(value: &str) -> Option<String> {
    let value: String = value
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    (!value.is_empty()).then_some(value)
}

/// Trims and bounds dynamic text after removing terminal and format controls.
pub(super) fn sanitize_status_text(value: &str) -> Option<String> {
    let value: String = value
        .trim()
        .chars()
        .filter(|character| !character.is_control() && !is_unicode_format_control(*character))
        .take(MAX_STATUS_TEXT_CHARS)
        .collect();
    (!value.is_empty()).then_some(value)
}

/// Unicode format controls with no visible status meaning.
fn is_unicode_format_control(character: char) -> bool {
    matches!(
        character,
        '\u{00ad}'
            | '\u{0600}'..='\u{0605}'
            | '\u{061c}'
            | '\u{06dd}'
            | '\u{070f}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08e2}'
            | '\u{17b4}'..='\u{17b5}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{110bd}'
            | '\u{110cd}'
            | '\u{13430}'..='\u{1343f}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            | '\u{e0001}'
            | '\u{e0020}'..='\u{e007f}'
    )
}

/// Selects the final complete or unterminated output line.
pub(super) fn command_output_text(output: &[u8]) -> Option<String> {
    let output = String::from_utf8_lossy(output);
    output.lines().next_back().and_then(sanitize_status_text)
}

#[cfg(test)]
mod tests;
