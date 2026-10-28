//! Play tic-tac-toe through a command and sequential atomic tools.
// Tests receive this component's path in MAESTRO_TIC_TAC_TOE_COMPONENT.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    AgentMessage, AgentToolResult, BeforeAgentStartEventResult, CommandOptions, CustomMessageInput,
    ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
    ExtensionResult, Message, Presence, SessionEntry, ToolDefinition, ToolExecutionMode,
    ToolMetadata, UserContent,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, time::Duration};
#[path = "tic_tac_toe/game.rs"]
mod game;
#[path = "tic_tac_toe/prompt.rs"]
mod prompt;
#[path = "tic_tac_toe/render.rs"]
mod render;
#[path = "tic_tac_toe/ui.rs"]
mod ui;
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
    restored.validate()?;
    state.replace(restored);
    Ok(())
}

/// Replays successful game snapshots, leaving runner errors and unrelated messages alone.
fn restore_result(message: AgentMessage, restored: &mut BoardDetails) -> ExtensionResult<()> {
    if let AgentMessage::Message(message) = message
        && let Message::ToolResult(message) = *message
        && matches!(
            message.tool_name.as_str(),
            "tic_tac_toe" | "tic_tac_toe_see_board"
        )
        && !message.is_error
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
    controls: Rc<ui::Controls>,
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
    if result.is_ok() && action == "play" && state.status != "playing" {
        controls.active.set(false);
        game_over(&api, &state)?;
    }
    if result.is_ok() || occupied {
        api.append_entry(
            SAVE_TYPE,
            Some(serde_json::to_value(&*state).map_err(|e| e.to_string())?),
        )?;
    }
    controls.refresh()?;
    state.result(result?)
}

/// Sends one final snapshot after the play that ends a game.
fn game_over(api: &ExtensionAPI, state: &BoardDetails) -> ExtensionResult<()> {
    let label = match state.status.as_str() {
        "win_X" => "Player X (human) wins",
        "win_O" => "Player O (agent) wins",
        "draw" => "Draw",
        _ => "Game over",
    };
    api.send_message(
        CustomMessageInput {
            custom_type: "tic-tac-toe-game-over".into(),
            content: UserContent::Text(format!("Game over: {label}.")),
            display: true,
            details: Presence::Present(
                serde_json::to_string(state)
                    .map_err(|e| e.to_string())?
                    .into(),
            ),
        },
        None,
    )
}

/// Builds metadata with exactly one action per call and explicit sequential execution.
fn metadata(inspect: bool) -> ToolMetadata {
    ToolMetadata {
        name: if inspect { "tic_tac_toe_see_board" } else { "tic_tac_toe" }.into(),
        label: if inspect { "See Board" } else { "Tic-Tac-Toe" }.into(),
        description: if inspect { "Read the tic-tac-toe position and your O cursor coordinates. The human cursor remains private." } else { "Perform one O action per call: move_up, move_down, move_left, move_right, or play. Movement stops at the board boundary. Submit the ordered moves and placement together in one assistant response, each as its own call; do not wait for results between calls. Only an accepted play returns your cursor to (0,0)." }.into(),
        parameters: if inspect { json!({"type":"object","properties":{}}) } else { json!({"type":"object","required":["action"],"properties":{"action":{"type":"string","enum":["move_up","move_down","move_left","move_right","play"]}}}) }.into(),
        execution_mode: if inspect { Presence::Missing } else { Presence::Present(ToolExecutionMode::Sequential) },
        prompt_snippet: Presence::Present(if inspect { "Read the tic-tac-toe position and your O cursor" } else { "Take one O action in tic-tac-toe: move_up, move_down, move_left, move_right, or play" }.into()),
        prompt_guidelines: if inspect { Presence::Missing } else { Presence::Present(vec![
            "On your tic-tac-toe turn, choose a destination before calling tools. Submit its ordered move_* calls followed by play together in one assistant response, with a separate tic_tac_toe call for each action. Do not continue the turn in another response or pause for action results.".into(),
            "Use the position and O cursor supplied with the human move instead of asking the human to describe the board. Request tic_tac_toe_see_board when you need another copy.".into(),
        ]) },
        render_shell: Presence::Missing,
    }
}

/// Registers one tool against the same retained board as every other callback.
fn register_tool(
    api: &ExtensionAPI,
    state: Rc<RefCell<BoardDetails>>,
    inspect: bool,
    controls: Rc<ui::Controls>,
) -> ExtensionResult<()> {
    let tool_api = api.clone();
    api.register_tool(ToolDefinition {
        metadata: metadata(inspect),
        prepare_arguments: None,
        execute: Rc::new(move |_, params, _, _, _| {
            if inspect {
                return Box::pin(std::future::ready(state.borrow().inspect()));
            }
            Box::pin(execute(
                tool_api.clone(),
                state.clone(),
                params,
                controls.clone(),
            ))
        }),
        render_call: Some(render::call_renderer(inspect)),
        render_result: Some(render::result_renderer(api.clone(), inspect)),
    })
}

/// Appends turn guidance only while this factory's interactive game remains active.
fn register_prompt(api: &ExtensionAPI, controls: Rc<ui::Controls>) -> ExtensionResult<()> {
    api.on(
        "before_agent_start",
        Rc::new(move |event, _| {
            let result = match event {
                ExtensionEvent::BeforeAgentStart(event) if controls.active.get() => Some(
                    ExtensionEventResult::BeforeAgentStart(BeforeAgentStartEventResult {
                        message: Presence::Missing,
                        system_prompt: Presence::Present(format!(
                            "{}{}",
                            event.system_prompt,
                            prompt::INSTRUCTIONS
                        )),
                    }),
                ),
                _ => None,
            };
            Box::pin(std::future::ready(Ok(result)))
        }),
    )
}

/// Registers the opt-in game tools and branch reconstruction hooks.
///
/// # Errors
/// Returns host registration failures and malformed retained snapshots.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let state = Rc::new(RefCell::new(BoardDetails::default()));
        let controls = Rc::new(ui::Controls::default());
        for event in ["session_start", "session_tree"] {
            let state = state.clone();
            let controls = controls.clone();
            api.on(
                event,
                Rc::new(move |_, ctx| {
                    controls.active.set(false);
                    Box::pin(std::future::ready(restore(&ctx, &state).map(|()| None)))
                }),
            )?;
        }
        register_tool(&api, state.clone(), false, controls.clone())?;
        register_tool(&api, state.clone(), true, controls.clone())?;
        render::register_messages(&api)?;
        register_prompt(&api, controls.clone())?;
        let command_api = api.clone();
        api.register_command(
            "tic-tac-toe",
            CommandOptions {
                description: Some("Play tic-tac-toe against the agent".into()),
                get_argument_completions: None,
                handler: Rc::new(move |_, ctx| {
                    Box::pin(ui::show(
                        command_api.clone(),
                        state.clone(),
                        controls.clone(),
                        ctx,
                    ))
                }),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
