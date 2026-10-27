//! Original terminal sprites, colours, borders and status text.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::game::{GAME_HEIGHT, GAME_WIDTH, GameState, PLAYER_Y};
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionResult, truncate_to_width, utils::TruncateOptions, visible_width,
};

/// Applies the authored ANSI colour or emphasis.
fn style(code: u8, text: &str) -> String {
    let end = if matches!(code, 1 | 2) { 22 } else { 0 };
    format!("\x1b[{code}m{text}\x1b[{end}m")
}
/// Renders the first occupied entity at one cell, in drawing priority order.
fn cell(state: &GameState, x: i32, y: i32) -> String {
    for alien in state.aliens.iter().filter(|a| a.alive) {
        if alien.y == y && (alien.x - x).abs() <= 1 {
            let sprite = match (alien.kind, x.cmp(&alien.x)) {
                (0, std::cmp::Ordering::Less) | (1, std::cmp::Ordering::Greater) => "╲",
                (0, std::cmp::Ordering::Greater) | (1, std::cmp::Ordering::Less) => "╱",
                (0, _) => "▼",
                (1, _) => "◆",
                (_, std::cmp::Ordering::Less) => "◄",
                (_, std::cmp::Ordering::Greater) => "►",
                _ => "☆",
            };
            return style([32, 36, 35].get(alien.kind).copied().unwrap_or(35), sprite);
        }
    }
    if state.shields.iter().any(|s| s.contains(x, y)) {
        return style(2, "█");
    }
    if y == PLAYER_Y && (x - state.player.x).abs() <= 1 {
        return style(97, if x == state.player.x { "▲" } else { "═" });
    }
    state
        .bullets
        .iter()
        .find(|b| b.x == x && b.y == y)
        .map_or_else(
            || " ".into(),
            |b| style(if b.direction == -1 { 33 } else { 31 }, "│"),
        )
}
/// The controls or current terminal outcome.
fn footer(state: &GameState, paused: bool) -> String {
    if paused {
        format!(
            "{} Press any key to continue, {} to quit",
            style(33, &style(1, "PAUSED")),
            style(1, "Q")
        )
    } else if state.game_over {
        format!(
            "{} Press {} to restart, {} to quit",
            style(31, &style(1, "GAME OVER!")),
            style(1, "R"),
            style(1, "Q")
        )
    } else if state.victory {
        format!(
            "{} Press {} for level {}, {} to quit",
            style(32, &style(1, "VICTORY!")),
            style(1, "R"),
            state.level + 1,
            style(1, "Q")
        )
    } else {
        format!(
            "←→ or AD to move, {}/F to fire, {} pause, {} quit",
            style(1, "SPACE"),
            style(1, "ESC"),
            style(1, "Q")
        )
    }
}
/// Wraps text inside the fixed-width game box.
fn box_line(api: &ExtensionAPI, content: &str) -> ExtensionResult<String> {
    let padding = 60_usize.saturating_sub(visible_width(api, content)?);
    Ok(format!(
        "{}{content}{}{}",
        style(2, " │"),
        " ".repeat(padding),
        style(2, "│")
    ))
}
/// Builds one complete frame within the requested terminal width.
pub fn render(
    api: &ExtensionAPI,
    state: &GameState,
    paused: bool,
    width: u32,
) -> ExtensionResult<Vec<String>> {
    let border = "─".repeat(usize::try_from(GAME_WIDTH).map_err(|e| e.to_string())?);
    let separator = style(2, &format!(" ├{border}┤"));
    let header = format!(
        "{} │ Score: {} │ Hi: {} │ Lv: {} │ {}",
        style(1, &style(32, "SPACE INVADERS")),
        style(1, &style(33, &state.score.to_string())),
        style(1, &style(33, &state.high_score.to_string())),
        style(1, &style(36, &state.level.to_string())),
        style(
            31,
            &"♥".repeat(usize::try_from(state.player.lives).map_err(|e| e.to_string())?)
        )
    );
    let mut lines = vec![
        style(2, &format!(" ╭{border}╮")),
        box_line(api, &header)?,
        separator.clone(),
    ];
    for y in 0..GAME_HEIGHT {
        let row: String = (0..GAME_WIDTH).map(|x| cell(state, x, y)).collect();
        lines.push(format!("{}{row}{}", style(2, " │"), style(2, "│")));
    }
    lines.extend([
        separator,
        box_line(api, &footer(state, paused))?,
        style(2, &format!(" ╰{border}╯")),
    ]);
    let options = TruncateOptions {
        ellipsis: "",
        pad: false,
    };
    for line in &mut lines {
        let padding = (width as usize).saturating_sub(visible_width(api, line)?);
        line.push_str(&" ".repeat(padding));
        *line = truncate_to_width(api, line, width as usize, options)?;
    }
    Ok(lines)
}
