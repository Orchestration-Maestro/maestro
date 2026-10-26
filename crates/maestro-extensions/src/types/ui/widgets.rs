//! The host capability that builds the extension widgets whose look depends on the host.
use maestro_tui::{
    Markdown, MarkdownOptions, MarkdownTheme, editor_component::BorderColor, tui::ComponentHandle,
};
use std::rc::Rc;

/// Builds the widgets whose look depends on the host: Markdown laid out for its terminal, the
/// live theme's Markdown styles and the application's dynamic border.
///
/// Every mode supplies one, so building a widget never needs an interactive UI; only showing
/// one goes through the UI context. Text, boxes, containers and spacers need nothing from the
/// host and are built from the toolkit directly.
pub trait ExtensionWidgets {
    /// Markdown laid out with the host terminal's capabilities.
    fn markdown(
        &self,
        text: String,
        options: MarkdownOptions,
        theme: Rc<MarkdownTheme>,
    ) -> Rc<Markdown>;
    /// Markdown styles that read the live theme when they run, with the host's code
    /// highlighting.
    fn markdown_theme(&self) -> MarkdownTheme;
    /// A rule across the full width, painted by `color` or by the live border color.
    fn dynamic_border(&self, color: Option<BorderColor>) -> ComponentHandle;
}
