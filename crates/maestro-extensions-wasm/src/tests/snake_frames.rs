//! Literal expected terminal frames for controlled Snake games.
use serde_json::{Value, json};

/// Counts visible characters in the source's literal terminal styles.
pub fn visible(text: &str) -> usize {
    regex::Regex::new("\x1b\\[[0-9;]*m")
        .unwrap()
        .replace_all(text, "")
        .chars()
        .count()
}
/// Literal host truncation replies for frames containing only single-cell glyphs and SGR styles.
pub fn fitted(line: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if visible(line) <= width {
        return line.into();
    }
    let tokens = regex::Regex::new("\x1b\\[[0-9;]*m|.").unwrap();
    let end = tokens
        .find_iter(line)
        .filter(|token| !token.as_str().starts_with('\x1b'))
        .take(width)
        .last()
        .unwrap()
        .end();
    format!("{}\x1b[0m", &line[..end])
}
/// A dim terminal border with the source's exact reset sequence.
fn dim(text: &str) -> String {
    format!("\x1b[2m{text}\x1b[22m")
}
/// Expected board rows from literal positions, independent of movement logic.
fn grid(game: &Value, columns: usize) -> Vec<String> {
    let snake = game["snake"].as_array().unwrap();
    (0..15)
        .map(|y| {
            let mut row = dim(" │");
            for x in 0..columns {
                let p = json!({"x":x,"y":y});
                row.push_str(if snake[0] == p {
                    "\x1b[32m██\x1b[0m"
                } else if snake[1..].contains(&p) {
                    "\x1b[32m▓▓\x1b[0m"
                } else if game["food"] == p {
                    "\x1b[31m◆ \x1b[0m"
                } else {
                    "  "
                });
            }
            row + &dim("│")
        })
        .collect()
}
/// Independently specified source output, including literal ANSI styles and footer wording.
pub fn frame(game: &Value, mode: &str, width: usize) -> Vec<String> {
    let columns = 40.min(width.saturating_sub(4) / 2);
    let size = columns * 2;
    let pad = |s: String| {
        let count = visible(&s);
        s + &" ".repeat(width.saturating_sub(count))
    };
    let boxed =
        |s: String| pad(dim(" │") + &s + &" ".repeat(size.saturating_sub(visible(&s))) + &dim("│"));
    let border = |l, r| pad(dim(&format!(" {l}{}{r}", "─".repeat(size))));
    let mut rows = vec![
        border("╭", "╮"),
        boxed(format!(
            "\x1b[1m\x1b[32mSNAKE\x1b[0m\x1b[22m │ Score: \x1b[1m\x1b[33m{}\x1b[0m\x1b[22m │ High: \x1b[1m\x1b[33m{}\x1b[0m\x1b[22m",
            game["score"], game["highScore"]
        )),
        border("├", "┤"),
    ];
    rows.extend(grid(game, columns).into_iter().map(pad));
    rows.push(border("├", "┤"));
    rows.push(boxed(match mode {
        "paused" => "\x1b[33m\x1b[1mPAUSED\x1b[22m\x1b[0m Press any key to continue, \x1b[1mQ\x1b[22m to quit",
        "over" => "\x1b[31m\x1b[1mGAME OVER!\x1b[22m\x1b[0m Press \x1b[1mR\x1b[22m to restart, \x1b[1mQ\x1b[22m to quit",
        _ => "↑↓←→ or WASD to move, \x1b[1mESC\x1b[22m pause, \x1b[1mQ\x1b[22m quit",
    }.into()));
    rows.push(border("╰", "╯"));
    rows
}
