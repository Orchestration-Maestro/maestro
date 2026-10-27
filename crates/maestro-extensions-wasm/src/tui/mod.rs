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
    /// Original native identity, only when this view directly retains a native component.
    fn native_component(&self) -> Option<NativeComponent> {
        None
    }
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
    /// The focus capability, absent for components without one.
    fn focusable(&self) -> Option<&dyn Focusable> {
        None
    }
    /// Invalidates retained rendering state.
    /// # Errors
    /// Returns the author's failure.
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()>;
}

#[macro_use]
mod widgets;
pub(crate) use widgets::host as widget_host;
mod custom;
mod editors;
pub use custom::{
    CustomClose, CustomComponentFactory, CustomDone, CustomFactory, CustomFactoryOutcome,
    CustomUIOptions, OverlayHandleCallback, OverlayOptionsResolver, OverlayOptionsSource,
};
mod focus;
mod overlays;
mod terminal;
pub use focus::{CURSOR_MARKER, Focusable, is_focusable};
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) use overlays::OverlayOptionsPort;
pub use overlays::{
    OverlayAnchor, OverlayHandle, OverlayHandlePort, OverlayMargin, OverlayMarginValue,
    OverlayOptions, OverlayOptionsHandle, OverlayVisible, SizeValue,
};
pub use terminal::{Terminal, TerminalPort};
mod keys;
mod lists;
pub use editors::{
    CustomEditor, CustomEditorPort, Editor, EditorOptions, EditorPort, EditorShortcut, TextHandler,
};
pub use keys::{
    EditorTheme, KeybindingConflict, KeybindingDefinition, KeybindingDefinitions, KeybindingKeys,
    KeybindingsConfig, KeybindingsManager, KeybindingsPort, get_editor_theme, get_keybindings,
    key_hint, key_text, raw_key_hint,
};
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) use lists::SubmenuDonePort;
pub use lists::{
    BorderedLoader, BorderedLoaderOptions, RowStyle, SelectHandler, SelectItem, SelectList,
    SelectListLayoutOptions, SelectListTheme, SelectListTruncatePrimaryContext, SettingChange,
    SettingItem, SettingsList, SettingsListOptions, SettingsListTheme, Submenu, SubmenuDone,
    TruncatePrimary, WidgetNotice, get_select_list_theme, get_settings_list_theme,
};
use widgets::Widget;
pub use widgets::{
    Box, Container, DefaultTextStyle, DynamicBorder, Highlight, Markdown, MarkdownTheme, Spacer,
    Style, Text, TextDecoration, get_markdown_theme,
};
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) use widgets::{
    CustomEditorParts, Supplied, Widget as HostWidgetHandle, WidgetHost, WidgetPort,
};

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
    /// Reads the original identity without unwrapping an author-supplied replacement.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn native_component(&self) -> Option<NativeComponent> {
        self.view.native_component()
    }
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
    /// A reference to the view that does not keep it.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn downgrade(&self) -> std::rc::Weak<dyn ComponentView> {
        Rc::downgrade(&self.view)
    }
    /// Whether `view` refers to this component's view.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn is(&self, view: &std::rc::Weak<dyn ComponentView>) -> bool {
        std::ptr::addr_eq(Rc::as_ptr(&self.view), view.as_ptr())
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
    /// Requests to the frame writer a factory received.
    TuiPort for TUI via port {
        /// The host frame writer's current count of full redraws.
        fn full_redraws() -> u64;
        /// The terminal the frame writer draws on.
        fn terminal() -> Terminal;
        /// Appends a component to the frame.
        fn add_child(component: &Component) -> ();
        /// Removes the first occurrence of a component; a missing one is ignored.
        fn remove_child(component: &Component) -> ();
        /// Removes every component of the frame.
        fn clear() -> ();
        /// Drops the cached rows of every component and overlay.
        fn invalidate() -> ();
        /// Renders the frame's components at `width`.
        fn render(width: u32) -> Vec<String>;
        /// Whether the hardware cursor is shown where a component asks for it.
        fn get_show_hardware_cursor() -> bool;
        /// Shows the hardware cursor where a component asks for it, or hides it.
        fn set_show_hardware_cursor(enabled: bool) -> ();
        /// Whether shrinking content clears the screen.
        fn get_clear_on_shrink() -> bool;
        /// Changes whether shrinking content clears the screen.
        fn set_clear_on_shrink(enabled: bool) -> ();
        /// Moves keyboard focus to `component`, or nowhere.
        fn set_focus(component: Option<&Component>) -> ();
        /// Shows `component` above the frame with the shared placement.
        fn show_overlay(
            component: &Component,
            options: Option<&OverlayOptionsHandle>
        ) -> OverlayHandle;
        /// Removes the most recently shown overlay and restores focus.
        fn hide_overlay() -> ();
        /// Whether any overlay is visible.
        fn has_overlay() -> bool;
        /// Starts the terminal and requests a frame.
        fn start() -> ();
        /// Moves the cursor past the frame and stops the terminal.
        fn stop() -> ();
        /// Requests a frame; `force` redraws the whole screen.
        fn request_render(force: bool) -> ();
        /// Adds a listener that sees input before the focused component; unsubscribing
        /// removes it.
        fn add_input_listener(listener: crate::TerminalInputHandler) -> crate::Subscription;
        /// Removes the listener added as `listener`; another one is ignored.
        fn remove_input_listener(listener: &crate::TerminalInputHandler) -> ();
        /// Replaces or removes the callback the debug key runs before the focused component.
        fn set_on_debug(callback: Option<WidgetNotice>) -> ();
    }
}

/// The terminal a component factory received; clones share its host owner.
#[derive(Clone)]
pub struct TUI {
    /// The owner of this capability, kept while the author holds the terminal.
    port: Rc<dyn TuiPort>,
    /// The host resource behind a terminal the host lent, which a widget can pass back.
    #[cfg(any(test, target_arch = "wasm32"))]
    host: Option<Rc<dyn std::any::Any>>,
}
impl TUI {
    /// Retains the supplied host capability.
    #[must_use]
    pub fn new(port: Rc<dyn TuiPort>) -> Self {
        Self {
            port,
            #[cfg(any(test, target_arch = "wasm32"))]
            host: None,
        }
    }
    /// Retains a host-lent terminal whose resource a widget can pass back.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn supplied<P: TuiPort + 'static>(port: Rc<P>) -> Self {
        Self {
            port: port.clone(),
            host: Some(port),
        }
    }
    /// The host resource behind this terminal, when the host lent it as a `P`.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn host<P: 'static>(&self) -> Option<Rc<P>> {
        self.host.clone()?.downcast().ok()
    }
}

/// Disposal the host runs once when it removes a component, with callback-scoped emission.
pub type ComponentDisposer = Rc<dyn for<'a> Fn(CallbackEmitter<'a>) -> ExtensionResult<()>>;

/// Changes a component's expansion synchronously with callback-scoped emission.
pub type ComponentExpansionHandler =
    Rc<dyn for<'a> Fn(bool, CallbackEmitter<'a>) -> ExtensionResult<()>>;

/// A component a factory built, with its optional disposal, expansion and width.
#[derive(Clone)]
pub struct ExtensionComponent {
    /// The component the host shows.
    pub component: Component,
    /// Runs once when the host removes the component.
    pub dispose: Option<ComponentDisposer>,
    /// Expands or collapses the component, when supported.
    pub set_expanded: Option<ComponentExpansionHandler>,
    /// Width an overlay can use when no explicit width was supplied.
    pub width: Option<f64>,
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

/// Opaque original row retained by the active adapter, never reconstructed by an author.
///
/// ```compile_fail
/// use maestro_extensions_wasm::NativeRow;
/// let _ = NativeRow {};
/// ```
#[derive(Clone)]
#[non_exhaustive]
pub struct NativeRow {
    /// Component adapter's independently retained original row.
    #[cfg(target_arch = "wasm32")]
    pub(crate) handle: Rc<crate::bindings::maestro::extension::host::NativeRow>,
}
/// Opaque original component identity retained by the active adapter.
///
/// ```compile_fail
/// use maestro_extensions_wasm::NativeComponent;
/// let _ = NativeComponent {};
/// ```
#[derive(Clone)]
#[non_exhaustive]
pub struct NativeComponent {
    /// Component adapter's independently retained original view.
    #[cfg(target_arch = "wasm32")]
    pub(crate) handle: Rc<crate::bindings::maestro::extension::host::NativeComponent>,
}
