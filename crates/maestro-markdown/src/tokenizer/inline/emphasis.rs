//! Delimiter flanking and masked emphasis resolution.
use super::Tokenizer;
use crate::{Kind, rules, tokenizer::capture};
use std::ops::Range;

impl Tokenizer<'_> {
    /// Check left flanking before searching any right-delimiter runs.
    fn opening(&self, range: &Range<usize>, previous: Option<char>) -> Option<(usize, u8)> {
        let text = &self.source.text[range.clone()];
        let width = self.runs.width(range)?;
        let tail = &text[width - 1..];
        let left = rules::inline_em_strong_l_delim(self.options)?.find(tail)?;
        if left.group(3).is_some()
            && previous.is_some_and(|previous| {
                rules::INLINE_UNICODE_ALPHA_NUMERIC
                    .as_ref()
                    .is_some_and(|rule| rule.find(previous.encode_utf8(&mut [0; 4])).is_some())
            })
        {
            return None;
        }
        let punctuation = capture(tail, &left, 1);
        let punctuation = if punctuation.is_empty() {
            capture(tail, &left, 2)
        } else {
            punctuation
        };
        let spacing = |previous: char| {
            let rule = rules::INLINE_PUNCTUATION.as_ref();
            rule.is_some_and(|rule| rule.find(previous.encode_utf8(&mut [0; 4])).is_some())
        };
        if !punctuation.is_empty() && previous.is_some_and(|previous| !spacing(previous)) {
            return None;
        }
        let delimiter = text.as_bytes()[0];
        Some((width, delimiter))
    }

    /// Select an eligible left delimiter and its matching masked right run.
    /// `masked` is the masked invocation source, its origin and the loop's previous character.
    pub(super) fn emphasis(
        &mut self,
        range: Range<usize>,
        masked: (&str, usize, Option<char>),
        atoms: &[Range<usize>],
        admission: Option<&mut crate::inline::InlineAdmission<'_>>,
    ) -> Option<(usize, Kind)> {
        let (masked, origin, previous) = masked;
        let (width, delimiter) = self.opening(&range, previous)?;
        let suffix = &masked[range.start - origin + width..];
        let rule = if delimiter == b'*' {
            rules::inline_em_strong_r_delim_ast(self.options)?
        } else {
            rules::INLINE_EM_STRONG_R_DELIM_UND.as_ref()?
        };
        let mut balance = isize::try_from(width).ok()?;
        let mut middle = 0;
        for right in rule.find_iter(suffix) {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.emphasis_right += right.range().len());
            let Some((group, run)) =
                (1..=6).find_map(|index| right.group(index).map(|range| (index, range)))
            else {
                continue;
            };
            let size = run.len();
            if group == 3 || group == 4 {
                balance += isize::try_from(size).ok()?;
                continue;
            }
            if group >= 5 && width % 3 != 0 && (width + size) % 3 == 0 {
                middle += size;
                continue;
            }
            balance -= isize::try_from(size).ok()?;
            if balance > 0 {
                continue;
            }
            let closing =
                size.min(usize::try_from(isize::try_from(size + middle).ok()? + balance).ok()?);
            let leading = suffix[right.start()..].chars().next()?.len_utf8();
            let end = width + right.start() + leading + closing;
            let trim = if width.min(closing) % 2 == 1 { 1 } else { 2 };
            if end > range.len() || end < trim * 2 {
                return None;
            }
            let children = self.tokens(
                range.start + trim..range.start + end - trim,
                atoms,
                admission,
            );
            return Some((
                end,
                if trim == 1 {
                    Kind::Emphasis(children)
                } else {
                    Kind::Strong(children)
                },
            ));
        }
        None
    }
}
