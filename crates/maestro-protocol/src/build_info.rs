//! Build identity helpers.
//! Adapted for Maestro: names, module layout and Rust documentation changed.

/// Base version.
pub const BASE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Channel.
#[must_use]
pub fn channel() -> &'static str {
    non_empty(option_env!("MAESTRO_BUILD_CHANNEL")).unwrap_or("stable")
}

/// Build id.
#[must_use]
pub fn build_id() -> Option<&'static str> {
    non_empty(option_env!("MAESTRO_BUILD_ID"))
}

/// Version.
#[must_use]
pub fn version() -> String {
    match channel() {
        "stable" => BASE_VERSION.to_string(),
        channel => match build_id() {
            Some(build_id) => format!("{BASE_VERSION}-{channel}.{build_id}"),
            None => format!("{BASE_VERSION}-{channel}"),
        },
    }
}

/// Is preview.
#[must_use]
pub fn is_preview() -> bool {
    channel() == "preview"
}

/// Non empty.
fn non_empty(value: Option<&'static str>) -> Option<&'static str> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}
