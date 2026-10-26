//! Retained components supplied by extension authors.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use crate::{CallbackEmitter, ExtensionResult};
use std::rc::Rc;

/// An author's synchronous component operations.
pub trait ComponentView {
    /// Renders lines at the supplied viewport width.
    /// # Errors
    /// Returns the author's failure.
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>>;
    /// Handles literal input; the default does nothing.
    /// # Errors
    /// Returns the author's failure.
    fn handle_input(&self, _data: &str, _emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
    /// Reads the current key-release preference; the default is false.
    /// # Errors
    /// Returns the author's failure.
    fn wants_key_release(&self, _emitter: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        Ok(false)
    }
    /// Invalidates retained rendering state.
    /// # Errors
    /// Returns the author's failure.
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()>;
}

mod widgets;
use widgets::Widget;
pub use widgets::{
    Box, Container, DefaultTextStyle, DynamicBorder, Highlight, Markdown, MarkdownTheme, Spacer,
    Style, Text, TextDecoration, get_markdown_theme,
};
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) use widgets::{Widget as HostWidgetHandle, WidgetHost, WidgetPort};

/// Independently retained component; clones share the authored view.
#[derive(Clone)]
pub struct Component {
    /// The view.
    view: Rc<dyn ComponentView>,
    /// The host widget the view forwards to, when it is one.
    #[cfg(any(test, target_arch = "wasm32"))]
    widget: Option<Widget>,
}
impl Component {
    /// Retains the supplied author view.
    #[must_use]
    pub fn new(view: impl ComponentView + 'static) -> Self {
        Self {
            view: Rc::new(view),
            #[cfg(any(test, target_arch = "wasm32"))]
            widget: None,
        }
    }
    /// A view of a host widget, which the host keeps as the widget itself.
    fn hosted(view: impl ComponentView + 'static, widget: Widget) -> Self {
        #[cfg(not(any(test, target_arch = "wasm32")))]
        drop(widget);
        Self {
            view: Rc::new(view),
            #[cfg(any(test, target_arch = "wasm32"))]
            widget: Some(widget),
        }
    }
    /// The host widget behind this component, when it is one.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn widget(&self) -> Option<&Widget> {
        self.widget.as_ref()
    }
    /// Whether both share one view.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.view, &other.view)
    }
    /// Renders at the supplied width.
    /// # Errors
    /// Returns the author's failure.
    pub fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.view.render(width, emitter)
    }
    /// Forwards input to the retained view.
    /// # Errors
    /// Returns the author's failure.
    pub fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.view.handle_input(data, emitter)
    }
    /// Reads the retained view's current release preference.
    /// # Errors
    /// Returns the author's failure.
    pub fn wants_key_release(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        self.view.wants_key_release(emitter)
    }
    /// Invalidates the retained view.
    /// # Errors
    /// Returns the author's failure.
    pub fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.view.invalidate(emitter)
    }
}

port! {
    /// Requests to the terminal a factory received.
    TuiPort for TUI via _port {}
}

/// The terminal a component factory received; clones share its host owner.
#[derive(Clone)]
pub struct TUI {
    /// The owner of this capability, kept while the author holds the terminal.
    _port: Rc<dyn TuiPort>,
}
impl TUI {
    /// Retains the supplied host capability.
    #[must_use]
    pub fn new(port: Rc<dyn TuiPort>) -> Self {
        Self { _port: port }
    }
}

/// Disposal the host runs once when it removes a component, with callback-scoped emission.
pub type ComponentDisposer = Rc<dyn for<'a> Fn(CallbackEmitter<'a>) -> ExtensionResult<()>>;

/// A component a factory built, with its optional disposal.
#[derive(Clone)]
pub struct ExtensionComponent {
    /// The component the host shows.
    pub component: Component,
    /// Runs once when the host removes the component.
    pub dispose: Option<ComponentDisposer>,
}

/// Builds a component from the terminal and the live theme, synchronously.
pub type ComponentFactory = Rc<
    dyn for<'a> Fn(TUI, crate::Theme, CallbackEmitter<'a>) -> ExtensionResult<ExtensionComponent>,
>;

/// Runs when the footer's branch may have changed.
pub type BranchChangeListener = Rc<dyn Fn() -> ExtensionResult<()>>;

port! {
    /// Reads of the footer metadata a footer factory received; each read is current.
    FooterDataPort for FooterData via port {
        /// The current branch: `None` outside a repository, `detached` for a detached head.
        fn get_git_branch() -> Option<String>;
        /// The extension status texts by key, in insertion order.
        fn get_extension_statuses() -> Vec<(String, String)>;
        /// The count of providers with available models.
        fn get_available_provider_count() -> f64;
        /// Calls `listener` when the branch may have changed; dropping the subscription keeps
        /// the listener.
        fn on_branch_change(listener: BranchChangeListener) -> crate::Subscription;
    }
}

/// The footer metadata a footer factory received; clones share its host owner.
#[derive(Clone)]
pub struct FooterData {
    /// The owner of this capability.
    port: Rc<dyn FooterDataPort>,
}
impl FooterData {
    /// Retains the supplied host capability.
    #[must_use]
    pub fn new(port: Rc<dyn FooterDataPort>) -> Self {
        Self { port }
    }
}

/// Builds a footer from the terminal, the live theme and the footer metadata, synchronously.
pub type FooterFactory = Rc<
    dyn for<'a> Fn(
        TUI,
        crate::Theme,
        FooterData,
        CallbackEmitter<'a>,
    ) -> ExtensionResult<ExtensionComponent>,
>;
