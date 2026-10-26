//! The host capability that builds the extension widgets whose look depends on the host.
use maestro_theme::Theme;
use maestro_tui::{
    Cancellation, Component, Container, Markdown, MarkdownOptions, MarkdownTheme, SelectListTheme,
    SettingsListTheme, TUI, editor_component::BorderColor, tui::ComponentHandle,
};
use std::rc::Rc;

/// The application's bordered loader, as an extension holds it.
pub trait ExtensionLoader: Component {
    /// The signal input cancels; a loader that cannot be cancelled keeps its own.
    fn signal(&self) -> Cancellation;
    /// Replaces or removes the notification a cancellation runs.
    fn set_on_abort(&self, on_abort: Option<Rc<dyn Fn()>>);
    /// Stops the animation without cancelling the signal.
    fn dispose(&self);
    /// The rows the loader renders, which an extension may extend.
    fn rows(&self) -> &Container;
}

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
    /// Selection list styles that read the live theme when they run.
    fn select_list_theme(&self) -> SelectListTheme;
    /// Settings list styles that read the live theme, with the cursor colored now.
    fn settings_list_theme(&self) -> SettingsListTheme;
    /// The application's bordered loader on `tui`, colored by `theme`.
    ///
    /// # Errors
    /// Returns the theme failure of the loader's cancellation hint.
    fn bordered_loader(
        &self,
        tui: &TUI,
        theme: Rc<Theme>,
        message: String,
        cancellable: bool,
    ) -> Result<Rc<dyn ExtensionLoader>, String>;
}
