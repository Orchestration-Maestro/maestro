//! Display policy shared by clipboard operations.

/// Detects a nonempty Wayland display or the exact Wayland session type.
pub fn is_wayland_session(env: impl Fn(&str) -> Option<String>) -> bool {
    env("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty())
        || env("XDG_SESSION_TYPE").as_deref() == Some("wayland")
}
