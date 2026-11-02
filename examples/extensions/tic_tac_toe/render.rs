//! Themed boards, move banners and final-game banners.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::game::BoardDetails;
use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, ExtensionAPI, ExtensionResult, Theme,
    utils::{TruncateOptions, truncate_to_width, visible_width},
};

/// Interior columns per cell.
const CELL_WIDTH: usize = 7;
/// Interior rows per cell.
const CELL_HEIGHT: usize = 3;
/// One border's relationship to its neighboring cells.
#[derive(Clone, Copy)]
enum Edge {
    /// Intersection of up to four cells.
    Corner,
    /// Segment above or below one column.
    Horizontal,
    /// Segment left or right of one row.
    Vertical,
}
/// Whether a cell touches this border coordinate, also used for cursor borders.
fn touches(point: (usize, usize), cell: (usize, usize), edge: Edge) -> bool {
    let rows = point.0 == cell.0 || point.0 == cell.0 + 1;
    let cols = point.1 == cell.1 || point.1 == cell.1 + 1;
    match edge {
        Edge::Corner => rows && cols,
        Edge::Horizontal => rows && point.1 == cell.1,
        Edge::Vertical => point.0 == cell.0 && cols,
    }
}
/// Centers with the host's cell-width measurements and clips whole styled graphemes.
pub(super) fn center(api: &ExtensionAPI, text: &str, width: usize) -> ExtensionResult<String> {
    let length = visible_width(api, text)?;
    if length >= width {
        return truncate_to_width(
            api,
            text,
            width,
            TruncateOptions {
                ellipsis: "",
                pad: false,
            },
        );
    }
    let pad = width - length;
    Ok(format!(
        "{}{}{}",
        " ".repeat(pad / 2),
        text,
        " ".repeat(pad - pad / 2)
    ))
}
/// Places a title between dim full-width separators and clips narrow terminals.
pub(super) fn heading(
    api: &ExtensionAPI,
    theme: &Theme,
    title: &str,
    width: usize,
) -> ExtensionResult<String> {
    let fill = width.saturating_sub(visible_width(api, title)? + 2);
    center(
        api,
        &format!(
            "{} {} {}",
            theme.fg("dim", &"─".repeat(fill / 2))?,
            title,
            theme.fg("dim", &"─".repeat(fill - fill / 2))?
        ),
        width,
    )
}
/// A board snapshot and optional live cursor, with no mutable rendering cache.
pub(super) struct Board<'a> {
    /// Caller-supplied snapshot.
    pub details: &'a BoardDetails,
    /// Live theme selected by the host.
    pub theme: &'a Theme,
    /// Cursor position and its semantic color; absent in static message snapshots.
    pub cursor: Option<(usize, usize, &'static str)>,
}
impl Board<'_> {
    /// Uses one role for a played cell and all of its matching borders.
    fn role(&self, cell: (usize, usize)) -> Option<&'static str> {
        let mark = self.details.board[cell.0][cell.1];
        if mark == ' ' {
            return None;
        }
        if self
            .details
            .win_line()
            .is_some_and(|line| line.contains(&cell))
        {
            return Some("success");
        }
        Some(if mark == 'X' { "accent" } else { "warning" })
    }
    /// Paints a border only when adjacent played cells agree, unless a cursor overrides it.
    fn edge(&self, text: &str, point: (usize, usize), kind: Edge) -> ExtensionResult<String> {
        if let Some((r, c, role)) = self.cursor
            && touches(point, (r, c), kind)
        {
            return self.theme.fg(role, &self.theme.bold(text)?);
        }
        let roles: Vec<_> = (0..3)
            .flat_map(|r| (0..3).map(move |c| (r, c)))
            .filter(|cell| touches(point, *cell, kind))
            .filter_map(|cell| self.role(cell))
            .collect();
        let role = roles
            .first()
            .filter(|first| roles.iter().all(|role| role == *first));
        role.map_or_else(
            || self.theme.fg("dim", text),
            |role| self.theme.fg(role, &self.theme.bold(text)?),
        )
    }
    /// Builds a horizontal row of intersections and seven-column segments.
    fn horizontal(&self, row: usize) -> ExtensionResult<String> {
        let corners = match row {
            0 => ['┌', '┬', '┬', '┐'],
            3 => ['└', '┴', '┴', '┘'],
            _ => ['├', '┼', '┼', '┤'],
        };
        let mut line = String::new();
        for (col, corner) in corners.into_iter().enumerate() {
            line.push_str(&self.edge(&corner.to_string(), (row, col), Edge::Corner)?);
            if col < 3 {
                line.push_str(&self.edge(&"─".repeat(CELL_WIDTH), (row, col), Edge::Horizontal)?);
            }
        }
        Ok(line)
    }
    /// Centers the mark on the cell's middle row, leaving its background untouched.
    fn cell(
        &self,
        api: &ExtensionAPI,
        cell: (usize, usize),
        line: usize,
    ) -> ExtensionResult<String> {
        let Some(role) = self.role(cell).filter(|_| line == CELL_HEIGHT / 2) else {
            return Ok(" ".repeat(CELL_WIDTH));
        };
        let glyph = if self.details.board[cell.0][cell.1] == 'X' {
            "╳"
        } else {
            "◯"
        };
        center(
            api,
            &self.theme.fg(role, &self.theme.bold(glyph)?)?,
            CELL_WIDTH,
        )
    }
    /// Builds one interior line with shared vertical borders.
    fn interior(&self, api: &ExtensionAPI, row: usize, line: usize) -> ExtensionResult<String> {
        let mut text = String::new();
        for col in 0..4 {
            text.push_str(&self.edge("│", (row, col), Edge::Vertical)?);
            if col < 3 {
                text.push_str(&self.cell(api, (row, col), line)?);
            }
        }
        Ok(text)
    }
    /// Returns thirteen centered board lines at any terminal width.
    pub fn render(&self, api: &ExtensionAPI, width: u32) -> ExtensionResult<Vec<String>> {
        let width = usize::try_from(width).unwrap_or(usize::MAX);
        let mut lines = Vec::new();
        for row in 0..3 {
            lines.push(center(api, &self.horizontal(row)?, width)?);
            for line in 0..CELL_HEIGHT {
                lines.push(center(api, &self.interior(api, row, line)?, width)?);
            }
        }
        lines.push(center(api, &self.horizontal(3)?, width)?);
        Ok(lines)
    }
}
/// A full-width title and optional board, retained independently of later game moves.
pub(super) struct Banner {
    /// Terminal helpers from the registering host.
    pub api: ExtensionAPI,
    /// Theme retained for this rendered message.
    pub theme: Theme,
    /// Themed title text.
    pub title: String,
    /// Detached board snapshot, never the live human cursor.
    pub details: Option<BoardDetails>,
    /// Whether a move or tool row includes its snapshot.
    pub expanded: bool,
    /// Final banners always show their board and outcome subtitle.
    pub game_over: bool,
}
impl ComponentView for Banner {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        if self.game_over {
            return self.final_lines(width);
        }
        let columns = usize::try_from(width).unwrap_or(usize::MAX);
        let mut lines = vec![heading(&self.api, &self.theme, &self.title, columns)?];
        if self.expanded
            && let Some(details) = &self.details
        {
            lines.push(String::new());
            lines.extend(
                Board {
                    details,
                    theme: &self.theme,
                    cursor: None,
                }
                .render(&self.api, width)?,
            );
        }
        Ok(lines)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}
impl Banner {
    /// Renders the final outcome between two full-width separators.
    fn final_lines(&self, width: u32) -> ExtensionResult<Vec<String>> {
        let status = self
            .details
            .as_ref()
            .map_or("draw", |details| details.status.as_str());
        let (title, subtitle, role) = match status {
            "win_X" => ("★ Player X wins ★", "You beat the agent.", "accent"),
            "win_O" => ("★ Player O wins ★", "The agent beat you.", "warning"),
            "draw" => ("— Draw —", "No winner.", "muted"),
            _ => ("Game over", "", "text"),
        };
        let columns = usize::try_from(width).unwrap_or(usize::MAX);
        let hr = self.theme.fg("dim", &"─".repeat(columns))?;
        let mut lines = vec![
            hr.clone(),
            String::new(),
            center(
                &self.api,
                &self.theme.bold(&self.theme.fg(role, title)?)?,
                columns,
            )?,
            center(&self.api, &self.theme.fg("dim", subtitle)?, columns)?,
            String::new(),
        ];
        if let Some(details) = &self.details {
            lines.extend(
                Board {
                    details,
                    theme: &self.theme,
                    cursor: None,
                }
                .render(&self.api, width)?,
            );
            lines.push(String::new());
        }
        lines.push(hr);
        Ok(lines)
    }
    /// Wraps a native component without creating a second rendering facade.
    pub fn component(self) -> Component {
        Component::new(self)
    }
}

/// Decodes optional saved details without accepting out-of-grid state into a renderer.
fn details(
    text: Option<&maestro_extensions_wasm::JsonText>,
) -> ExtensionResult<Option<BoardDetails>> {
    let details: Option<BoardDetails> = text
        .map(|text| serde_json::from_str(text))
        .transpose()
        .map_err(|e| e.to_string())?;
    if let Some(details) = &details {
        details.validate()?;
    }
    Ok(details)
}
/// Builds the ordinary unpadded text widget for collapsed rows and tool calls.
fn text(text: &str) -> ExtensionResult<Component> {
    Ok((&maestro_extensions_wasm::Text::new(text, 0, 0, None)?).into())
}
/// Retains the name of the tool in its call renderer, with the supplied action unchanged.
pub(super) fn call_renderer(inspect: bool) -> maestro_extensions_wasm::ToolRenderCall {
    std::rc::Rc::new(move |args, theme, _, _| {
        let name = if inspect {
            "tic_tac_toe_see_board"
        } else {
            "tic_tac_toe "
        };
        let mut title = theme.fg("toolTitle", &theme.bold(name)?)?;
        if !inspect {
            title.push_str(
                &theme.fg(
                    "muted",
                    args.as_ref()
                        .and_then(|args| args["action"].as_str())
                        .unwrap_or_default(),
                )?,
            );
        }
        text(&title)
    })
}
/// Uses error-aware summaries while expanded results show their detached board snapshot.
pub(super) fn result_renderer(
    api: ExtensionAPI,
    inspect: bool,
) -> maestro_extensions_wasm::ToolRenderResult {
    std::rc::Rc::new(move |result, options, theme, context, _| {
        let error = context.is_error;
        let details = if error {
            None
        } else {
            details(result.result.details.present())?
        };
        let message = match result.result.content.first() {
            Some(maestro_extensions_wasm::UserBlock::Text(text)) => text.text.as_str(),
            _ => "",
        };
        let prefix = theme.fg(
            if error { "error" } else { "success" },
            if error { "✗ " } else { "✓ " },
        )?;
        let summary = if inspect && !error {
            let (row, col) = details
                .as_ref()
                .map_or((0, 0), |d| (d.agent_cursor_row, d.agent_cursor_col));
            format!(
                "{prefix}{}",
                theme.fg("muted", &format!("cursor ({row},{col})"))?
            )
        } else {
            format!("{prefix}{}", theme.fg("muted", message)?)
        };
        if !options.expanded || details.is_none() {
            return text(&summary);
        }
        Ok(Banner {
            api: api.clone(),
            theme,
            title: summary,
            details,
            expanded: true,
            game_over: false,
        }
        .component())
    })
}
/// Registers user-move and final-game messages through the existing message renderer owner.
pub(super) fn register_messages(api: &ExtensionAPI) -> ExtensionResult<()> {
    for name in ["tic-tac-toe-move", "tic-tac-toe-game-over"] {
        let api_for_view = api.clone();
        api.register_message_renderer(
            name,
            std::rc::Rc::new(move |message, options, theme, _| {
                let details = details(message.details.present())?;
                let turn = if details
                    .as_ref()
                    .is_some_and(|details| details.current_turn == 'O')
                {
                    "O (Agent)"
                } else {
                    "X (You)"
                };
                let title = format!(
                    "{}  {}  next: {}",
                    theme.fg("accent", &theme.bold("Player X played")?)?,
                    theme.fg("dim", "→")?,
                    turn
                );
                Ok(Some(
                    Banner {
                        api: api_for_view.clone(),
                        theme,
                        title,
                        details,
                        expanded: options.expanded,
                        game_over: name == "tic-tac-toe-game-over",
                    }
                    .component(),
                ))
            }),
        )?;
    }
    Ok(())
}
