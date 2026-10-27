//! Board snapshots shared by the tools and session reconstruction.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{AgentToolResult, ExtensionResult, Presence, TextContent, UserBlock};
use serde::{Deserialize, Serialize};

/// Detached public snapshot, deliberately excluding the human's private cursor.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BoardDetails {
    /// Nine player marks or spaces.
    pub board: [[char; 3]; 3],
    /// Agent cursor row, reset only after a successful play.
    pub agent_cursor_row: usize,
    /// Agent cursor column, retained after a rejected play.
    pub agent_cursor_col: usize,
    /// Playing, either player's win, or draw.
    pub status: String,
    /// Player whose next play is legal.
    pub current_turn: char,
}
impl Default for BoardDetails {
    fn default() -> Self {
        Self {
            board: [[' '; 3]; 3],
            agent_cursor_row: 0,
            agent_cursor_col: 0,
            status: "playing".into(),
            current_turn: 'X',
        }
    }
}
impl BoardDetails {
    /// Renders empty coordinates and only the agent's cursor.
    pub fn ascii(&self) -> String {
        self.board
            .iter()
            .enumerate()
            .map(|(r, row)| {
                row.iter()
                    .enumerate()
                    .map(|(c, mark)| {
                        match (
                            *mark,
                            r == self.agent_cursor_row && c == self.agent_cursor_col,
                        ) {
                            (' ', true) => format!("<[{r},{c}]>"),
                            (' ', false) => format!(" [{r},{c}] "),
                            (mark, true) => format!("   <{mark}>   "),
                            (mark, false) => format!("    {mark}    "),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("|")
            })
            .collect::<Vec<_>>()
            .join("\n---------+---------+---------\n")
    }
    /// Encodes a detached snapshot beside the tool's human-readable answer.
    pub fn result(&self, text: String) -> ExtensionResult<AgentToolResult> {
        Ok(AgentToolResult {
            content: vec![UserBlock::Text(TextContent::new(text))],
            details: Presence::Present(
                serde_json::to_string(self)
                    .map_err(|e| e.to_string())?
                    .into(),
            ),
            terminate: Presence::Missing,
        })
    }
    /// Restates the board without revealing the human cursor.
    pub fn inspect(&self) -> ExtensionResult<AgentToolResult> {
        let row = self.agent_cursor_row;
        let col = self.agent_cursor_col;
        let turn = if self.current_turn == 'O' {
            "Player O (you)"
        } else {
            "Player X"
        };
        self.result(format!("Board (your cursor marked with <>):\n{}\n\nYour cursor: (row={row}, col={col})\nStatus: {}\nTurn: {turn}", self.ascii(), self.status))
    }
    /// Applies one clamped cursor move or legal placement, retaining failed cursors.
    pub fn action(&mut self, action: &str) -> ExtensionResult<String> {
        match action {
            "move_up" => self.agent_cursor_row = self.agent_cursor_row.saturating_sub(1),
            "move_down" => self.agent_cursor_row = (self.agent_cursor_row + 1).min(2),
            "move_left" => self.agent_cursor_col = self.agent_cursor_col.saturating_sub(1),
            "move_right" => self.agent_cursor_col = (self.agent_cursor_col + 1).min(2),
            "play" => return self.play(),
            _ => return Err(format!("Unknown action: {action}")),
        }
        Ok(format!(
            "Moved {}. Cursor: ({}, {})",
            action.trim_start_matches("move_"),
            self.agent_cursor_row,
            self.agent_cursor_col
        ))
    }
    /// Places O at its cursor and resets only a successful placement to home.
    fn play(&mut self) -> ExtensionResult<String> {
        if self.status != "playing" {
            return Err(format!("Game is over ({}).", self.status));
        }
        if self.current_turn != 'O' {
            return Err("It is not your turn.".into());
        }
        let (r, c) = (self.agent_cursor_row, self.agent_cursor_col);
        if self.board[r][c] != ' ' {
            return Err(format!(
                "Cell ({r},{c}) is already {}. Your cursor is still at ({r},{c}). Move to an empty cell and retry play.",
                self.board[r][c]
            ));
        }
        self.board[r][c] = 'O';
        self.agent_cursor_row = 0;
        self.agent_cursor_col = 0;
        self.current_turn = 'X';
        Ok(format!(
            "Placed O at ({r},{c}). Cursor reset to (0,0). Your turn, X!"
        ))
    }
}
