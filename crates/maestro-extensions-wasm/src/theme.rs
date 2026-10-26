//! Retained theme capability for extension renderers.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[cfg(any(test, target_arch = "wasm32"))]
use std::any::Any;
use std::rc::Rc;

pub use crate::bindings::maestro::extension::host::{ColorMode, ThemeInfo, ThemeSetResult};
use crate::{ExtensionResult, SourceInfo};

/// Independently retained theme capability; clones share its host owner.
#[derive(Clone)]
pub struct Theme {
    /// The owner of this capability.
    port: Rc<dyn ThemePort>,
    /// The host resource behind a theme the host supplied, which a selection can pass back.
    #[cfg(any(test, target_arch = "wasm32"))]
    host: Option<Rc<dyn Any>>,
}
impl Theme {
    /// Retains the supplied host capability.
    #[must_use]
    pub fn new(port: Rc<dyn ThemePort>) -> Self {
        Self {
            port,
            #[cfg(any(test, target_arch = "wasm32"))]
            host: None,
        }
    }

    /// Retains a host-supplied theme whose resource a selection can pass back.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn supplied<P: ThemePort + 'static>(port: Rc<P>) -> Self {
        Self {
            port: port.clone(),
            host: Some(port),
        }
    }

    /// The host resource behind this theme, when the host supplied it as a `P`.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn host<P: 'static>(&self) -> Option<Rc<P>> {
        self.host.clone()?.downcast().ok()
    }
}

/// A theme to select: by name, or a retained theme.
#[derive(Clone)]
pub enum ThemeSelection {
    /// The name of a theme the host loads.
    Name(String),
    /// A theme the host supplied.
    Theme(Theme),
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
