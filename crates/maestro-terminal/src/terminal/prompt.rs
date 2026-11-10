//! Shell semantic phases and prompt-location policy.

use super::Terminal;
use crate::{
    osc::semantic_prompt::{Action, Command, Option as PromptOption, OptionValue, PromptKind},
    page::{cell::SemanticContent as CellContent, row::SemanticPrompt},
    page_list::IncreaseCapacityError,
    screen::{InputTermination, SemanticContent},
    screens::Key,
};

impl Terminal {
    /// Apply a shell prompt action and its independently decoded options.
    ///
    /// # Errors
    /// Returns the delegated capacity refusal when a fresh line needs scrolling.
    pub fn semantic_prompt(&mut self, cmd: &Command) -> Result<(), IncreaseCapacityError> {
        match cmd.action {
            Action::FreshLine => self.semantic_prompt_fresh_line()?,
            Action::FreshLineNewPrompt | Action::NewCommand => {
                self.semantic_prompt_fresh_line()?;
                self.start_semantic_prompt(cmd);
                if let Some(OptionValue::Redraw(value)) = cmd.read_option(PromptOption::Redraw) {
                    self.flags.shell_redraws_prompt = value;
                }
            }
            Action::PromptStart => self.start_semantic_prompt(cmd),
            Action::EndPromptStartInput => {
                self.screens
                    .active_mut()
                    .cursor_set_semantic_content(SemanticContent::Input(
                        InputTermination::ClearExplicit,
                    ));
            }
            Action::EndPromptStartInputTerminateEol => self
                .screens
                .active_mut()
                .cursor_set_semantic_content(SemanticContent::Input(InputTermination::ClearEol)),
            Action::EndInputStartOutput => {
                let screen = self.screens.active_mut();
                screen.cursor_set_semantic_content(SemanticContent::Output);
                if screen.cursor.x == 0
                    && let Some(pin) = screen.pages.tracked_pin(screen.cursor.page_pin).copied()
                    && let Some((row, _)) = pin.row_and_cell_mut(&mut screen.pages)
                {
                    row.set_semantic_prompt(SemanticPrompt::None);
                }
            }
            Action::EndCommand => self
                .screens
                .active_mut()
                .cursor_set_semantic_content(SemanticContent::Output),
        }
        Ok(())
    }

    /// Begin a prompt with its first valid kind option or the initial kind.
    fn start_semantic_prompt(&mut self, cmd: &Command) {
        let kind = match cmd.read_option(PromptOption::PromptKind) {
            Some(OptionValue::PromptKind(kind)) => kind,
            _ => PromptKind::Initial,
        };
        self.screens
            .active_mut()
            .cursor_set_semantic_content(SemanticContent::Prompt(kind));
    }

    /// Begin a line unless already at the applicable left boundary.
    fn semantic_prompt_fresh_line(&mut self) -> Result<(), IncreaseCapacityError> {
        let x = self.screens.active().cursor.x;
        let left = if x < self.scrolling_region.left {
            0
        } else {
            self.scrolling_region.left
        };
        if x != left {
            self.carriage_return();
            self.index()?;
        }
        Ok(())
    }

    /// Whether shell semantics identify the primary cursor as prompt or input.
    #[must_use]
    pub fn cursor_is_at_prompt(&self) -> bool {
        if self.screens.active_key() == Key::Alternate {
            return false;
        }
        let screen = self.screens.active();
        let prompt_row = screen
            .pages
            .tracked_pin(screen.cursor.page_pin)
            .and_then(|pin| pin.row_and_cell(&screen.pages))
            .is_some_and(|(row, _)| row.semantic_prompt() != SemanticPrompt::None);
        prompt_row
            || matches!(
                screen.cursor.semantic_content,
                CellContent::Prompt | CellContent::Input
            )
    }
}
