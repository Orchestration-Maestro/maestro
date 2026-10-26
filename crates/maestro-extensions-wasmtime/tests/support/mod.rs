//! Live domain bindings shared by component and typed-request witnesses.
#![cfg(test)]
use maestro_extensions::{
    ExtensionContextActions, ExtensionExecution, ExtensionRunner, Local, types::ProviderActions,
};
use std::rc::Rc;

/// Binds a prompt reader without replacing command-specific operations.
pub fn bind_prompt(runner: &ExtensionRunner<Local>, prompt: &'static str) {
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_system_prompt: Rc::new(move || Ok(prompt.into())),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
}

/// Toolkit widgets over a live theme, for a terminal without image support, with Syntect
/// highlighting; the rule paints like the application's rule.
struct ToolkitWidgets(maestro_theme::LiveTheme);

impl maestro_extensions::ExtensionWidgets for ToolkitWidgets {
    fn markdown(
        &self,
        text: String,
        options: maestro_tui::MarkdownOptions,
        theme: Rc<maestro_tui::MarkdownTheme>,
    ) -> Rc<maestro_tui::Markdown> {
        let terminal = maestro_tui::TerminalImage::new(|_| None, || 1);
        Rc::new(maestro_tui::Markdown::new(text, options, theme, terminal))
    }
    fn markdown_theme(&self) -> maestro_tui::MarkdownTheme {
        let highlighter = Rc::new(maestro_theme::SyntectHighlighter::new().unwrap());
        maestro_theme::get_markdown_theme(self.0.clone(), highlighter, true)
    }
    fn dynamic_border(
        &self,
        color: Option<maestro_tui::editor_component::BorderColor>,
    ) -> maestro_extensions::ComponentHandle {
        Rc::new(Rule(color, self.0.clone()))
    }
    fn select_list_theme(&self) -> maestro_tui::SelectListTheme {
        maestro_theme::get_select_list_theme(self.0.clone())
    }
    fn settings_list_theme(&self) -> maestro_tui::SettingsListTheme {
        maestro_theme::get_settings_list_theme(self.0.clone())
    }
    fn bordered_loader(
        &self,
        tui: &maestro_tui::TUI,
        theme: Rc<maestro_theme::Theme>,
        message: String,
        cancellable: bool,
    ) -> Result<Rc<dyn maestro_extensions::widgets::ExtensionLoader>, String> {
        let color =
            move |key: maestro_theme::ThemeColor| -> maestro_tui::editor_component::BorderColor {
                let theme = theme.clone();
                Rc::new(move |text: &str| maestro_theme::paint(Some(&theme), &key, text))
            };
        let loader = maestro_tui::Loader::new(
            tui.clone(),
            color(maestro_theme::ThemeColor::Accent),
            color(maestro_theme::ThemeColor::Muted),
            Some(message),
            None,
        );
        let loader = Rc::new(maestro_tui::CancellableLoader::new(loader));
        let rows = maestro_tui::Container::new();
        rows.add_child(loader.clone());
        Ok(Rc::new(Loader {
            rows,
            inner: loader,
            cancellable,
            signal: maestro_tui::Cancellation::new(),
        }))
    }
}

/// A loader between no borders: input cancels it only when it is cancellable.
struct Loader {
    /// The rows it renders.
    rows: maestro_tui::Container,
    /// The cancellable loader among them.
    inner: Rc<maestro_tui::CancellableLoader>,
    /// Whether input can cancel it.
    cancellable: bool,
    /// The signal of a loader input cannot cancel.
    signal: maestro_tui::Cancellation,
}

impl maestro_tui::Component for Loader {
    fn render(&self, width: usize) -> Vec<String> {
        self.rows.render(width)
    }
    fn input_handler(&self) -> Option<&dyn maestro_extensions::InputHandler> {
        Some(self)
    }
}

impl maestro_extensions::InputHandler for Loader {
    fn handle_input(&self, data: &str) {
        if self.cancellable {
            maestro_extensions::InputHandler::handle_input(&*self.inner, data);
        }
    }
}

impl maestro_extensions::widgets::ExtensionLoader for Loader {
    fn signal(&self) -> maestro_tui::Cancellation {
        if self.cancellable {
            self.inner.signal().clone()
        } else {
            self.signal.clone()
        }
    }
    fn set_on_abort(&self, on_abort: Option<Rc<dyn Fn()>>) {
        self.inner.set_on_abort(on_abort);
    }
    fn dispose(&self) {
        self.inner.dispose();
    }
    fn rows(&self) -> &maestro_tui::Container {
        &self.rows
    }
}

/// A rule across the width, painted by its color or the live border color.
struct Rule(
    Option<maestro_tui::editor_component::BorderColor>,
    maestro_theme::LiveTheme,
);

impl maestro_tui::Component for Rule {
    fn render(&self, width: usize) -> Vec<String> {
        let rule = "─".repeat(width.max(1));
        vec![match &self.0 {
            Some(color) => color(&rule),
            None => maestro_theme::paint(
                self.1.get().ok().as_deref(),
                &maestro_theme::ThemeColor::Border,
                &rule,
            ),
        }]
    }
}

/// Toolkit widgets over `theme`.
pub fn widgets_over(
    theme: maestro_theme::LiveTheme,
) -> Rc<dyn maestro_extensions::ExtensionWidgets> {
    Rc::new(ToolkitWidgets(theme))
}

/// Toolkit widgets over an unpublished live theme.
pub fn widgets() -> Rc<dyn maestro_extensions::ExtensionWidgets> {
    widgets_over(maestro_theme::LiveTheme::default())
}
