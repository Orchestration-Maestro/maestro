//! Pointer-format grammars independent of schema reference resolution.

use super::matches;

/// Check slash-separated pointer tokens and tilde escapes.
pub(super) fn json_pointer(value: &str) -> bool {
    matches(r"^(?:\/(?:[^~/]|~0|~1)*)*$", value, "")
}

/// Check hash-prefixed pointer tokens with percent escapes.
pub(super) fn json_pointer_uri_fragment(value: &str) -> bool {
    matches(
        r"^#(?:\/(?:[a-z0-9_\-.!$&'()*+,;:=@]|%[0-9a-f]{2}|~0|~1)*)*$",
        value,
        "i",
    )
}

/// Check canonical upward counts followed by a key marker or pointer.
pub(super) fn relative_json_pointer(value: &str) -> bool {
    matches(
        r"^(?:0|[1-9][0-9]*)(?:#|(?:\/(?:[^~/]|~0|~1)*)*)$",
        value,
        "",
    )
}
