//! The human's private cursor and the retained interactive game view.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{SAVE_TYPE, game::BoardDetails, render};
use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, CustomDone, CustomFactoryOutcome,
    CustomMessageInput, ExtensionAPI, ExtensionCommandContext, ExtensionComponent, ExtensionResult,
    NotificationType, Presence, SendMessageOptions, TUI, Theme, UserContent, keys::matches_key,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// The current view's redraw handle and whether to inject game instructions.
#[derive(Default)]
pub(super) struct Controls {
    /// Closing or ending the game disables the turn instructions.
    pub active: Cell<bool>,
    /// No terminal handle is retained after view disposal.
    tui: RefCell<Option<TUI>>,
}
impl Controls {
    /// Requests a redraw without retaining any old component instance.
    pub fn refresh(&self) -> ExtensionResult<()> {
        if let Some(tui) = self.tui.borrow().as_ref() {
            tui.request_render(false)?;
        }
        Ok(())
    }
    /// Releases the view's terminal binding and stops prompt injection.
    fn stop(&self) {
        self.active.set(false);
        self.tui.take();
    }
}
/// The human cursor stays here, never in a saved or model-facing snapshot.
struct GameView {
    /// The same board that registered tool calls mutate.
    state: Rc<RefCell<BoardDetails>>,
    /// Private row and column, initialized at the center.
    cursor: RefCell<(usize, usize)>,
    /// Native key and message operations.
    api: ExtensionAPI,
    /// Host-selected live theme for this view.
    theme: Theme,
    /// View lifetime shared with the tool callbacks.
    controls: Rc<Controls>,
    /// Completes the custom view at most once.
    done: CustomDone<()>,
}
impl GameView {
    /// Closes without changing either player's board or cursor.
    fn close(&self) -> ExtensionResult<()> {
        self.controls.stop();
        (self.done)(())
    }
    /// Decodes arrows only on the human's turn and clamps at the board edges.
    fn move_cursor(&self, data: &str) -> ExtensionResult<bool> {
        let mut cursor = self.cursor.borrow_mut();
        if matches_key(&self.api, data, "up")? {
            cursor.0 = cursor.0.saturating_sub(1);
        } else if matches_key(&self.api, data, "down")? {
            cursor.0 = (cursor.0 + 1).min(2);
        } else if matches_key(&self.api, data, "left")? {
            cursor.1 = cursor.1.saturating_sub(1);
        } else if matches_key(&self.api, data, "right")? {
            cursor.1 = (cursor.1 + 1).min(2);
        } else {
            return Ok(false);
        }
        Ok(true)
    }
    /// Saves the human's move before asking the host to trigger an agent turn.
    fn play(&self) -> ExtensionResult<()> {
        let (r, c) = *self.cursor.borrow();
        let mut state = self.state.borrow_mut();
        if state.board[r][c] != ' ' {
            return Ok(());
        }
        state.board[r][c] = 'X';
        state.settle();
        if state.status == "playing" {
            state.current_turn = 'O';
        }
        self.api.append_entry(
            SAVE_TYPE,
            Some(serde_json::to_value(&*state).map_err(|e| e.to_string())?),
        )?;
        self.controls.refresh()?;
        if state.status != "playing" {
            self.controls.active.set(false);
            return super::game_over(&self.api, &state);
        }
        self.api.send_message(CustomMessageInput {
            custom_type: "tic-tac-toe-move".into(),
            content: UserContent::Text(format!("Player X played at (row={r}, col={c}). It is now Player O's turn.\n\nBoard (your cursor marked with <>):\n{}\n\nYour cursor is at (row={}, col={}). Decide your target cell, then emit every move_* and the final play as separate tic_tac_toe tool calls in THIS response.",state.ascii(),state.agent_cursor_row,state.agent_cursor_col)),
            display: true,
            details: Presence::Present(serde_json::to_string(&*state).map_err(|e| e.to_string())?.into()),
        },Some(SendMessageOptions { trigger_turn:Some(true),deliver_as:None }))
    }
    /// The status and footer preserve text labels as well as player colors.
    fn labels(&self, state: &BoardDetails) -> ExtensionResult<(String, String)> {
        let labels = match state.status.as_str() {
            "win_X" => ("X wins!", "R restart  |  Q/ESC quit"),
            "win_O" => ("O wins!", "R restart  |  Q/ESC quit"),
            "draw" => ("Draw!", "R restart  |  Q/ESC quit"),
            _ if state.current_turn == 'O' => {
                ("X (You)  |  Turn: O (Agent)", "Agent is thinking...")
            }
            _ => (
                "Turn: X (You)  |  O (Agent)",
                "←↑↓→ move  |  ENTER play  |  ESC quit",
            ),
        };
        Ok((self.theme.bold(labels.0)?, self.theme.fg("dim", labels.1)?))
    }
}
impl ComponentView for GameView {
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        if matches_key(&self.api, data, "escape")? || data.eq_ignore_ascii_case("q") {
            return self.close();
        }
        if self.state.borrow().status != "playing" {
            if data.eq_ignore_ascii_case("r") {
                return self.close();
            }
            return Ok(());
        }
        if self.state.borrow().current_turn != 'X' {
            return Ok(());
        }
        if self.move_cursor(data)? {
            return self.controls.refresh();
        }
        if matches_key(&self.api, data, "return")? || data == " " {
            return self.play();
        }
        Ok(())
    }
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        if width == 0 {
            return Ok(Vec::new());
        }
        let columns = usize::try_from(width).unwrap_or(usize::MAX);
        let state = self.state.borrow();
        let cursor = if state.status != "playing" {
            None
        } else if state.current_turn == 'X' {
            let (r, c) = *self.cursor.borrow();
            Some((r, c, "success"))
        } else {
            Some((state.agent_cursor_row, state.agent_cursor_col, "warning"))
        };
        let (status, footer) = self.labels(&state)?;
        let title = self.theme.fg("accent", &self.theme.bold("Tic-Tac-Toe")?)?;
        let mut lines = vec![
            render::heading(&self.api, &self.theme, &title, columns)?,
            String::new(),
            render::center(&self.api, &status, columns)?,
            String::new(),
            String::new(),
        ];
        lines.extend(
            render::Board {
                details: &state,
                theme: &self.theme,
                cursor,
            }
            .render(&self.api, width)?,
        );
        lines.extend([
            String::new(),
            String::new(),
            render::center(&self.api, &footer, columns)?,
            String::new(),
            self.theme.fg("dim", &"─".repeat(columns))?,
        ]);
        Ok(lines)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}
/// Opens the command only on interactive hosts, reconstructing before each new view.
pub(super) async fn show(
    api: ExtensionAPI,
    state: Rc<RefCell<BoardDetails>>,
    controls: Rc<Controls>,
    ctx: ExtensionCommandContext,
) -> ExtensionResult<()> {
    if !ctx.has_ui()? {
        ctx.ui()?.notify(
            "Tic-tac-toe requires interactive mode",
            Some(NotificationType::Error),
        )?;
        return Ok(());
    }
    super::restore(&ctx, &state)?;
    if state.borrow().status != "playing" {
        state.replace(BoardDetails::default());
    }
    api.set_session_name("Tic-Tac-Toe")?;
    let ui = ctx.ui()?;
    controls.active.set(true);
    let closing = controls.clone();
    let result = ui
        .custom(
            Rc::new(move |tui, theme, _, done, _| {
                controls.tui.replace(Some(tui));
                let disposing = controls.clone();
                Ok(CustomFactoryOutcome::Ready(ExtensionComponent {
                    component: Component::new(GameView {
                        state: state.clone(),
                        cursor: RefCell::new((1, 1)),
                        api: api.clone(),
                        theme,
                        controls: controls.clone(),
                        done,
                    }),
                    dispose: Some(Rc::new(move |_| {
                        disposing.stop();
                        Ok(())
                    })),
                    set_expanded: None,
                    width: None,
                }))
            }),
            None,
        )
        .await;
    closing.stop();
    result.map(|_| ())
}
