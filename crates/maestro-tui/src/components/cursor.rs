//! Display-only cursor mapping around recognized terminal escapes.
/// Advances a display cursor out of recognized escapes, including adjacent escapes.
pub(crate) fn display_cursor(text: &str, mut cursor: usize) -> usize {
    let endings = crate::text::Endings::of(text);
    let mut position = 0;
    while position < text.len() && position <= cursor {
        if let Some(escape) = endings.recognize(text, position) {
            let end = position + escape.len();
            if cursor >= position && cursor < end {
                cursor = end;
            }
            position = end;
        } else if let Some(scalar) = text[position..].chars().next() {
            position += scalar.len_utf8();
        }
    }
    cursor
}
