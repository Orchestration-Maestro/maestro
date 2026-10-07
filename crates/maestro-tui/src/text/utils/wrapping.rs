use super::{TextPiece, ansi::AnsiCodeTracker, fragments, pieces, visible_width};

fn split_into_tokens_with_ansi(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut pending = String::new();
    let mut in_space = false;
    for piece in fragments(text) {
        let plain = match piece {
            TextPiece::Ansi(code) => {
                pending.push_str(code);
                continue;
            }
            TextPiece::Text(plain) => plain,
        };
        for ch in plain.chars() {
            let space = ch == ' ';
            if space != in_space && !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            current.push_str(&pending);
            pending.clear();
            current.push(ch);
            in_space = space;
        }
    }
    current.push_str(&pending);
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn break_long_word(
    word: &str,
    max: usize,
    tracker: &mut AnsiCodeTracker,
    prefix: String,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = prefix;
    let mut width: usize = 0;
    for piece in pieces(word) {
        match piece {
            TextPiece::Ansi(code) => {
                current.push_str(code);
                tracker.process(code);
            }
            TextPiece::Text(grapheme) => {
                let cells = visible_width(grapheme);
                if width > 0 && cells > max.saturating_sub(width) {
                    current.push_str(&tracker.get_line_end_reset());
                    lines.push(std::mem::take(&mut current));
                    current = tracker.get_active_codes();
                    width = 0;
                }
                current.push_str(grapheme);
                width = width.saturating_add(cells);
            }
        }
    }
    lines.push(current);
    lines
}

fn wrap_single_line(text: &str, max: usize) -> Vec<String> {
    if visible_width(text) <= max {
        return vec![text.into()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut width: usize = 0;
    let mut tracker = AnsiCodeTracker::default();
    for token in split_into_tokens_with_ansi(text) {
        let cells = visible_width(&token);
        let whitespace = token.trim().is_empty();
        if cells > max && !whitespace {
            if width > 0 {
                current.push_str(&tracker.get_line_end_reset());
                lines.push(std::mem::take(&mut current));
            }
            let prefix = if current.is_empty() {
                tracker.get_active_codes()
            } else {
                std::mem::take(&mut current)
            };
            let mut broken = break_long_word(&token, max, &mut tracker, prefix);
            current = broken.pop().unwrap_or_default();
            width = visible_width(&current);
            lines.extend(broken);
            continue;
        }
        if width > 0 && cells > max.saturating_sub(width) {
            let mut line = current.trim_end().to_owned();
            line.push_str(&tracker.get_line_end_reset());
            lines.push(line);
            current = tracker.get_active_codes();
            width = 0;
            if !whitespace {
                current.push_str(&token);
                width = cells;
            }
        } else {
            current.push_str(&token);
            width = width.saturating_add(cells);
        }
        tracker.update(&token);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
        .into_iter()
        .map(|line| line.trim_end().to_owned())
        .collect()
}
/// Wrap words while retaining ANSI styles, without padding.
#[must_use]
pub fn wrap_text_with_ansi(text: &str, max: usize) -> Vec<String> {
    let mut tracker = AnsiCodeTracker::default();
    text.split('\n')
        .enumerate()
        .flat_map(|(index, line)| {
            let prefix = if index == 0 {
                String::new()
            } else {
                tracker.get_active_codes()
            };
            let wrapped = wrap_single_line(&(prefix + line), max);
            tracker.update(line);
            wrapped
        })
        .collect()
}
