//! ANSI-aware terminal cell operations.
use std::{cell::RefCell, collections::VecDeque};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
thread_local! { static WIDTH_CACHE: RefCell<VecDeque<(String, usize)>> = const { RefCell::new(VecDeque::new()) }; }
fn printable_ascii(s: &str) -> bool {
    s.bytes().all(|b| (0x20..=0x7e).contains(&b))
}
fn grapheme_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}
fn ansi_len(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.first() != Some(&27) {
        return None;
    }
    match b.get(1)? {
        b'[' => (2..b.len())
            .find(|&i| b"mGKHJ".contains(&b[i]))
            .map(|i| i + 1),
        b']' | b'_' => {
            let mut i = 2;
            while i < b.len() {
                if b[i] == 7 {
                    return Some(i + 1);
                }
                if b[i] == 27 && b.get(i + 1) == Some(&b'\\') {
                    return Some(i + 2);
                }
                i += 1;
            }
            None
        }
        _ => None,
    }
}

/// Measure visible terminal columns.
pub fn visible_width(text: &str) -> usize {
    if text.is_empty() || printable_ascii(text) {
        return text.len();
    }
    if let Some(width) =
        WIDTH_CACHE.with(|c| c.borrow().iter().find(|(k, _)| k == text).map(|(_, w)| *w))
    {
        return width;
    }
    let tabs = text.replace('\t', "   ");
    let mut clean = String::new();
    let mut i = 0;
    while i < tabs.len() {
        if let Some(len) = ansi_len(&tabs[i..]) {
            i += len;
        } else {
            let c = tabs[i..].chars().next().unwrap();
            clean.push(c);
            i += c.len_utf8();
        }
    }
    let width = clean.graphemes(true).map(grapheme_width).sum();
    WIDTH_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= 512 {
            c.pop_front();
        }
        c.push_back((text.to_owned(), width));
    });
    width
}
/// Truncate to columns; defaults to `...` and no padding.
pub fn truncate_to_width(
    text: &str,
    max_width: usize,
    ellipsis: Option<&str>,
    pad: Option<bool>,
) -> String {
    let ellipsis = ellipsis.unwrap_or("...");
    let pad = pad.unwrap_or(false);
    if max_width == 0 {
        return String::new();
    }
    if text.is_empty() {
        return if pad {
            " ".repeat(max_width)
        } else {
            String::new()
        };
    }
    let ew = visible_width(ellipsis);
    if ew >= max_width {
        let tw = visible_width(text);
        if tw <= max_width {
            return padded(text.to_owned(), max_width.saturating_sub(tw), pad);
        }
        let (clipped, cw) = fragment(ellipsis, max_width);
        if cw == 0 {
            return if pad {
                " ".repeat(max_width)
            } else {
                String::new()
            };
        }
        return finalized("", 0, &clipped, cw, max_width, pad);
    }
    if printable_ascii(text) {
        if text.len() <= max_width {
            return padded(text.into(), max_width.saturating_sub(text.len()), pad);
        }
        let target = max_width.saturating_sub(ew);
        return finalized(
            &text[..target.min(text.len())],
            target,
            ellipsis,
            ew,
            max_width,
            pad,
        );
    }
    let target = max_width.saturating_sub(ew);
    let (mut result, mut pending, mut visible, mut kept, mut contiguous, mut overflow) =
        (String::new(), String::new(), 0usize, 0usize, true, false);
    for (ansi, segment) in pieces(text, true) {
        if ansi {
            pending.push_str(segment);
            continue;
        }
        let w = if segment == "\t" {
            3
        } else {
            grapheme_width(segment)
        };
        if contiguous && kept + w <= target {
            result.push_str(&pending);
            pending.clear();
            result.push_str(segment);
            kept += w;
        } else {
            contiguous = false;
            pending.clear();
        }
        visible += w;
        if visible > max_width {
            overflow = true;
            break;
        }
    }
    if !overflow {
        return padded(text.into(), max_width.saturating_sub(visible), pad);
    }
    finalized(&result, kept, ellipsis, ew, max_width, pad)
}
fn padded(mut text: String, count: usize, pad: bool) -> String {
    if pad {
        text += &" ".repeat(count);
    }
    text
}
fn finalized(prefix: &str, pw: usize, ellipsis: &str, ew: usize, max: usize, pad: bool) -> String {
    let result = if ellipsis.is_empty() {
        format!("{prefix}\x1b[0m")
    } else {
        format!("{prefix}\x1b[0m{ellipsis}\x1b[0m")
    };
    padded(result, max.saturating_sub(pw + ew), pad)
}
fn fragment(text: &str, max: usize) -> (String, usize) {
    if max == 0 || text.is_empty() {
        return (String::new(), 0);
    }
    if printable_ascii(text) {
        let clipped = &text[..max.min(text.len())];
        return (clipped.into(), clipped.len());
    }
    let (mut out, mut pending, mut width) = (String::new(), String::new(), 0);
    for (ansi, segment) in pieces(text, true) {
        if ansi {
            pending += segment;
            continue;
        }
        let w = if segment == "\t" {
            3
        } else {
            grapheme_width(segment)
        };
        if width + w > max {
            break;
        }
        out += &pending;
        pending.clear();
        out += segment;
        width += w;
    }
    (out, width)
}
fn pieces(text: &str, tabs: bool) -> impl Iterator<Item = (bool, &str)> {
    let mut rest = text;
    let mut graphemes = "".graphemes(true);
    std::iter::from_fn(move || {
        if let Some(g) = graphemes.next() {
            return Some((false, g));
        }
        if rest.is_empty() {
            return None;
        }
        if let Some(len) = ansi_len(rest) {
            let code = &rest[..len];
            rest = &rest[len..];
            return Some((true, code));
        }
        if tabs && rest.starts_with('\t') {
            rest = &rest[1..];
            return Some((false, "\t"));
        }
        let end = rest
            .char_indices()
            .skip(1)
            .find(|(i, c)| {
                (tabs && *c == '\t') || (*c == '\x1b' && ansi_len(&rest[*i..]).is_some())
            })
            .map_or(rest.len(), |(i, _)| i);
        graphemes = rest[..end].graphemes(true);
        rest = &rest[end..];
        graphemes.next().map(|g| (false, g))
    })
}

/// Decompose only Thai and Lao AM vowels for terminal output.
pub fn normalize_terminal_output(text: &str) -> String {
    text.replace('\u{e33}', "\u{e4d}\u{e32}")
        .replace('\u{eb3}', "\u{ecd}\u{eb2}")
}

#[derive(Default)]
struct Tracker {
    flags: [bool; 8],
    fg: Option<String>,
    bg: Option<String>,
    link: Option<(String, String, String)>,
}
impl Tracker {
    fn process(&mut self, s: &str) {
        if let Some(body) = s.strip_prefix("\x1b]8;") {
            let term = if body.ends_with('\x07') {
                "\x07"
            } else {
                "\x1b\\"
            };
            if let Some(body) = body.strip_suffix(term)
                && let Some((params, url)) = body.split_once(';')
            {
                self.link = if url.is_empty() {
                    None
                } else {
                    Some((params.into(), url.into(), term.into()))
                };
            }
            return;
        }
        if !s.ends_with('m') {
            return;
        }
        let Some(body) = s.match_indices("\x1b[").find_map(|(i, _)| {
            let tail = &s[i + 2..];
            let end = tail.find('m')?;
            let body = &tail[..end];
            body.bytes()
                .all(|b| b.is_ascii_digit() || b == b';')
                .then_some(body)
        }) else {
            return;
        };
        if body.is_empty() || body == "0" {
            self.flags = [false; 8];
            self.fg = None;
            self.bg = None;
            return;
        }
        let p: Vec<&str> = body.split(';').collect();
        let mut i = 0;
        while i < p.len() {
            let c = p[i].parse::<usize>().unwrap_or(999);
            if c == 38 || c == 48 {
                let n = if p.get(i + 1) == Some(&"5") && p.get(i + 2).is_some() {
                    3
                } else if p.get(i + 1) == Some(&"2") && p.get(i + 4).is_some() {
                    5
                } else {
                    0
                };
                if n > 0 {
                    let code = p[i..i + n].join(";");
                    if c == 38 {
                        self.fg = Some(code)
                    } else {
                        self.bg = Some(code)
                    }
                    i += n;
                    continue;
                }
            }
            match c {
                0 => {
                    self.flags = [false; 8];
                    self.fg = None;
                    self.bg = None;
                }
                1..=5 => self.flags[c - 1] = true,
                7..=9 => self.flags[c - 2] = true,
                21 => self.flags[0] = false,
                22 => {
                    self.flags[0] = false;
                    self.flags[1] = false;
                }
                23..=25 => self.flags[c - 21] = false,
                27..=29 => self.flags[c - 22] = false,
                39 => self.fg = None,
                49 => self.bg = None,
                30..=37 | 90..=97 => self.fg = Some(c.to_string()),
                40..=47 | 100..=107 => self.bg = Some(c.to_string()),
                _ => {}
            }
            i += 1;
        }
    }
    fn update(&mut self, s: &str) {
        for (a, g) in pieces(s, false) {
            if a {
                self.process(g)
            }
        }
    }
    fn active(&self) -> String {
        let mut codes = vec![];
        for (i, on) in self.flags.iter().enumerate() {
            if *on {
                codes.push((if i < 5 { i + 1 } else { i + 2 }).to_string())
            }
        }
        if let Some(f) = &self.fg {
            codes.push(f.clone())
        }
        if let Some(b) = &self.bg {
            codes.push(b.clone())
        }
        let mut s = if codes.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", codes.join(";"))
        };
        if let Some((p, u, t)) = &self.link {
            s += &format!("\x1b]8;{p};{u}{t}")
        };
        s
    }
    fn end(&self) -> String {
        let mut s = if self.flags[3] {
            "\x1b[24m".into()
        } else {
            String::new()
        };
        if let Some((_, _, t)) = &self.link {
            s += &format!("\x1b]8;;{t}")
        };
        s
    }
}
fn trim_end(s: &str) -> &str {
    s.trim_end()
}
fn tokens(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut pending, mut space) = (vec![], String::new(), String::new(), false);
    // Space boundaries do not depend on grapheme segmentation.
    let mut i = 0;
    while i < s.len() {
        if let Some(l) = ansi_len(&s[i..]) {
            pending.push_str(&s[i..i + l]);
            i += l;
            continue;
        }
        let c = s[i..].chars().next().unwrap();
        let sp = c == ' ';
        if sp != space && !cur.is_empty() {
            out.push(std::mem::take(&mut cur))
        }
        cur.push_str(&pending);
        pending.clear();
        cur.push(c);
        space = sp;
        i += c.len_utf8();
    }
    cur.push_str(&pending);
    if !cur.is_empty() {
        out.push(cur)
    }
    out
}
fn break_word(s: &str, max: usize, t: &mut Tracker) -> Vec<String> {
    let (mut out, mut cur, mut w) = (vec![], t.active(), 0);
    for (a, g) in pieces(s, false) {
        if a {
            cur.push_str(g);
            t.process(g);
            continue;
        }
        let n = visible_width(g);
        if w + n > max {
            cur.push_str(&t.end());
            out.push(cur);
            cur = t.active();
            w = 0;
        }
        cur.push_str(g);
        w += n;
    }
    if !cur.is_empty() {
        out.push(cur)
    }
    if out.is_empty() {
        out.push(String::new())
    }
    out
}
fn wrap_line(s: &str, max: usize) -> Vec<String> {
    if s.is_empty() || visible_width(s) <= max {
        return vec![s.into()];
    }
    let (mut out, mut cur, mut w, mut t) = (vec![], String::new(), 0, Tracker::default());
    for token in tokens(s) {
        let n = visible_width(&token);
        let space = token.trim().is_empty();
        if n > max && !space {
            if !cur.is_empty() {
                cur.push_str(&t.end());
                out.push(cur);
            }
            let mut broken = break_word(&token, max, &mut t);
            cur = broken.pop().unwrap();
            w = visible_width(&cur);
            out.extend(broken);
            continue;
        }
        if w + n > max && w > 0 {
            let mut line = trim_end(&cur).to_string();
            line.push_str(&t.end());
            out.push(line);
            cur = t.active();
            if space {
                w = 0
            } else {
                cur.push_str(&token);
                w = n
            }
        } else {
            cur.push_str(&token);
            w += n;
        }
        t.update(&token);
    }
    if !cur.is_empty() {
        out.push(cur)
    }
    if out.is_empty() {
        out.push(String::new())
    }
    out.into_iter().map(|s| trim_end(&s).to_string()).collect()
}
/// Wrap words while retaining ANSI styles, without padding.
pub fn wrap_text_with_ansi(s: &str, max: usize) -> Vec<String> {
    let (mut out, mut t) = (vec![], Tracker::default());
    for line in s.split('\n') {
        let prefix = if out.is_empty() {
            String::new()
        } else {
            t.active()
        };
        out.extend(wrap_line(&(prefix + line), max));
        t.update(line);
    }
    out
}
/// Shared grapheme segmenter with owned-interface results.
pub struct Segmenter(());
/// A grapheme and its byte location in the original input.
pub struct SegmentData<'a> {
    /// Grapheme text.
    pub segment: &'a str,
    /// Start offset in UTF-8 bytes.
    pub index: usize,
    /// Original input.
    pub input: &'a str,
}
impl Segmenter {
    /// Iterate graphemes, retaining their byte offsets.
    pub fn segment<'a>(&self, input: &'a str) -> impl Iterator<Item = SegmentData<'a>> {
        let mut index = 0;
        input.graphemes(true).map(move |segment| {
            let start = index;
            index += segment.len();
            SegmentData {
                segment,
                index: start,
                input,
            }
        })
    }
}
/// Return the shared grapheme segmenter.
pub fn get_segmenter() -> &'static Segmenter {
    static SEGMENTER: Segmenter = Segmenter(());
    &SEGMENTER
}
/// Extract a supported escape at a byte offset, returning code and byte length.
pub fn extract_ansi_code(text: &str, pos: usize) -> Option<(String, usize)> {
    let rest = text.get(pos..)?;
    let len = ansi_len(rest)?;
    Some((rest[..len].into(), len))
}
/// Whether any supplied character is whitespace.
pub fn is_whitespace_char(text: &str) -> bool {
    text.chars().any(char::is_whitespace)
}
/// Whether any supplied character belongs to the punctuation set.
pub fn is_punctuation_char(text: &str) -> bool {
    text.chars()
        .any(|c| "(){}[]<>.,;:'\"!?+-=*/\\|&%^$#@~`".contains(c))
}
/// Pad without clipping, then invoke the supplied background style exactly once.
pub fn apply_background_to_line(
    line: &str,
    width: usize,
    bg_fn: &dyn Fn(&str) -> String,
) -> String {
    bg_fn(&(line.to_owned() + &" ".repeat(width.saturating_sub(visible_width(line)))))
}
/// Extract visible columns; strict mode excludes a crossing right-bound grapheme.
pub fn slice_by_column(
    line: &str,
    start_col: usize,
    length: usize,
    strict: Option<bool>,
) -> String {
    slice_with_width(line, start_col, length, strict).0
}
/// Extract visible columns and their measured width; tabs are zero-width here.
pub fn slice_with_width(
    line: &str,
    start_col: usize,
    length: usize,
    strict: Option<bool>,
) -> (String, usize) {
    if length == 0 {
        return (String::new(), 0);
    }
    let end = start_col + length;
    let (mut out, mut pending, mut width, mut col) = (String::new(), String::new(), 0, 0usize);
    for (ansi, g) in pieces(line, false) {
        if ansi {
            if col >= start_col && col < end {
                out += g;
            } else if col < start_col {
                pending += g;
            }
            continue;
        }
        let w = grapheme_width(g);
        if col >= start_col && col < end && (!strict.unwrap_or(false) || col + w <= end) {
            out += &pending;
            pending.clear();
            out += g;
            width += w;
        }
        col += w;
        if col >= end {
            break;
        }
    }
    (out, width)
}
/// Extract before/after regions, inheriting canonical active style in the after region.
pub fn extract_segments(
    line: &str,
    before_end: usize,
    after_start: usize,
    after_len: usize,
    strict_after: Option<bool>,
) -> (String, usize, String, usize) {
    let (mut before, mut bw, mut after, mut aw, mut col, mut pending, mut started, mut tracker) = (
        String::new(),
        0,
        String::new(),
        0,
        0usize,
        String::new(),
        false,
        Tracker::default(),
    );
    let end = after_start + after_len;
    for (ansi, g) in pieces(line, false) {
        if ansi {
            tracker.process(g);
            if col < before_end {
                pending += g;
            } else if col >= after_start && col < end && started {
                after += g;
            }
            continue;
        }
        let w = grapheme_width(g);
        if col < before_end {
            before += &pending;
            pending.clear();
            before += g;
            bw += w;
        } else if col >= after_start
            && col < end
            && (!strict_after.unwrap_or(false) || col + w <= end)
        {
            if !started {
                after += &tracker.active();
                started = true;
            }
            after += g;
            aw += w;
        }
        col += w;
        if if after_len == 0 {
            col >= before_end
        } else {
            col >= end
        } {
            break;
        }
    }
    (before, bw, after, aw)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn width_cache_retains_insertion_order() {
        WIDTH_CACHE.with(|c| c.borrow_mut().clear());
        visible_width("");
        visible_width("ASCII");
        WIDTH_CACHE.with(|c| assert!(c.borrow().is_empty()));
        visible_width("\t");
        visible_width("\x1b[31m界");
        for i in 0..510 {
            visible_width(&format!("界{i}"));
        }
        WIDTH_CACHE.with(|c| {
            let c = c.borrow();
            assert_eq!(c.len(), 512);
            assert_eq!(c[0].0, "\t");
            assert_eq!(c[1].0, "\x1b[31m界");
        });
        visible_width("\t");
        visible_width("界new");
        WIDTH_CACHE.with(|c| {
            let c = c.borrow();
            assert_eq!(c.len(), 512);
            assert_eq!(c[0].0, "\x1b[31m界");
            assert!(!c.iter().any(|(s, _)| s == "\t"));
        });
    }
}
