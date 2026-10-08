//! Components the host asks to draw and the renderers that produce them.

use std::rc::Rc;

use crate::api::ExtensionResult;
use crate::bindings::maestro::extension::types::CustomMessage;

/// What an extension implements to draw itself.
pub trait ComponentView {
    /// Lines for the given width.
    fn render(&self, width: u32) -> Vec<String>;
    /// Drops cached output.
    fn invalidate(&self) {}
}

/// An owned component handed to the host.
pub struct Component(Box<dyn ComponentView>);

impl Component {
    /// Wraps a view; the host owns the component until it drops it.
    pub fn new(view: impl ComponentView + 'static) -> Self {
        Self(Box::new(view))
    }

    /// Hands the author's view to the component adapter.
    pub(crate) fn into_view(self) -> Box<dyn ComponentView> {
        self.0
    }

    /// Lines for the given width.
    #[must_use]
    pub fn render(&self, width: u32) -> Vec<String> {
        self.0.render(width)
    }

    /// Drops cached output.
    pub fn invalidate(&self) {
        self.0.invalidate();
    }
}

/// Renders a custom message; `None` keeps the host's default rendering.
pub type MessageRenderer = Rc<dyn Fn(CustomMessage, bool) -> ExtensionResult<Option<Component>>>;
