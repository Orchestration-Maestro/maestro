//! The game's bordered, two-column terminal cells and input hints.
use super::game::{GAME_HEIGHT, GAME_WIDTH, GameState, Point};
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionResult, truncate_to_width, utils::TruncateOptions,
};

/// Wraps text in the requested terminal style.
fn style(code: &str, text: &str) -> String {
    let reset = if code == "1" || code == "2" {
        "22"
    } else {
        "0"
    };
    format!("\x1b[{code}m{text}\x1b[{reset}m")
}
/// Pads one known-width row without decoding the generated style sequences.
fn pad(line: String, visible: usize, width: usize) -> String {
    line + &" ".repeat(width.saturating_sub(visible))
}
/// Places known-width text inside the game's dim border.
fn box_line(text: &str, visible: usize, size: usize) -> (String, usize) {
    (
        format!(
            "{}{text}{}{}",
            style("2", " │"),
            " ".repeat(size.saturating_sub(visible)),
            style("2", "│")
        ),
        size.max(visible) + 3,
    )
}
/// Draws the frame within the viewport, retaining the authored text and terminal colours.
///
/// # Errors
/// Returns the host's terminal truncation failure.
pub fn render(
    api: &ExtensionAPI,
    state: &GameState,
    paused: bool,
    width: u32,
) -> ExtensionResult<Vec<String>> {
    let width = usize::try_from(width).unwrap_or(usize::MAX);
    let columns = (width.saturating_sub(4) / 2).min(usize::try_from(GAME_WIDTH).unwrap_or(40));
    let size = columns * 2;
    let border = |left: &str, right: &str| {
        pad(
            style("2", &format!(" {left}{}{right}", "─".repeat(size))),
            size + 3,
            width,
        )
    };
    let boxed = |text: &str, visible| {
        let (line, visible) = box_line(text, visible, size);
        pad(line, visible, width)
    };
    let score = state.score.to_string();
    let high = state.high_score.to_string();
    let title = format!(
        "{} │ Score: {} │ High: {}",
        style("1", &style("32", "SNAKE")),
        style("1", &style("33", &score)),
        style("1", &style("33", &high))
    );
    let mut lines = vec![
        border("╭", "╮"),
        boxed(&title, 24 + score.len() + high.len()),
        border("├", "┤"),
    ];
    for y in 0..GAME_HEIGHT {
        let mut row = String::new();
        for x in 0..columns {
            let point = Point {
                x: i32::try_from(x).unwrap_or(0),
                y,
            };
            row.push_str(&cell(state, point));
        }
        lines.push(pad(
            format!("{}{row}{}", style("2", " │"), style("2", "│")),
            size + 3,
            width,
        ));
    }
    lines.push(border("├", "┤"));
    let (footer, visible) = footer(state, paused);
    lines.push(boxed(&footer, visible));
    lines.push(border("╰", "╯"));
    let options = TruncateOptions {
        ellipsis: "",
        pad: false,
    };
    lines
        .iter()
        .map(|line| truncate_to_width(api, line, width, options))
        .collect()
}
/// Selects the head, body, food or empty-cell glyph in that order.
fn cell(state: &GameState, point: Point) -> String {
    if state.snake.first() == Some(&point) {
        style("32", "██")
    } else if state.snake[1..].contains(&point) {
        style("32", "▓▓")
    } else if state.food == point {
        style("31", "◆ ")
    } else {
        "  ".into()
    }
}
/// The current input hint and its width before terminal styling.
fn footer(state: &GameState, paused: bool) -> (String, usize) {
    if paused {
        (
            format!(
                "{} Press any key to continue, {} to quit",
                style("33", &style("1", "PAUSED")),
                style("1", "Q")
            ),
            43,
        )
    } else if state.game_over {
        (
            format!(
                "{} Press {} to restart, {} to quit",
                style("31", &style("1", "GAME OVER!")),
                style("1", "R"),
                style("1", "Q")
            ),
            40,
        )
    } else {
        (
            format!(
                "↑↓←→ or WASD to move, {} pause, {} quit",
                style("1", "ESC"),
                style("1", "Q")
            ),
            39,
        )
    }
}
