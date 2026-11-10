//! Input preparation operations shared by the command-line frontend.

pub mod arguments;
pub mod configuration;
pub mod file_processor;
pub mod messages;
pub mod model_listing;
pub mod selection;
pub mod session_picker;

/// Pad `text` to `width` UTF-16 code units with trailing spaces.
///
/// Longer text is returned unchanged, and no valid string is ever split: the
/// measure counts the same units the authored tables and help labels are aligned by.
fn pad_column(text: &str, width: usize) -> String {
    let measure = text.encode_utf16().count();
    let mut padded = String::from(text);
    padded.extend(std::iter::repeat_n(' ', width.saturating_sub(measure)));
    padded
}
