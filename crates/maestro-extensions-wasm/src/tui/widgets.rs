//! Toolkit and application widgets the host keeps for an extension.
//!
//! Each facade forwards to one widget the host built; building one needs no interactive UI.
//! A widget joins a container or a component tree as a [`Component`].
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::{Component, ComponentView};
use crate::{CallbackEmitter, ExtensionResult};
#[cfg(any(test, target_arch = "wasm32"))]
use std::any::Any;
use std::rc::Rc;

/// An authored styling function's body.
type StyleFn = dyn Fn(&str) -> ExtensionResult<String>;
/// An authored highlighting function's body.
type HighlightFn = dyn Fn(&str, Option<&str>) -> ExtensionResult<Vec<String>>;

/// A function an author wrote or the host supplied, with the host resource behind a supplied
/// one; clones share the function.
pub(crate) struct Supplied<F: ?Sized> {
    /// The function.
    pub(crate) function: Rc<F>,
    /// The host resource behind a supplied function.
    #[cfg(any(test, target_arch = "wasm32"))]
    host: Option<Rc<dyn Any>>,
}
impl<F: ?Sized> Clone for Supplied<F> {
    fn clone(&self) -> Self {
        Self {
            function: self.function.clone(),
            #[cfg(any(test, target_arch = "wasm32"))]
            host: self.host.clone(),
        }
    }
}
impl<F: ?Sized> Supplied<F> {
    /// An authored function.
    pub(crate) fn authored(function: Rc<F>) -> Self {
        Self {
            function,
            #[cfg(any(test, target_arch = "wasm32"))]
            host: None,
        }
    }
    /// A function the host supplied, kept as `host`.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn lent<P: 'static>(function: Rc<F>, host: Rc<P>) -> Self {
        Self {
            function,
            host: Some(host),
        }
    }
    /// The host resource behind this function, when the host supplied it as a `P`.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn host<P: 'static>(&self) -> Option<Rc<P>> {
        self.host.clone()?.downcast().ok()
    }
}

/// Styles text, for example with a color; the host calls it while it renders.
///
/// Clones share one function. A style the host supplied goes back to the host unchanged.
#[derive(Clone)]
pub struct Style(pub(crate) Supplied<StyleFn>);
impl Style {
    /// Wraps an authored styling function.
    #[must_use]
    pub fn new(apply: impl Fn(&str) -> ExtensionResult<String> + 'static) -> Self {
        Self(Supplied::authored(Rc::new(apply)))
    }
    /// Styles `text`.
    ///
    /// # Errors
    /// Returns the function's failure.
    pub fn apply(&self, text: &str) -> ExtensionResult<String> {
        (self.0.function)(text)
    }
}

/// Highlights a code block into rows, given its language when the block names one.
#[derive(Clone)]
pub struct Highlight(pub(crate) Supplied<HighlightFn>);
impl Highlight {
    /// Wraps an authored highlighting function.
    #[must_use]
    pub fn new(
        highlight: impl Fn(&str, Option<&str>) -> ExtensionResult<Vec<String>> + 'static,
    ) -> Self {
        Self(Supplied::authored(Rc::new(highlight)))
    }
    /// Highlights `code`.
    ///
    /// # Errors
    /// Returns the function's failure.
    pub fn highlight(&self, code: &str, lang: Option<&str>) -> ExtensionResult<Vec<String>> {
        (self.0.function)(code, lang)
    }
}

/// Styles of the Markdown elements.
#[derive(Clone)]
pub struct MarkdownTheme {
    /// Headings.
    pub heading: Style,
    /// Link labels.
    pub link: Style,
    /// Printed link destinations.
    pub link_url: Style,
    /// Inline code.
    pub code: Style,
    /// Code block rows without highlighting.
    pub code_block: Style,
    /// Code fences.
    pub code_block_border: Style,
    /// Quoted text.
    pub quote: Style,
    /// Quote borders.
    pub quote_border: Style,
    /// Horizontal rules.
    pub hr: Style,
    /// List markers.
    pub list_bullet: Style,
    /// Bold text.
    pub bold: Style,
    /// Italic text.
    pub italic: Style,
    /// Struck-through text.
    pub strikethrough: Style,
    /// Underlined text.
    pub underline: Style,
    /// Highlights code blocks; absent leaves them to `code_block`.
    pub highlight_code: Option<Highlight>,
    /// Prefix of each code block row; absent selects two spaces.
    pub code_block_indent: Option<String>,
}

/// A text decoration of Markdown's base style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextDecoration {
    /// Bold text.
    Bold,
    /// Italic text.
    Italic,
    /// Struck-through text.
    Strikethrough,
    /// Underlined text.
    Underline,
}

/// Base style of Markdown text, applied unless an element styles it.
#[derive(Clone, Default)]
pub struct DefaultTextStyle {
    /// Foreground.
    pub color: Option<Style>,
    /// Row background.
    pub bg_color: Option<Style>,
    /// Decorations, applied in bold, italic, strikethrough, underline order.
    pub decorations: Vec<TextDecoration>,
}

/// Requests to the host widget behind a facade.
pub(crate) trait WidgetPort {
    /// Renders rows at `width`.
    fn render(&self, width: u32) -> ExtensionResult<Vec<String>>;
    /// Drops cached rows, the children's included.
    fn invalidate(&self) -> ExtensionResult<()>;
    /// Replaces the text.
    fn set_text(&self, text: &str) -> ExtensionResult<()>;
    /// Replaces or removes the row background.
    fn set_background(&self, style: Option<&Style>) -> ExtensionResult<()>;
    /// Replaces the row count.
    fn set_lines(&self, lines: u32) -> ExtensionResult<()>;
    /// Appends a child.
    fn add_child(&self, child: &Component) -> ExtensionResult<()>;
    /// Removes the first occurrence of a child.
    fn remove_child(&self, child: &Component) -> ExtensionResult<()>;
    /// Removes every child.
    fn clear(&self) -> ExtensionResult<()>;
    /// Delivers input.
    fn handle_input(&self, data: &str) -> ExtensionResult<()>;
    /// Filters a selection list.
    fn set_filter(&self, filter: &str) -> ExtensionResult<()>;
    /// Selects a row of a selection list.
    fn set_selected_index(&self, index: i64) -> ExtensionResult<()>;
    /// The selected choice of a selection list.
    fn get_selected_item(&self) -> ExtensionResult<Option<super::SelectItem>>;
    /// Replaces or removes a selection list's confirmation handler.
    fn set_on_select(&self, handler: Option<super::SelectHandler>) -> ExtensionResult<()>;
    /// Replaces or removes a selection list's cancellation handler.
    fn set_on_cancel(&self, handler: Option<super::WidgetNotice>) -> ExtensionResult<()>;
    /// Replaces or removes a selection list's movement handler.
    fn set_on_selection_change(&self, handler: Option<super::SelectHandler>)
    -> ExtensionResult<()>;
    /// Replaces or removes a loader's abort handler.
    fn set_on_abort(&self, handler: Option<super::WidgetNotice>) -> ExtensionResult<()>;
    /// Replaces a setting's current value.
    fn update_value(&self, id: &str, value: &str) -> ExtensionResult<()>;
    /// A loader's signal.
    fn signal(&self) -> ExtensionResult<crate::AbortSignal>;
    /// Stops a loader's animation.
    fn dispose(&self) -> ExtensionResult<()>;
}

/// One host widget; clones share it.
#[derive(Clone)]
pub(crate) struct Widget {
    /// Its requests.
    pub(super) port: Rc<dyn WidgetPort>,
    /// The same owner, for the adapter to find its host resource.
    #[cfg(any(test, target_arch = "wasm32"))]
    owner: Rc<dyn Any>,
}
impl Widget {
    /// Retains a host widget.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn new<P: WidgetPort + 'static>(owner: Rc<P>) -> Self {
        Self {
            port: owner.clone(),
            owner,
        }
    }
    /// The owner, when it is a `P`.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn owner<P: 'static>(&self) -> Option<Rc<P>> {
        self.owner.clone().downcast().ok()
    }
}

/// Builds host widgets.
pub(crate) trait WidgetHost {
    /// Creates one shared host overlay placement owner.
    fn overlay_options(
        &self,
        options: super::OverlayOptions,
    ) -> ExtensionResult<super::OverlayOptionsHandle>;
    /// The host's live theme, independent of UI availability.
    fn theme(&self) -> ExtensionResult<crate::Theme>;
    /// Wrapped text between padding.
    fn text(
        &self,
        text: &str,
        padding: (u32, u32),
        background: Option<&Style>,
    ) -> ExtensionResult<Widget>;
    /// Children between padding.
    fn r#box(&self, padding: (u32, u32), background: Option<&Style>) -> ExtensionResult<Widget>;
    /// Children in order.
    fn container(&self) -> ExtensionResult<Widget>;
    /// Blank rows.
    fn spacer(&self, lines: u32) -> ExtensionResult<Widget>;
    /// Markdown between padding.
    fn markdown(
        &self,
        text: &str,
        padding: (u32, u32),
        theme: &MarkdownTheme,
        style: Option<&DefaultTextStyle>,
    ) -> ExtensionResult<Widget>;
    /// A full-width rule.
    fn dynamic_border(&self, color: Option<&Style>) -> ExtensionResult<Widget>;
    /// The live theme's Markdown styles.
    fn markdown_theme(&self) -> ExtensionResult<MarkdownTheme>;
    /// A selection list.
    fn select_list(
        &self,
        items: &[super::SelectItem],
        max_visible: u32,
        theme: &super::SelectListTheme,
        layout: &super::SelectListLayoutOptions,
    ) -> ExtensionResult<Widget>;
    /// A settings list.
    fn settings_list(
        &self,
        items: (Vec<super::SettingItem>, u32),
        theme: &super::SettingsListTheme,
        callbacks: (super::SettingChange, super::WidgetNotice),
        options: super::SettingsListOptions,
    ) -> ExtensionResult<Widget>;
    /// A bordered loader.
    fn bordered_loader(
        &self,
        tui: &super::TUI,
        theme: &crate::Theme,
        message: &str,
        cancellable: Option<bool>,
    ) -> ExtensionResult<Widget>;
    /// The live theme's selection list styles.
    fn select_list_theme(&self) -> ExtensionResult<super::SelectListTheme>;
    /// The live theme's settings list styles.
    fn settings_list_theme(&self) -> ExtensionResult<super::SettingsListTheme>;
    /// A manager of the application's keybindings.
    fn keybindings(
        &self,
        user_bindings: super::KeybindingsConfig,
        config_path: Option<&str>,
    ) -> ExtensionResult<super::KeybindingsManager>;
    /// A manager of the application's keybindings loaded from an agent directory.
    fn create_keybindings(
        &self,
        agent_dir: Option<&str>,
    ) -> ExtensionResult<super::KeybindingsManager>;
    /// A manager of the supplied keybindings.
    fn keybindings_with_definitions(
        &self,
        definitions: super::KeybindingDefinitions,
        user_bindings: super::KeybindingsConfig,
    ) -> ExtensionResult<super::KeybindingsManager>;
    /// The keybindings the host's terminal uses.
    fn get_keybindings(&self) -> ExtensionResult<super::KeybindingsManager>;
    /// The keys of a keybinding, joined with `/`.
    fn key_text(&self, keybinding: &str) -> ExtensionResult<String>;
    /// A keybinding's hint, styled with the live theme.
    fn key_hint(&self, keybinding: &str, description: &str) -> ExtensionResult<String>;
    /// A literal key's hint, styled with the live theme.
    fn raw_key_hint(&self, key: &str, description: &str) -> ExtensionResult<String>;
    /// The live theme's editor styles.
    fn editor_theme(&self) -> ExtensionResult<super::EditorTheme>;
    /// The toolkit editor on a lent terminal.
    fn editor(
        &self,
        tui: &super::TUI,
        theme: &super::EditorTheme,
        options: super::EditorOptions,
    ) -> ExtensionResult<(Widget, Rc<dyn super::EditorPort>)>;
    /// The application's custom editor on a lent terminal.
    fn custom_editor(
        &self,
        tui: &super::TUI,
        theme: &super::EditorTheme,
        keybindings: &super::KeybindingsManager,
        options: super::EditorOptions,
    ) -> ExtensionResult<CustomEditorParts>;
    /// Builds an editor with an editor factory another extension set, kept as `factory`.
    #[cfg(any(test, target_arch = "wasm32"))]
    fn foreign_editor(
        &self,
        factory: &dyn std::any::Any,
        tui: &super::TUI,
        theme: &super::EditorTheme,
        keybindings: &super::KeybindingsManager,
    ) -> ExtensionResult<crate::EditorFactoryOutcome>;
}

/// A custom editor's widget, editor requests and custom editor requests.
pub(crate) type CustomEditorParts = (
    Widget,
    Rc<dyn super::EditorPort>,
    Rc<dyn super::CustomEditorPort>,
);

/// The host that builds widgets.
///
/// # Errors
/// Returns a message when no extension host runs this code.
pub(crate) fn host() -> ExtensionResult<Rc<dyn WidgetHost>> {
    #[cfg(any(test, target_arch = "wasm32"))]
    return crate::component_adapter::widget_host();
    #[cfg(not(any(test, target_arch = "wasm32")))]
    Err("host widgets need an extension component host".to_owned())
}

/// A host widget seen as a component.
pub(super) struct Hosted(pub(super) Widget);
impl ComponentView for Hosted {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.0.port.render(width)
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.0.port.handle_input(data)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.0.port.invalidate()
    }
}

/// Declares a facade over one host widget with the operations every widget has.
macro_rules! facade {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone)]
        pub struct $name(Widget);
        impl $name {
            /// Renders rows at `width`; the host may call authored functions first.
            ///
            /// # Errors
            /// Returns the host's or an authored function's failure.
            pub fn render(&self, width: u32) -> ExtensionResult<Vec<String>> {
                self.0.port.render(width)
            }
            /// Drops cached rows.
            ///
            /// # Errors
            /// Returns the host's failure.
            pub fn invalidate(&self) -> ExtensionResult<()> {
                self.0.port.invalidate()
            }
        }
        impl From<&$name> for Component {
            fn from(widget: &$name) -> Self {
                Component::hosted(Hosted(widget.0.clone()), widget.0.clone())
            }
        }
    };
}

/// Declares the child operations of a facade whose widget holds children.
macro_rules! children {
    ($name:ident) => {
        impl $name {
            /// Appends `child`; the same component may be added more than once.
            ///
            /// # Errors
            /// Returns the host's failure.
            pub fn add_child(&self, child: &Component) -> ExtensionResult<()> {
                self.0.port.add_child(child)
            }
            /// Removes the first occurrence of `child`; an absent child changes nothing.
            ///
            /// # Errors
            /// Returns the host's failure.
            pub fn remove_child(&self, child: &Component) -> ExtensionResult<()> {
                self.0.port.remove_child(child)
            }
            /// Removes every child.
            ///
            /// # Errors
            /// Returns the host's failure.
            pub fn clear(&self) -> ExtensionResult<()> {
                self.0.port.clear()
            }
        }
    };
}

facade! {
    /// Word-wrapped text between horizontal and vertical padding, over an optional row
    /// background.
    Text
}
impl Text {
    /// Builds the text widget.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn new(
        text: &str,
        padding_x: u32,
        padding_y: u32,
        custom_bg_fn: Option<&Style>,
    ) -> ExtensionResult<Self> {
        let host = host()?;
        host.text(text, (padding_x, padding_y), custom_bg_fn)
            .map(Self)
    }
    /// Replaces the text.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn set_text(&self, text: &str) -> ExtensionResult<()> {
        self.0.port.set_text(text)
    }
    /// Replaces or removes the row background.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn set_custom_bg_fn(&self, custom_bg_fn: Option<&Style>) -> ExtensionResult<()> {
        self.0.port.set_background(custom_bg_fn)
    }
}

facade! {
    /// Children between horizontal and vertical padding, over an optional row background.
    Box
}
children!(Box);
impl Box {
    /// Builds the box.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn new(padding_x: u32, padding_y: u32, bg_fn: Option<&Style>) -> ExtensionResult<Self> {
        let host = host()?;
        host.r#box((padding_x, padding_y), bg_fn).map(Self)
    }
    /// Replaces or removes the row background.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn set_bg_fn(&self, bg_fn: Option<&Style>) -> ExtensionResult<()> {
        self.0.port.set_background(bg_fn)
    }
}

facade! {
    /// Children rendered one after another.
    Container
}
children!(Container);
impl Container {
    /// Builds an empty container.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn new() -> ExtensionResult<Self> {
        host()?.container().map(Self)
    }
}

facade! {
    /// Blank rows.
    Spacer
}
impl Spacer {
    /// Builds `lines` blank rows.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn new(lines: u32) -> ExtensionResult<Self> {
        host()?.spacer(lines).map(Self)
    }
    /// Replaces the row count.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn set_lines(&self, lines: u32) -> ExtensionResult<()> {
        self.0.port.set_lines(lines)
    }
}

facade! {
    /// Markdown laid out for the host terminal between horizontal and vertical padding.
    Markdown
}
impl Markdown {
    /// Builds the Markdown widget.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn new(
        text: &str,
        padding_x: u32,
        padding_y: u32,
        theme: &MarkdownTheme,
        default_text_style: Option<&DefaultTextStyle>,
    ) -> ExtensionResult<Self> {
        let host = host()?;
        host.markdown(text, (padding_x, padding_y), theme, default_text_style)
            .map(Self)
    }
    /// Replaces the text.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn set_text(&self, text: &str) -> ExtensionResult<()> {
        self.0.port.set_text(text)
    }
}

facade! {
    /// A rule across the full width.
    DynamicBorder
}
impl DynamicBorder {
    /// Builds the rule, painted by `color` or by the live theme's border color.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn new(color: Option<&Style>) -> ExtensionResult<Self> {
        let host = host()?;
        host.dynamic_border(color).map(Self)
    }
}

/// Markdown styles that read the live theme when they run, with the host's code highlighting.
///
/// # Errors
/// Returns the host's failure.
pub fn get_markdown_theme() -> ExtensionResult<MarkdownTheme> {
    host()?.markdown_theme()
}
