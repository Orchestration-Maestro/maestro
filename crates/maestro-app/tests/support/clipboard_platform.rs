//! Platform spellings shared by clipboard fixtures.
#![cfg(test)]
use maestro_app::utils::clipboard::ClipboardPlatform;

/// Platform conversion for the recorded host observations.
pub fn platform(name: &str) -> ClipboardPlatform {
    match name {
        "linux" => ClipboardPlatform::Linux,
        "darwin" => ClipboardPlatform::Darwin,
        "win32" => ClipboardPlatform::Win32,
        _ => ClipboardPlatform::Other,
    }
}
