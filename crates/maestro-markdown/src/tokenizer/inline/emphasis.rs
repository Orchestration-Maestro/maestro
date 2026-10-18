//! Delimiter flanking and masked emphasis resolution.
use super::Tokenizer;
use crate::{Kind, rules, tokenizer::capture};
use std::ops::Range;

impl Tokenizer<'_> {
    /// Check left flanking before searching any right-delimiter runs.
    fn opening(&self, range: &Range<usize>) -> Option<(usize, u8)> {
        let text = &self.source.text[range.clone()];
        let left = rules::inline_em_strong_l_delim(self.options)?.find(text)?;
        let previous = self.source.text[..range.start].chars().next_back();
        if left.group(3).is_some() && previous.is_some_and(char::is_alphanumeric) {
            return None;
        }
        let punctuation = capture(text, &left, 1);
        let punctuation = if punctuation.is_empty() {
            capture(text, &left, 2)
        } else {
            punctuation
        };
        if !punctuation.is_empty() && previous.is_some_and(char::is_alphanumeric) {
            return None;
        }
        let delimiter = text.as_bytes()[0];
        let width = text.bytes().take_while(|byte| *byte == delimiter).count();
        Some((width, delimiter))
    }

    /// Select an eligible left delimiter and its matching masked right run.
    pub(super) fn emphasis(
        &mut self,
        range: Range<usize>,
        masked: (&str, usize),
        atoms: &[Range<usize>],
        in_link: bool,
    ) -> Option<(usize, Kind)> {
        let (width, delimiter) = self.opening(&range)?;
        let (masked, origin) = masked;
        let suffix = &masked[range.start - origin + width..];
        let rule = if delimiter == b'*' {
            rules::inline_em_strong_r_delim_ast(self.options)?
        } else {
            rules::INLINE_EM_STRONG_R_DELIM_UND.as_ref()?
        };
        let mut balance = isize::try_from(width).ok()?;
        let mut middle = 0;
        for right in rule.find_iter(suffix) {
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
            let children =
                self.tokens(range.start + trim..range.start + end - trim, atoms, in_link);
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
