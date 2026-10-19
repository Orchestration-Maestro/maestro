//! Typed workspace preference values and validation.

/// Owned workspace preference areas.
pub mod model;

/// Outer terminal window-title templates.
pub mod window_title;

#[cfg(test)]
mod tests;

/// Workspace theme choices and authored color data.
pub mod theme;

/// Notification sound selection and authored paths.
pub mod sound;

/// Ordered right-side tab entries and schedule validation.
pub mod tab_bar;

/// Sidebar row composition and styling.
pub mod sidebar;

/// Global workspace preference projection and reload.
pub mod io;

/// Annotated global workspace defaults; optional values remain commented out.
pub const DEFAULTS_TEXT: &str = include_str!("defaults.jsonc");
