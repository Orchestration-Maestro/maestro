//! Development build profile check.

use std::path::Path;

use crate::source::read_manifest;

/// Require `[profile.dev] opt-level = 1` and no `opt-level` in `[profile.test]` or
/// `[profile.dev.package."*"]`: unoptimised code gives nearly every function its own unwind
/// record, which pushes the macOS test binaries over the linker's `__eh_frame` limit.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    let manifest = root.join("Cargo.toml");
    let document = read_manifest(&manifest)?;
    let profile = document.get("profile");
    let dev = profile.and_then(|profile| profile.get("dev"));
    let level = dev
        .and_then(|dev| dev.get("opt-level"))
        .and_then(toml::Value::as_integer);
    if level != Some(1) {
        return Err(format!(
            "{}: [profile.dev] must set opt-level = 1 to keep macOS __eh_frame under the linker limit",
            manifest.display()
        ));
    }
    let overrides = [
        ("[profile.test]", profile.and_then(|p| p.get("test"))),
        (
            "[profile.dev.package.\"*\"]",
            dev.and_then(|dev| dev.get("package"))
                .and_then(|package| package.get("*")),
        ),
    ];
    for (name, table) in overrides {
        if table.is_some_and(|table| table.get("opt-level").is_some()) {
            return Err(format!(
                "{}: {name} must not set opt-level; it would undo [profile.dev] opt-level = 1",
                manifest.display()
            ));
        }
    }
    Ok(())
}
