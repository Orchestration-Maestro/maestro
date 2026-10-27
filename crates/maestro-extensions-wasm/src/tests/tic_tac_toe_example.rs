//! The shipped game exercised through the built component's public callbacks.
use super::{host::Harness, host_family::guest::ToolCapabilities};
use serde_json::{Value, json};

/// Invokes a registered game tool with the same ordinary context as the runtime.
fn tool(host: &mut Harness, name: &str, params: &Value) -> Result<Value, String> {
    let key = host.observed().callback(&format!("tool {name}"))?;
    let ctx = host.ordinary("/work").map_err(|e| e.to_string())?;
    let encoded = super::example_support::invoke_tool_on_component(
        host,
        key,
        params,
        ToolCapabilities {
            ctx,
            signal: None,
            update: None,
        },
    )?;
    serde_json::from_str(&encoded).map_err(|e| e.to_string())
}

/// The agent sees only its cursor; an out-of-turn play leaves the board untouched.
#[test]
fn tic_tac_toe_initial_board_and_out_of_turn_play() -> Result<(), String> {
    let mut host = Harness::example("tic_tac_toe")?;
    let initial = tool(&mut host, "tic_tac_toe_see_board", &json!({}))?;
    assert_eq!(
        initial["details"],
        json!({
            "board":[[" "," "," "],[" "," "," "],[" "," "," "]],
            "agentCursorRow":0,"agentCursorCol":0,"status":"playing","currentTurn":"X"
        })
    );
    assert!(
        initial["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("<[0,0]>")
    );
    assert_eq!(
        tool(&mut host, "tic_tac_toe", &json!({"action":"play"})),
        Err("It is not your turn.".into())
    );
    assert_eq!(
        tool(&mut host, "tic_tac_toe_see_board", &json!({}))?,
        initial
    );
    super::example_support::released_component(&mut host)?;
    Ok(())
}
