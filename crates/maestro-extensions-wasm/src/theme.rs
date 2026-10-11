//! Opaque theme ownership for extension renderers.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;
/// Host ownership behind a theme capability.
pub trait ThemePort {}
/// Independently retained theme capability.
#[derive(Clone)]
pub struct Theme {
    /// The owner of this capability.
    _port: Rc<dyn ThemePort>,
}
impl Theme {
    /// Retains the supplied host capability.
    #[must_use]
    pub fn new(port: Rc<dyn ThemePort>) -> Self {
        Self { _port: port }
    }
}
