//! Semantic prompt transitions shared by direct terminal operations.

use super::Screen;
use crate::{
    osc::semantic_prompt::PromptKind,
    page::{cell::SemanticContent as CellContent, row::SemanticPrompt},
};

/// How input semantics end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputTermination {
    /// Keep input semantics until the next explicit shell phase.
    ClearExplicit,
    /// End input semantics on the next line transition.
    ClearEol,
}

/// Semantic content selected for subsequent screen writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticContent {
    /// Begin prompt text with the selected row role.
    Prompt(PromptKind),
    /// Begin command output.
    Output,
    /// Begin user input with the selected termination policy.
    Input(InputTermination),
}

impl Screen {
    /// Change subsequent cell semantics without marking existing grid content dirty.
    pub fn cursor_set_semantic_content(&mut self, content: SemanticContent) {
        self.cursor.semantic_content_clear_eol = false;
        match content {
            SemanticContent::Output => self.cursor.semantic_content = CellContent::Output,
            SemanticContent::Input(termination) => {
                self.cursor.semantic_content = CellContent::Input;
                self.cursor.semantic_content_clear_eol = termination == InputTermination::ClearEol;
            }
            SemanticContent::Prompt(kind) => {
                self.semantic_prompt.seen = true;
                self.cursor.semantic_content = CellContent::Prompt;
                let prompt = match kind {
                    PromptKind::Initial | PromptKind::Right => SemanticPrompt::Prompt,
                    PromptKind::Continuation | PromptKind::Secondary => {
                        SemanticPrompt::PromptContinuation
                    }
                };
                if let Some(pin) = self.pages.tracked_pin(self.cursor.page_pin).copied()
                    && let Some((row, _)) = pin.row_and_cell_mut(&mut self.pages)
                {
                    row.set_semantic_prompt(prompt);
                }
            }
        }
    }
}
