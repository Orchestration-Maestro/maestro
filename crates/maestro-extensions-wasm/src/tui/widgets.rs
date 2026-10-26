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

/// Styles text, for example with a color; the host calls it while it renders.
///
/// Clones share one function. A style the host supplied goes back to the host unchanged.
#[derive(Clone)]
pub struct Style {
    /// The function.
    apply: Rc<StyleFn>,
    /// The host resource behind a supplied style.
    #[cfg(any(test, target_arch = "wasm32"))]
    host: Option<Rc<dyn Any>>,
}
impl Style {
    /// Wraps an authored styling function.
    #[must_use]
    pub fn new(apply: impl Fn(&str) -> ExtensionResult<String> + 'static) -> Self {
        Self {
            apply: Rc::new(apply),
            #[cfg(any(test, target_arch = "wasm32"))]
            host: None,
        }
    }
    /// Styles `text`.
    ///
    /// # Errors
    /// Returns the function's failure.
    pub fn apply(&self, text: &str) -> ExtensionResult<String> {
        (self.apply)(text)
    }
    /// A style the host supplied, kept as `host`.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn supplied<P: 'static>(apply: Rc<StyleFn>, host: Rc<P>) -> Self {
        Self {
            apply,
            host: Some(host),
        }
    }
    /// The host resource behind this style, when the host supplied it as a `P`.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn host<P: 'static>(&self) -> Option<Rc<P>> {
        self.host.clone()?.downcast().ok()
    }
}

/// Highlights a code block into rows, given its language when the block names one.
#[derive(Clone)]
pub struct Highlight {
    /// The function.
    highlight: Rc<HighlightFn>,
    /// The host resource behind supplied highlighting.
    #[cfg(any(test, target_arch = "wasm32"))]
    host: Option<Rc<dyn Any>>,
}
impl Highlight {
    /// Wraps an authored highlighting function.
    #[must_use]
    pub fn new(
        highlight: impl Fn(&str, Option<&str>) -> ExtensionResult<Vec<String>> + 'static,
    ) -> Self {
        Self {
            highlight: Rc::new(highlight),
            #[cfg(any(test, target_arch = "wasm32"))]
            host: None,
        }
    }
    /// Highlights `code`.
    ///
    /// # Errors
    /// Returns the function's failure.
    pub fn highlight(&self, code: &str, lang: Option<&str>) -> ExtensionResult<Vec<String>> {
        (self.highlight)(code, lang)
    }
    /// Highlighting the host supplied, kept as `host`.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn supplied<P: 'static>(highlight: Rc<HighlightFn>, host: Rc<P>) -> Self {
        Self {
            highlight,
            host: Some(host),
        }
    }
    /// The host resource behind this highlighting, when the host supplied it as a `P`.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn host<P: 'static>(&self) -> Option<Rc<P>> {
        self.host.clone()?.downcast().ok()
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
}

/// One host widget; clones share it.
#[derive(Clone)]
pub(crate) struct Widget {
    /// Its requests.
    port: Rc<dyn WidgetPort>,
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
}

/// The host that builds widgets.
///
/// # Errors
/// Returns a message when no extension host runs this code.
fn host() -> ExtensionResult<Rc<dyn WidgetHost>> {
    #[cfg(any(test, target_arch = "wasm32"))]
    return crate::component_adapter::widget_host();
    #[cfg(not(any(test, target_arch = "wasm32")))]
    Err("host widgets need an extension component host".to_owned())
}

/// A host widget seen as a component.
struct Hosted(Widget);
impl ComponentView for Hosted {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.0.port.render(width)
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
