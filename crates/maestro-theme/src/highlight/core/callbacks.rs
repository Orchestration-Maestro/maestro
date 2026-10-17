//! Boundary admission with callback data owned by entered frames.
use super::{matcher::Hit, types::Callback};
use maestro_text::is_whitespace_scalar;

/// Whether this callback can reject a boundary match.
pub(super) fn may_ignore(kind: Callback) -> bool {
    matches!(
        kind,
        Callback::NotDot | Callback::Shebang | Callback::Same | Callback::Jsx
    )
}

/// Apply one admission callback without modifying immutable mode data.
pub(super) fn admit(
    kind: Callback,
    hit: &Hit<'_>,
    text: &str,
    delimiter: &mut Option<String>,
) -> bool {
    let capture = |index| hit.captures.get(index).copied().flatten();
    match kind {
        Callback::None => true,
        Callback::NotDot => !text[..hit.index].ends_with('.'),
        Callback::Shebang => hit.index == 0,
        Callback::Save => {
            *delimiter = capture(0).map(str::to_owned);
            true
        }
        Callback::SaveEither => {
            *delimiter = capture(0)
                .filter(|s| !s.is_empty())
                .or_else(|| capture(1))
                .map(str::to_owned);
            true
        }
        Callback::Same => delimiter.as_deref() == capture(0),
        Callback::Jsx => opening_tag(hit, text),
    }
}

/// Reject generic and comparison syntax while admitting real opening tags.
fn opening_tag(hit: &Hit<'_>, text: &str) -> bool {
    let tail = &text[hit.end..];
    let next = tail.chars().next();
    if matches!(next, Some('<' | ',')) {
        return false;
    }
    if next == Some('>') && !tail.contains(&format!("</{}", hit.text.trim_start_matches('<'))) {
        return false;
    }
    let trimmed = tail.trim_start_matches(is_whitespace_scalar);
    !(trimmed.starts_with('=')
        || trimmed.len() < tail.len()
            && trimmed
                .strip_prefix("extends")
                .is_some_and(|rest| rest.starts_with(is_whitespace_scalar)))
}
