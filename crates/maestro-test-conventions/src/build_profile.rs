//! Development build profile check.

use std::path::Path;

use crate::source::read_manifest;

/// Require `[profile.dev] opt-level = 1`: unoptimised code gives nearly every function its own
/// unwind record, which pushes the macOS test binaries over the linker's `__eh_frame` limit.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    let manifest = root.join("Cargo.toml");
    let level = read_manifest(&manifest)?
        .get("profile")
        .and_then(|profile| profile.get("dev"))
        .and_then(|dev| dev.get("opt-level"))
        .and_then(toml::Value::as_integer);
    if level == Some(1) {
        return Ok(());
    }
    Err(format!(
        "{}: [profile.dev] must set opt-level = 1",
        manifest.display()
    ))
}
