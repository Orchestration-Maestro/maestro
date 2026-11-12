//! Expandable compaction summary with a constructor-formatted token count.
use super::branch_summary_message::{SummaryDisplay, SummaryText};
use maestro_app::{CompactionSummaryMessage, LocaleNumberFormat};
use maestro_theme::LiveTheme;
use maestro_tui::{Box, Component, MarkdownTheme, TerminalImage};
use std::{io, rc::Rc};

/// An editable compaction panel retaining its formatted token count.
pub struct CompactionSummaryMessageComponent(SummaryDisplay);
impl CompactionSummaryMessageComponent {
    /// Builds the collapsed panel with the supplied locale formatter.
    ///
    /// # Errors
    /// Returns the formatter's number conversion failure.
    pub fn new(
        message: CompactionSummaryMessage,
        theme: LiveTheme,
        markdown_theme: Rc<MarkdownTheme>,
        terminal_image: TerminalImage,
        numbers: &LocaleNumberFormat,
    ) -> io::Result<Self> {
        let title = format!(
            "Compacted from {} tokens",
            numbers.format(message.tokens_before)?
        );
        Ok(Self(SummaryDisplay::new(
            SummaryText {
                summary: message.summary,
                label: "[compaction]",
                header: format!("**{title}**\n\n"),
                collapsed: format!("{title} ("),
            },
            theme,
            markdown_theme,
            terminal_image,
        )))
    }
    /// Rebuilds the summary, including requests for the current state.
    pub fn set_expanded(&self, expanded: bool) {
        self.0.set_expanded(expanded);
    }
}
component_children!(CompactionSummaryMessageComponent, Box, 0.panel);
impl Component for CompactionSummaryMessageComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.0.panel.render(width)
    }
    fn invalidate(&self) {
        self.0.invalidate();
    }
}
