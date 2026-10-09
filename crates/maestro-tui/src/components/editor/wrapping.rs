//! Word boundaries with original byte ranges.
use crate::visible_width;
/// Emitted text and its half-open range in the original line.
#[derive(Debug, PartialEq, Eq)]
pub struct TextChunk {
    /// Text emitted for this chunk.
    pub text: String,
    /// Original starting byte offset.
    pub start_index: usize,
    /// Original ending byte offset.
    pub end_index: usize,
}
/// Wraps text with original byte ranges; empty input or zero width yields one empty chunk.
/// At positive widths, recognized escapes and indivisible overwide graphemes stay intact.
#[must_use]
pub fn word_wrap_line(
    line: &str,
    max_width: usize,
    pre_segmented: Option<&[(usize, &str)]>,
) -> Vec<TextChunk> {
    if line.is_empty() || max_width == 0 {
        return vec![chunk(line, 0, 0)];
    }
    if visible_width(line) <= max_width {
        return vec![chunk(line, 0, line.len())];
    }
    let segments = escape_safe_segments(line, pre_segmented);
    let mut scan = Wrap::default();
    for (index, &(at, text)) in segments.iter().enumerate() {
        let cells = visible_width(text);
        scan.overflow(line, at, cells, max_width);
        if cells > max_width && crate::get_segmenter(text).count() > 1 {
            let mut sub = word_wrap_line(text, max_width, None);
            if let Some(last) = sub.pop() {
                scan.start = at + last.start_index;
                scan.width = visible_width(&last.text);
                scan.chunks.extend(sub.into_iter().map(|part| TextChunk {
                    text: part.text,
                    start_index: at + part.start_index,
                    end_index: at + part.end_index,
                }));
                scan.opportunity = None;
            }
            continue;
        }
        scan.width += cells;
        if !marker(text)
            && visible_whitespace(text)
            && let Some(&(next, following)) = segments.get(index + 1)
            && (marker(following) || !visible_whitespace(following))
        {
            scan.opportunity = Some((next, scan.width));
        }
    }
    scan.chunks.push(chunk(line, scan.start, line.len()));
    scan.chunks
}
/// Keeps supplied boundaries except those inside recognized escapes.
/// Without supplied segments, grapheme boundaries frame the visible text.
fn escape_safe_segments<'a>(
    line: &'a str,
    supplied: Option<&[(usize, &str)]>,
) -> Vec<(usize, &'a str)> {
    let endings = crate::text::Endings::of(line);
    let mut escapes = Vec::new();
    let mut end = 0;
    for (at, _) in line.char_indices() {
        if at >= end
            && let Some(code) = endings.recognize(line, at)
        {
            end = at + code.len();
            escapes.push(at..end);
        }
    }
    let mut boundaries = supplied.map_or_else(
        || {
            crate::get_segmenter(line)
                .map(|(at, _)| at)
                .collect::<Vec<_>>()
        },
        |parts| parts.iter().map(|&(at, _)| at).collect(),
    );
    boundaries.retain(|&at| {
        let index = escapes.partition_point(|range| range.end <= at);
        escapes.get(index).is_none_or(|range| at <= range.start)
    });
    boundaries.push(line.len());
    boundaries
        .windows(2)
        .map(|pair| (pair[0], &line[pair[0]..pair[1]]))
        .collect()
}
/// Classifies whitespace outside recognized escape payloads.
fn visible_whitespace(text: &str) -> bool {
    let endings = crate::text::Endings::of(text);
    let mut end = 0;
    for (at, scalar) in text.char_indices() {
        if at < end {
            continue;
        }
        if let Some(code) = endings.recognize(text, at) {
            end = at + code.len();
        } else if crate::text::utils::is_whitespace_scalar(scalar) {
            return true;
        }
    }
    false
}
/// Current chunk and latest viable word boundary.
#[derive(Default)]
struct Wrap {
    /// Already emitted chunks.
    chunks: Vec<TextChunk>,
    /// Current range start.
    start: usize,
    /// Current cells.
    width: usize,
    /// Byte boundary and preceding cell count.
    opportunity: Option<(usize, usize)>,
}
impl Wrap {
    /// Selects the last viable word boundary or the current segment boundary.
    fn overflow(&mut self, line: &str, at: usize, cells: usize, maximum: usize) {
        if self.width + cells <= maximum {
            return;
        }
        if let Some((boundary, before)) = self
            .opportunity
            .filter(|&(_, before)| self.width - before + cells <= maximum)
        {
            self.chunks.push(chunk(line, self.start, boundary));
            self.start = boundary;
            self.width -= before;
        } else if self.start < at {
            self.chunks.push(chunk(line, self.start, at));
            self.start = at;
            self.width = 0;
        }
        self.opportunity = None;
    }
}
/// Recognizes the atomic marker spelling without interpreting its numeric identity.
fn marker(text: &str) -> bool {
    let Some(body) = text
        .strip_prefix("[paste #")
        .and_then(|text| text.strip_suffix(']'))
    else {
        return false;
    };
    let (id, suffix) = body
        .split_once(' ')
        .map_or((body, None), |(id, suffix)| (id, Some(suffix)));
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    digits(id)
        && suffix.is_none_or(|suffix| {
            suffix
                .strip_prefix('+')
                .and_then(|text| text.strip_suffix(" lines"))
                .or_else(|| suffix.strip_suffix(" chars"))
                .is_some_and(digits)
        })
}
/// Copies one selected range.
fn chunk(line: &str, start: usize, end: usize) -> TextChunk {
    TextChunk {
        text: line[start..end].to_owned(),
        start_index: start,
        end_index: end,
    }
}
