//! Collapsed and expanded skill invocation content.
use maestro_app::{ParsedSkillBlock, key_text};
use maestro_theme::{LiveTheme, ThemeBg, ThemeColor};
use maestro_tui::{Box as Panel, Component, MarkdownTheme, TerminalImage, Text};
use std::{cell::Cell, rc::Rc};

/// Skill block rendering with the application-owned parsed record.
///
/// The block's location and trailing user message belong to the caller and are not shown.
pub struct SkillInvocationMessageComponent {
    /// Padded skill body owner.
    panel: Panel,
    /// Current expansion flag.
    expanded: Cell<bool>,
    /// Owned parsed skill record.
    skill_block: ParsedSkillBlock,
    /// Shared rendering context.
    rendering: super::MessageRendering,
}
impl SkillInvocationMessageComponent {
    /// Builds the collapsed skill panel with supplied rendering state.
    #[must_use]
    pub fn new(
        skill_block: ParsedSkillBlock,
        theme: LiveTheme,
        markdown_theme: Rc<MarkdownTheme>,
        terminal_image: TerminalImage,
    ) -> Self {
        let panel = Panel::new(
            1,
            1,
            Some(super::background(theme.clone(), ThemeBg::CustomMessageBg)),
        );
        let component = Self {
            panel,
            expanded: Cell::new(false),
            skill_block,
            rendering: super::MessageRendering::new(theme, markdown_theme, terminal_image),
        };
        component.update_display();
        component
    }
    /// Rebuilds the panel, including repeated requests for the same state.
    pub fn set_expanded(&self, expanded: bool) {
        self.expanded.set(expanded);
        self.update_display();
    }
    /// Selects one of the two visible states from current theme and key registry values.
    fn update_display(&self) {
        self.panel.clear();
        if self.expanded.get() {
            let label = self
                .rendering
                .paint(&ThemeColor::CustomMessageLabel, "\x1b[1m[skill]\x1b[22m");
            self.panel.add_child(Rc::new(Text::new(label, 0, 0, None)));
            self.panel.add_child(self.rendering.markdown(
                format!(
                    "**{}**\n\n{}",
                    self.skill_block.name, self.skill_block.content
                ),
                0,
                Some(ThemeColor::CustomMessageText),
                false,
            ));
            return;
        }
        let line = self
            .rendering
            .paint(&ThemeColor::CustomMessageLabel, "\x1b[1m[skill]\x1b[22m ")
            + &self
                .rendering
                .paint(&ThemeColor::CustomMessageText, &self.skill_block.name)
            + &self.rendering.paint(
                &ThemeColor::Dim,
                &format!(" ({} to expand)", key_text("app.tools.expand")),
            );
        self.panel.add_child(Rc::new(Text::new(line, 0, 0, None)));
    }
}
component_children!(SkillInvocationMessageComponent, Panel, panel);
impl Component for SkillInvocationMessageComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.panel.render(width)
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.panel.try_render(width)
    }
    fn invalidate(&self) {
        self.panel.invalidate();
        self.update_display();
    }
}
