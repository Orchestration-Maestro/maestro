//! Retained item admission over an append-only child operand.
use super::{
    Lexer,
    blocks::{BlockTokens, Mark},
    frames::forget,
};
use crate::{Kind, Node, rules::code, source::Segmented};

/// One item's child invocation, retained while its list resumes.
#[derive(Default)]
pub(crate) struct Child {
    /// Settled contributions, then those admitted from `open`.
    output: BlockTokens,
    /// Latest operand, after any task marker; its handle places the final task marker.
    operand: Option<Segmented>,
    /// Task-marker bytes removed before the operand.
    prefix: usize,
    /// Operand offset of the first open step: its grammar read the apparent end, or the
    /// content after it sits on the unterminated last line.
    open: usize,
    /// Output before that step.
    mark: Mark,
    /// Indented code from `open` through the apparent end, published only when settled.
    code: Option<code::Recognizer>,
}

impl Child {
    /// Publish undecided code, nested lists and the task marker once.
    pub(crate) fn finish(&mut self, task: Option<bool>) -> BlockTokens {
        let mut output = std::mem::take(&mut self.output);
        let Some(operand) = self.operand.take() else {
            return output;
        };
        if self.code.take().is_some() {
            output.code(&operand.select(self.open..operand.len()));
        }
        if let Some(checked) = task {
            let end = operand
                .native(&(0..operand.len()))
                .map_or(0, |range| range.end);
            output.nodes.insert(
                0,
                Node {
                    authored: operand.select(0..0).mapped(),
                    range: end..end,
                    kind: Kind::TaskListMarker(checked),
                },
            );
        }
        output
    }

    /// Grammar position this item's last open step was admitted from.
    pub(crate) fn enter_top(&self) -> bool {
        self.mark.top
    }

    /// Take the item's output for the block level admitting its body.
    pub(crate) fn take_output(&mut self) -> BlockTokens {
        std::mem::take(&mut self.output)
    }

    /// Keep the item's output once its body level completed.
    pub(crate) fn restore_output(&mut self, output: BlockTokens) {
        self.output = output;
    }
}

impl<E> Lexer<'_, E> {
    /// Select the suffix to admit again from the first open step, or none when the operand is
    /// unchanged without a caller tokenizer or open indented code still runs to its end.
    pub(crate) fn reopen(
        &mut self,
        source: &Segmented,
        prefix: usize,
        child: &mut Child,
    ) -> Option<Segmented> {
        if child.prefix != prefix {
            self.release(&mut child.output, Mark::default());
            *child = Child {
                prefix,
                ..Child::default()
            };
        }
        let seen = child.operand.replace(source.clone());
        if self.tokenizer.is_none() && seen.is_some_and(|seen| seen.len() == source.len()) {
            return None;
        }
        let rest = source.select(child.open..source.len());
        if let Some(code) = &mut child.code {
            if runs_to_end(code, &rest) {
                return None;
            }
            child.code = None;
        }
        self.release(&mut child.output, child.mark.clone());
        Some(rest)
    }

    /// Retain the first open step, keeping undecided code unpublished.
    pub(crate) fn settle(
        &mut self,
        source: &Segmented,
        child: &mut Child,
        open: Option<(usize, Mark)>,
    ) {
        let top = child.mark.top;
        let (offset, mark) =
            open.unwrap_or_else(|| (source.len() - child.open, Mark::new(&child.output, top)));
        child.open += offset;
        child.mark = mark;
        if self.tokenizer.is_none() {
            child.code = open_code(&source.select(child.open..source.len()));
        }
        if child.code.is_some() {
            self.release(&mut child.output, child.mark.clone());
        }
    }

    /// Restore `output` to `mark`, releasing scopes that removed containers published.
    fn release(&mut self, output: &mut BlockTokens, mark: Mark) {
        for node in output.restore(mark) {
            forget(&mut self.scopes, &node);
        }
    }
}

/// Indented code that runs from the operand's start through its apparent end.
fn open_code(rest: &Segmented) -> Option<code::Recognizer> {
    let content = rest
        .chunks()
        .flat_map(str::bytes)
        .take_while(|byte| *byte != b'\n')
        .any(|byte| !matches!(byte, b' ' | b'\t'));
    let mut code = code::Recognizer::default();
    (content && runs_to_end(&mut code, rest)).then_some(code)
}

/// Whether retained code still admits every line, ending in content that leaves no
/// separator for the dispatcher after it.
fn runs_to_end(code: &mut code::Recognizer, rest: &Segmented) -> bool {
    rest.mapped()
        .last_byte()
        .is_some_and(|byte| !matches!(byte, b' ' | b'\t' | b'\n'))
        && matches!(code.extend(rest), code::Verdict::NeedMore)
        && code.clone().finish(rest) == Some(rest.len())
}
