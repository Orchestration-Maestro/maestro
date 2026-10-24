//! Word navigation over retained cell atoms.
use super::{RetainedTextBuffer, TerminalTextPoint, TerminalWordMotion, TextClass};

impl RetainedTextBuffer {
    /// Resolve a cell into the requested word motion.
    pub(in crate::panes::display) fn word_motion(
        &self,
        row: u32,
        col: u16,
        motion: TerminalWordMotion,
    ) -> Option<TerminalTextPoint> {
        let current = self.atoms.iter().position(|atom| {
            atom.point
                .is_some_and(|point| point.row == row && col >= point.col && col <= atom.end_col)
        })?;
        match motion {
            TerminalWordMotion::NextStart => self.next_word_start(current, false),
            TerminalWordMotion::PreviousStart => self.previous_word_start(current, false),
            TerminalWordMotion::NextEnd => self.next_word_end(current, false),
            TerminalWordMotion::NextBigStart => self.next_word_start(current, true),
            TerminalWordMotion::PreviousBigStart => self.previous_word_start(current, true),
            TerminalWordMotion::NextBigEnd => self.next_word_end(current, true),
        }
    }

    /// Skip the current word and whitespace to the next addressable run.
    fn next_word_start(&self, current: usize, big: bool) -> Option<TerminalTextPoint> {
        let class = self.atoms.get(current)?.class;
        let mut next = current.saturating_add(1);
        if class != TextClass::Whitespace {
            while self
                .atoms
                .get(next)
                .is_some_and(|atom| same_word(class, atom.class, big))
            {
                next += 1;
            }
        }
        while self
            .atoms
            .get(next)
            .is_some_and(|atom| atom.class == TextClass::Whitespace)
        {
            next += 1;
        }
        self.next_point(next)
    }

    /// Find the beginning of the preceding class or whitespace-delimited run.
    fn previous_word_start(&self, current: usize, big: bool) -> Option<TerminalTextPoint> {
        let mut previous = current.checked_sub(1)?;
        while self
            .atoms
            .get(previous)
            .is_some_and(|atom| atom.class == TextClass::Whitespace)
        {
            previous = previous.checked_sub(1)?;
        }
        let class = self.atoms.get(previous)?.class;
        while previous > 0
            && self
                .atoms
                .get(previous - 1)
                .is_some_and(|atom| same_word(class, atom.class, big))
        {
            previous -= 1;
        }
        self.previous_point(previous)
    }

    /// Find the end of the current or next word under the selected classification.
    fn next_word_end(&self, current: usize, big: bool) -> Option<TerminalTextPoint> {
        let mut next = current.saturating_add(1);
        while self
            .atoms
            .get(next)
            .is_some_and(|atom| atom.class == TextClass::Whitespace)
        {
            next += 1;
        }
        let class = self.atoms.get(next)?.class;
        while self
            .atoms
            .get(next + 1)
            .is_some_and(|atom| same_word(class, atom.class, big))
        {
            next += 1;
        }
        self.previous_point(next)
    }

    /// Find the next addressable cell.
    fn next_point(&self, mut index: usize) -> Option<TerminalTextPoint> {
        while let Some(atom) = self.atoms.get(index) {
            if let Some(point) = atom.point {
                return Some(point);
            }
            index += 1;
        }
        None
    }

    /// Find the previous addressable cell.
    fn previous_point(&self, mut index: usize) -> Option<TerminalTextPoint> {
        loop {
            if let Some(point) = self.atoms.get(index)?.point {
                return Some(point);
            }
            index = index.checked_sub(1)?;
        }
    }

    /// Whether a point is the final addressable atom, including wide heads.
    pub(super) fn point_is_final_atom(&self, point: TerminalTextPoint) -> bool {
        self.atoms
            .iter()
            .rev()
            .find(|atom| atom.point.is_some())
            .is_some_and(|atom| atom.point == Some(point))
    }
}

/// WORD ignores punctuation; ordinary words retain the copy-mode classes.
fn same_word(left: TextClass, right: TextClass, big: bool) -> bool {
    if big {
        left != TextClass::Whitespace && right != TextClass::Whitespace
    } else {
        left == right
    }
}
