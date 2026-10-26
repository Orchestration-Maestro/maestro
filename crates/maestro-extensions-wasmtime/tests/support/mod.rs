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
