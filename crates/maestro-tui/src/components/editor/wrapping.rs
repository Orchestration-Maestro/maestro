//! Word boundaries with original byte ranges.
use crate::visible_width;
use std::collections::HashSet;
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
    let atoms = atoms(line);
    let groups = groups(&atoms, pre_segmented);
    let mut scan = Wrap::default();
    for (index, group) in groups.iter().enumerate() {
        let at = group[0].start;
        let cells = group.iter().map(|atom| atom.cells).sum();
        scan.overflow(line, at, cells, max_width);
        if cells > max_width && group.len() > 1 {
            let inner = Wrap::refine(line, group, max_width);
            scan.chunks.extend(inner.chunks);
            scan.start = inner.start;
            scan.width = inner.width;
            scan.opportunity = None;
            continue;
        }
        scan.width += cells;
        if whitespace(line, group)
            && let Some(following) = groups.get(index + 1)
            && !whitespace(line, following)
        {
            scan.opportunity = Some((following[0].start, scan.width));
        }
    }
    scan.chunks.push(chunk(line, scan.start, line.len()));
    scan.chunks
}
/// One visible grapheme with any escapes inside or immediately before it.
pub(super) struct Atom {
    /// Original byte range start, including preceding escapes.
    pub(super) start: usize,
    /// Original byte range end, including trailing escapes at line end.
    pub(super) end: usize,
    /// Shared measurement of the visible grapheme.
    pub(super) cells: usize,
    /// Shared whitespace classification of the visible grapheme.
    whitespace: bool,
}
/// Removes recognized escapes for segmentation while retaining original scalar ends.
pub(super) fn atoms(line: &str) -> Vec<Atom> {
    let endings = crate::text::Endings::of(line);
    let mut visible = String::new();
    let mut positions = Vec::new();
    let mut end = 0;
    for (at, scalar) in line.char_indices() {
        if at < end {
            continue;
        }
        if let Some(code) = endings.recognize(line, at) {
            end = at + code.len();
        } else {
            visible.push(scalar);
            positions.push((visible.len(), at + scalar.len_utf8()));
        }
    }
    let mut start = 0;
    let mut atoms = Vec::new();
    for (at, grapheme) in crate::get_segmenter(&visible) {
        let index = positions.partition_point(|&(end, _)| end < at + grapheme.len());
        let end = positions[index].1;
        atoms.push(Atom {
            start,
            end,
            cells: visible_width(grapheme),
            whitespace: crate::text::utils::is_whitespace_char(grapheme),
        });
        start = end;
    }
    if let Some(last) = atoms.last_mut() {
        last.end = line.len();
    }
    atoms
}
/// Keeps supplied spans only at visible atom boundaries; an empty list stays empty.
fn groups<'a>(atoms: &'a [Atom], supplied: Option<&[(usize, &str)]>) -> Vec<&'a [Atom]> {
    let Some(parts) = supplied else {
        return atoms.chunks(1).collect();
    };
    if parts.is_empty() {
        return Vec::new();
    }
    let starts: HashSet<usize> = parts.iter().map(|&(at, _)| at).collect();
    let mut boundaries = vec![0];
    for (index, atom) in atoms.iter().enumerate().skip(1) {
        if starts.contains(&atom.start) {
            boundaries.push(index);
        }
    }
    boundaries.push(atoms.len());
    boundaries
        .windows(2)
        .map(|pair| &atoms[pair[0]..pair[1]])
        .collect()
}
/// Supplied marker spans suppress internal word opportunities.
fn whitespace(line: &str, group: &[Atom]) -> bool {
    !marker(&line[group[0].start..group[group.len() - 1].end])
        && group.iter().any(|atom| atom.whitespace)
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
    /// Refines an oversized supplied span by consuming its visible atoms once.
    fn refine(line: &str, atoms: &[Atom], maximum: usize) -> Self {
        let mut scan = Self {
            start: atoms[0].start,
            ..Self::default()
        };
        for (index, atom) in atoms.iter().enumerate() {
            scan.overflow(line, atom.start, atom.cells, maximum);
            scan.width += atom.cells;
            if atom.whitespace
                && let Some(next) = atoms.get(index + 1)
                && !next.whitespace
            {
                scan.opportunity = Some((next.start, scan.width));
            }
        }
        scan
    }

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
