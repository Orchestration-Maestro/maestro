//! Expandable branch summary and the shared summary display owner.
use maestro_app::{BranchSummaryMessage, key_text};
use maestro_theme::{LiveTheme, ThemeBg, ThemeColor};
use maestro_tui::{Box, Component, MarkdownTheme, Spacer, TerminalImage, Text};
use std::{cell::Cell, rc::Rc};

/// Expandable branch summary with an editable padded panel.
pub struct BranchSummaryMessageComponent(SummaryDisplay);
impl BranchSummaryMessageComponent {
    /// Builds the collapsed branch panel with supplied rendering state.
    #[must_use]
    pub fn new(
        message: BranchSummaryMessage,
        theme: LiveTheme,
        markdown_theme: Rc<MarkdownTheme>,
        terminal_image: TerminalImage,
    ) -> Self {
        Self(SummaryDisplay::new(
            SummaryText {
                summary: message.summary,
                label: "[branch]",
                header: "**Branch Summary**\n\n".into(),
                collapsed: "Branch summary (".into(),
            },
            theme,
            markdown_theme,
            terminal_image,
        ))
    }
    /// Rebuilds the summary, including repeated requests for the same state.
    pub fn set_expanded(&self, expanded: bool) {
        self.0.set_expanded(expanded);
    }
}
/// Text retained by the shared summary display; identifiers are not rendered.
pub(super) struct SummaryText {
    /// Summary body.
    pub summary: String,
    /// Literal component label.
    pub label: &'static str,
    /// Markdown heading and its paragraph boundary.
    pub header: String,
    /// Text before the live expansion keys.
    pub collapsed: String,
}
/// Shared panel composition for branch and compaction summaries.
pub(super) struct SummaryDisplay {
    /// Existing child-array owner.
    pub panel: Box,
    /// Text prepared by the concrete message type.
    text: SummaryText,
    /// Whether to show the Markdown body.
    expanded: Cell<bool>,
    /// Shared rendering context.
    rendering: super::MessageRendering,
}
impl SummaryDisplay {
    /// Builds a collapsed panel without another rendering owner.
    pub fn new(
        text: SummaryText,
        theme: LiveTheme,
        markdown: Rc<MarkdownTheme>,
        terminal: TerminalImage,
    ) -> Self {
        let panel = Box::new(
            1,
            1,
            Some(super::background(theme.clone(), ThemeBg::CustomMessageBg)),
        );
        let display = Self {
            panel,
            text,
            expanded: Cell::new(false),
            rendering: super::MessageRendering::new(theme, markdown, terminal),
        };
        display.rebuild();
        display
    }
    /// Rebuilds even when the state does not change.
    pub fn set_expanded(&self, expanded: bool) {
        self.expanded.set(expanded);
        self.rebuild();
    }
    /// Rebuilds labels and hints from current theme and key registry values.
    fn rebuild(&self) {
        self.panel.clear();
        let color = |key, text: &str| self.rendering.paint(&key, text);
        let label = color(
            ThemeColor::CustomMessageLabel,
            &format!("\x1b[1m{}\x1b[22m", self.text.label),
        );
        self.panel.add_child(Rc::new(Text::new(label, 0, 0, None)));
        self.panel.add_child(Rc::new(Spacer::new(1)));
        if self.expanded.get() {
            self.panel.add_child(self.rendering.markdown(
                format!("{}{}", self.text.header, self.text.summary),
                0,
                Some(ThemeColor::CustomMessageText),
                false,
            ));
        } else {
            let hint = color(ThemeColor::CustomMessageText, &self.text.collapsed)
                + &color(ThemeColor::Dim, &key_text("app.tools.expand"))
                + &color(ThemeColor::CustomMessageText, " to expand)");
            self.panel.add_child(Rc::new(Text::new(hint, 0, 0, None)));
        }
    }
    /// Invalidates existing children before restoring the current display.
    pub fn invalidate(&self) {
        self.panel.invalidate();
        self.rebuild();
    }
}
message_children!(BranchSummaryMessageComponent, Box, 0.panel);
impl Component for BranchSummaryMessageComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.0.panel.render(width)
    }
    fn invalidate(&self) {
        self.0.invalidate();
    }
}
