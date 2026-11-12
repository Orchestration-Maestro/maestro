//! Class names derived from authored style identifiers.
/// Collapse literal space/dot and ampersand runs, then lowercase Unicode text.
pub(crate) fn escape_class_name(class_name: &str) -> String {
    let mut result = String::new();
    let mut previous = None;
    for character in class_name.chars() {
        let run = match character {
            ' ' | '.' => Some('-'),
            '&' => Some('&'),
            _ => None,
        };
        if run.is_none() || run != previous {
            match run {
                Some('-') => result.push('-'),
                Some('&') => result.push_str("and"),
                _ => result.push(character),
            }
        }
        previous = run;
    }
    result.to_lowercase()
}
