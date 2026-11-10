//! Clipboard image MIME policy, display detection and native reading.

/// Detects a nonempty Wayland display or the exact Wayland session type.
pub fn is_wayland_session(env: impl Fn(&str) -> Option<String>) -> bool {
    env("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty())
        || env("XDG_SESSION_TYPE").as_deref() == Some("wayland")
}

/// Supported image types in preference order.
#[cfg(not(target_arch = "wasm32"))]
const SUPPORTED: [&str; 4] = ["image/png", "image/jpeg", "image/webp", "image/gif"];

/// Trims the whitespace accepted at clipboard text boundaries.
fn trim(text: &str) -> &str {
    text.trim_matches(maestro_text::is_whitespace_scalar)
}

/// Trims and lowercases the first MIME segment, discarding parameters.
fn base_mime_type(mime_type: &str) -> String {
    trim(mime_type.split(';').next().unwrap_or_default()).to_lowercase()
}

/// Returns the supported suffix after normalizing the base MIME type.
#[must_use]
pub fn extension_for_image_mime_type(mime_type: &str) -> Option<&'static str> {
    match base_mime_type(mime_type).as_str() {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        _ => None,
    }
}

/// Native image effects and routing.
#[cfg(not(target_arch = "wasm32"))]
mod reader;
#[cfg(not(target_arch = "wasm32"))]
pub use reader::{
    ClipboardImage, ClipboardImageOperations, ReadClipboardImageOptions, read_clipboard_image,
    read_clipboard_image_with,
};
