//! Play tic-tac-toe through a command and sequential atomic tools.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    AgentToolResult, ExtensionAPI, ExtensionFuture, ExtensionResult, Presence, TextContent,
    ToolDefinition, ToolExecutionMode, ToolMetadata, UserBlock,
};
use serde_json::json;

/// Returns the initial board without exposing the human's private cursor.
fn inspect() -> AgentToolResult {
    let board = [[" "; 3]; 3];
    let rows = (0..3)
        .map(|r| {
            (0..3)
                .map(|c| {
                    if r == 0 && c == 0 {
                        format!("<[{r},{c}]>")
                    } else {
                        format!(" [{r},{c}] ")
                    }
                })
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect::<Vec<_>>()
        .join("\n---------+---------+---------\n");
    AgentToolResult {
        content: vec![UserBlock::Text(TextContent::new(format!("Board (your cursor marked with <>):\n{rows}\n\nYour cursor: (row=0, col=0)\nStatus: playing\nTurn: Player X")))],
        details: Presence::Present(json!({"board":board,"agentCursorRow":0,"agentCursorCol":0,"status":"playing","currentTurn":"X"}).to_string().into()),
        terminate: Presence::Missing,
    }
}

/// Routes each registered tool without admitting out-of-turn actions.
fn execute(name: &str) -> ExtensionResult<AgentToolResult> {
    if name == "tic_tac_toe" {
        Err("It is not your turn.".into())
    } else {
        Ok(inspect())
    }
}

/// Registers the opt-in game tools.
///
/// # Errors
/// Returns registration failures from the host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        for name in ["tic_tac_toe", "tic_tac_toe_see_board"] {
            api.register_tool(ToolDefinition {
                metadata: ToolMetadata {
                    name: name.into(), label: "Tic-Tac-Toe".into(),
                    description: if name == "tic_tac_toe" { "Execute ONE tic-tac-toe action as Player O." } else { "Inspect the board and your cursor. The user's cursor is never exposed." }.into(),
                    parameters: if name == "tic_tac_toe" { json!({"type":"object","required":["action"],"properties":{"action":{"type":"string","enum":["move_up","move_down","move_left","move_right","play"]}}}) } else { json!({"type":"object","properties":{}}) }.into(),
                    execution_mode: Presence::Present(ToolExecutionMode::Sequential),
                    prompt_snippet: Presence::Missing, prompt_guidelines: Presence::Missing, render_shell: Presence::Missing,
                },
                prepare_arguments: None,
                execute: std::rc::Rc::new(move |_, _, _, _, _| Box::pin(std::future::ready(execute(name)))),
                render_call: None, render_result: None,
            })?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
