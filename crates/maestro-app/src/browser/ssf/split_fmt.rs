//! Protected spreadsheet format section boundaries.

#[cfg(test)]
use super::FormatError;

/// Borrow sections while retaining protected delimiters and empty sections.
#[cfg(test)]
pub(super) fn split_fmt(format: &str) -> Result<Vec<&str>, FormatError> {
    let mut sections = Vec::new();
    let (mut start, mut quoted) = (0, false);
    let mut characters = format.char_indices();
    while let Some((offset, character)) = characters.next() {
        match character {
            '"' => quoted = !quoted,
            '\\' | '_' | '*' => {
                characters.next();
            }
            ';' if !quoted => {
                sections.push(&format[start..offset]);
                start = offset + 1;
            }
            _ => {}
        }
    }
    if quoted {
        return Err(FormatError::UnterminatedString);
    }
    sections.push(&format[start..]);
    Ok(sections)
}
