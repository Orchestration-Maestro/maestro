//! Play tic-tac-toe through a command and sequential atomic tools.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    AgentMessage, AgentToolResult, ExtensionAPI, ExtensionContext, ExtensionFuture,
    ExtensionResult, Message, Presence, SessionEntry, ToolDefinition, ToolExecutionMode,
    ToolMetadata,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, time::Duration};
#[path = "tic_tac_toe/game.rs"]
mod game;
use game::BoardDetails;

/// The custom entry holding each accepted state, including failed occupied-cell plays.
const SAVE_TYPE: &str = "tic-tac-toe-save";

/// Replays both human-only saves and tool-result snapshots on the supplied branch.
fn restore(ctx: &ExtensionContext, state: &RefCell<BoardDetails>) -> ExtensionResult<()> {
    let mut restored = BoardDetails::default();
    for entry in ctx.session_manager()?.get_branch(None)? {
        match entry {
            SessionEntry::Custom(entry) if entry.custom_type == SAVE_TYPE => {
                if let Some(data) = entry.data.present() {
                    restored = serde_json::from_str(data).map_err(|e| e.to_string())?;
                }
            }
            SessionEntry::Message(entry) => restore_result(entry.message, &mut restored)?,
            _ => {}
        }
    }
    state.replace(restored);
    Ok(())
}

/// Admits only the two game tools' snapshots; other messages leave the state alone.
fn restore_result(message: AgentMessage, restored: &mut BoardDetails) -> ExtensionResult<()> {
    if let AgentMessage::Message(message) = message
        && let Message::ToolResult(message) = *message
        && matches!(
            message.tool_name.as_str(),
            "tic_tac_toe" | "tic_tac_toe_see_board"
        )
        && let Some(details) = message.details
    {
        *restored = serde_json::from_value(details).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Delays movements through the native owner, then applies and persists one atomic action.
async fn execute(
    api: ExtensionAPI,
    state: Rc<RefCell<BoardDetails>>,
    params: Value,
) -> ExtensionResult<AgentToolResult> {
    let action = params["action"].as_str().ok_or("action required")?;
    if matches!(action, "move_up" | "move_down" | "move_left" | "move_right") {
        api.native_io()
            .sleep(Duration::from_millis(300), None)
            .await?;
    }
    let mut state = state.borrow_mut();
    let occupied = action == "play"
        && state.status == "playing"
        && state.current_turn == 'O'
        && state.board[state.agent_cursor_row][state.agent_cursor_col] != ' ';
    let result = state.action(action);
    if result.is_ok() || occupied {
        api.append_entry(
            SAVE_TYPE,
            Some(serde_json::to_value(&*state).map_err(|e| e.to_string())?),
        )?;
    }
    state.result(result?)
}

/// Builds metadata with exactly one action per call and explicit sequential execution.
fn metadata(inspect: bool) -> ToolMetadata {
    ToolMetadata {
        name: if inspect { "tic_tac_toe_see_board" } else { "tic_tac_toe" }.into(),
        label: if inspect { "See Board" } else { "Tic-Tac-Toe" }.into(),
        description: if inspect { "Return the current tic-tac-toe board and YOUR cursor position (Player O). The user's cursor is never exposed." } else { "Execute ONE action as Player O: move_up, move_down, move_left, move_right, or play. Moves clamp at edges. Emit every move and the final play as separate tool calls in the SAME assistant response, without waiting for intermediate results. The cursor resets to (0,0) only after a successful play." }.into(),
        parameters: if inspect { json!({"type":"object","properties":{}}) } else { json!({"type":"object","required":["action"],"properties":{"action":{"type":"string","enum":["move_up","move_down","move_left","move_right","play"]}}}) }.into(),
        execution_mode: if inspect { Presence::Missing } else { Presence::Present(ToolExecutionMode::Sequential) },
        prompt_snippet: Presence::Present(if inspect { "Inspect the tic-tac-toe board and your cursor" } else { "Play a tic-tac-toe action (move_up/down/left/right or play) as Player O" }.into()),
        prompt_guidelines: Presence::Missing,
        render_shell: Presence::Missing,
    }
}

/// Registers one tool against the same retained board as every other callback.
fn register_tool(
    api: &ExtensionAPI,
    state: Rc<RefCell<BoardDetails>>,
    inspect: bool,
) -> ExtensionResult<()> {
    let tool_api = api.clone();
    api.register_tool(ToolDefinition {
        metadata: metadata(inspect),
        prepare_arguments: None,
        execute: Rc::new(move |_, params, _, _, _| {
            if inspect {
                return Box::pin(std::future::ready(state.borrow().inspect()));
            }
            Box::pin(execute(tool_api.clone(), state.clone(), params))
        }),
        render_call: None,
        render_result: None,
    })
}

/// Registers the opt-in game tools and branch reconstruction hooks.
///
/// # Errors
/// Returns host registration failures and malformed retained snapshots.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let state = Rc::new(RefCell::new(BoardDetails::default()));
        for event in ["session_start", "session_tree"] {
            let state = state.clone();
            api.on(
                event,
                Rc::new(move |_, ctx| {
                    Box::pin(std::future::ready(restore(&ctx, &state).map(|()| None)))
                }),
            )?;
        }
        register_tool(&api, state.clone(), false)?;
        register_tool(&api, state, true)
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
