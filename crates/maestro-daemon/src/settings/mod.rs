//! Typed workspace preference values and validation.

/// Owned workspace preference areas.
pub mod model;

/// Outer terminal window-title templates.
pub mod titles;

#[cfg(test)]
mod tests;

/// Workspace theme choices and authored color data.
pub mod appearance;

/// Notification sound selection and authored paths.
pub mod audio;

/// Ordered right-side tab entries and schedule validation.
pub mod tabs;

/// Sidebar row composition and styling.
pub mod navigation;

/// Global workspace preference projection and reload.
pub mod io;

/// Annotated global workspace defaults; optional values remain commented out.
pub const DEFAULTS_TEXT: &str = include_str!("defaults.jsonc");
