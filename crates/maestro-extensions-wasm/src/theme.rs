//! Retained theme capability for extension renderers.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

pub use crate::bindings::maestro::extension::host::ColorMode;
use crate::{ExtensionResult, SourceInfo};

/// Independently retained theme capability; clones share its host owner.
#[derive(Clone)]
pub struct Theme {
    /// The owner of this capability.
    port: Rc<dyn ThemePort>,
}
impl Theme {
    /// Retains the supplied host capability.
    #[must_use]
    pub fn new(port: Rc<dyn ThemePort>) -> Self {
        Self { port }
    }
}

port! {
    /// Requests to the theme's owner; each method reads the owner's current state and resolves
    /// to the owner's message when it rejects the request.
    ThemePort for Theme via port {
        /// The theme name, when it has one.
        fn name() -> Option<String>;
        /// The file the theme was loaded from, when it has one.
        fn source_path() -> Option<String>;
        /// The theme's provenance, when it has one.
        fn source_info() -> Option<SourceInfo>;
        /// Replaces the theme's provenance, or clears it.
        fn set_source_info(source: Option<SourceInfo>) -> ();
        /// Wraps text in a foreground color, restoring only the foreground.
        fn fg(color: &str, text: &str) -> String;
        /// Wraps text in a background color, restoring only the background.
        fn bg(color: &str, text: &str) -> String;
        /// Styles text bold.
        fn bold(text: &str) -> String;
        /// Styles text italic.
        fn italic(text: &str) -> String;
        /// Underlines text.
        fn underline(text: &str) -> String;
        /// Swaps the foreground and background of text.
        fn inverse(text: &str) -> String;
        /// Strikes text through.
        fn strikethrough(text: &str) -> String;
        /// The escape that selects a foreground color.
        fn get_fg_ansi(color: &str) -> String;
        /// The escape that selects a background color.
        fn get_bg_ansi(color: &str) -> String;
        /// The color depth the theme renders at.
        fn get_color_mode() -> ColorMode;
    }
}
