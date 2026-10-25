//! Directional traversal of primary and continuation prompt rows.

use super::{Direction, PageList, Pin};
use crate::{Point, page::row::SemanticPrompt};

/// Owned prompt traversal state, borrowing rows only during advancement.
pub struct PromptIterator {
    /// Next row to inspect.
    current: Option<Pin>,
    /// Inclusive final row; the column does not constrain row traversal.
    limit: Option<Pin>,
    /// Direction through retained rows.
    direction: Direction,
}

impl PromptIterator {
    /// Return the first row of the next encountered prompt group.
    pub fn next(&mut self, list: &PageList) -> Option<Pin> {
        while let Some(pin) = self.current.take() {
            let role = pin.row_and_cell(list)?.0.semantic_prompt();
            let at_limit = self.at_limit(pin);
            self.current = if at_limit {
                None
            } else {
                self.advance(list, pin)
            };
            if role == SemanticPrompt::None {
                continue;
            }
            if at_limit {
                return Some(pin.left(pin.x.into()));
            }
            if self.direction == Direction::RightDown {
                self.skip_continuations(list, pin);
                return Some(pin.left(pin.x.into()));
            }
            if role == SemanticPrompt::Prompt {
                return Some(pin.left(pin.x.into()));
            }
            return Some(self.find_prompt_start(list, pin));
        }
        None
    }

    /// Test the inclusive row boundary in its checked page lifetime.
    fn at_limit(&self, pin: Pin) -> bool {
        self.limit
            .is_some_and(|limit| limit.node == pin.node && limit.y == pin.y)
    }

    /// Move one row while preserving the current column.
    fn advance(&self, list: &PageList, pin: Pin) -> Option<Pin> {
        match self.direction {
            Direction::LeftUp => pin.up(list, 1),
            Direction::RightDown => pin.down(list, 1),
        }
    }

    /// Consume the continuation suffix following a forward prompt match.
    fn skip_continuations(&mut self, list: &PageList, mut pin: Pin) {
        self.current = None;
        while let Some(next) = pin.down(list, 1) {
            let Some((row, _)) = next.row_and_cell(list) else {
                return;
            };
            if row.semantic_prompt() != SemanticPrompt::PromptContinuation {
                self.current = Some(next);
                return;
            }
            if self.at_limit(next) {
                return;
            }
            pin = next;
        }
    }

    /// Find the primary prompt or the beginning of an orphaned continuation.
    fn find_prompt_start(&mut self, list: &PageList, start: Pin) -> Pin {
        self.current = None;
        let mut end = start;
        while let Some(prior) = end.up(list, 1) {
            let Some((row, _)) = prior.row_and_cell(list) else {
                break;
            };
            let first = match row.semantic_prompt() {
                SemanticPrompt::None => end,
                _ => prior,
            };
            if self.at_limit(prior) {
                return first.left(first.x.into());
            }
            match row.semantic_prompt() {
                SemanticPrompt::None => {
                    self.current = Some(prior);
                    return end.left(end.x.into());
                }
                SemanticPrompt::Prompt => {
                    self.current = prior.up(list, 1);
                    return prior.left(prior.x.into());
                }
                SemanticPrompt::PromptContinuation => end = prior,
            }
        }
        end.left(end.x.into())
    }
}

impl Pin {
    /// Create an inclusive prompt cursor beginning at this retained position.
    #[must_use]
    pub fn prompt_iterator(self, direction: Direction, limit: Option<Self>) -> PromptIterator {
        PromptIterator {
            current: Some(self),
            limit,
            direction,
        }
    }
}

impl PageList {
    /// Traverse prompt groups inside the inclusive region endpoints.
    #[must_use]
    pub fn prompt_iterator(
        &self,
        direction: Direction,
        start: Point,
        end: Option<Point>,
    ) -> PromptIterator {
        let low = self.pin(start);
        let high = end.map_or_else(
            || self.get_bottom_right(start.tag()),
            |point| self.pin(point),
        );
        let (current, limit) = match direction {
            Direction::RightDown => (low, high),
            Direction::LeftUp => (high, low),
        };
        PromptIterator {
            current: current.filter(|_| limit.is_some()),
            limit,
            direction,
        }
    }
}
