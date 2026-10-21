//! Retained indented-code admission with line-local indentation grammar.

/// Whether later source can change the recognized raw end.
pub(crate) enum Verdict {
    /// Another indented or separator line can still be admitted.
    NeedMore,
    /// The first non-code line settled the raw end.
    Stable(Option<usize>),
}

/// Monotone input framing with no retained authored copy.
#[derive(Default)]
pub(crate) struct Recognizer {
    /// Shared monotone physical-line framing.
    lines: super::lines::Cursor,
    /// End of the last admitted code or separator line.
    consumed: Option<usize>,
    /// Whether a non-code line ended this operand.
    stable: bool,
}

impl Recognizer {
    /// Extend one operand without revisiting completed lines.
    pub(crate) fn extend(&mut self, source: &(impl super::lines::Input + ?Sized)) -> Verdict {
        while !self.stable {
            let Some(line) = self.lines.next(source) else {
                break;
            };
            self.consume(&source.window(line.clone()), line.end);
        }
        if self.stable {
            Verdict::Stable(self.consumed)
        } else {
            Verdict::NeedMore
        }
    }

    /// Resolve the final unterminated line at the caller's boundary.
    pub(crate) fn finish(&mut self, source: &(impl super::lines::Input + ?Sized)) -> Option<usize> {
        if let Verdict::Stable(result) = self.extend(source) {
            return result;
        }
        if let Some(line) = self.lines.finish(source) {
            self.consume(&source.window(line.clone()), line.end);
        }
        self.stable = true;
        self.consumed
    }

    /// Admit an indented nonempty line, or a separator following admitted code.
    fn consume(&mut self, text: &str, end: usize) {
        charge(text.len());
        let content = text.strip_suffix('\n').unwrap_or(text);
        let prefix = indentation(content);
        let code = (prefix == 4
            || content.as_bytes().get(prefix.saturating_sub(1)) == Some(&b'\t'))
            && content.len() > prefix;
        let blank =
            self.consumed.is_some() && content.bytes().all(|byte| matches!(byte, b' ' | b'\t'));
        if code || blank {
            self.consumed = Some(end);
        } else {
            self.stable = true;
        }
    }
}

/// Code or separator indentation, shared by admission and semantic dedenting.
pub(crate) fn indentation(line: &str) -> usize {
    let spaces = line
        .bytes()
        .take(4)
        .take_while(|byte| *byte == b' ')
        .count();
    if spaces == 4 {
        4
    } else if line.as_bytes().get(spaces) == Some(&b'\t') {
        spaces + 1
    } else {
        spaces
    }
}

/// Charge admission operands, including rejected lines.
fn charge(bytes: usize) {
    #[cfg(test)]
    crate::tests::WORK.with_borrow_mut(|work| work.code_bytes += bytes);
    #[cfg(not(test))]
    let _ = bytes;
}
