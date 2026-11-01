//! Shared terminal fixture projections.

pub(crate) mod kitty_terminal;

pub(crate) mod parsing;

pub(crate) mod terminal;

/// Whether this target sizes terminal pages in 16 KiB memory pages, as macOS on Apple silicon
/// does. Every page then holds more rows, so history controls record their own values there.
pub(crate) const LARGE_PAGES: bool = cfg!(all(target_os = "macos", target_arch = "aarch64"));
