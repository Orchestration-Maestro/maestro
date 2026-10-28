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

/// Rejected session naming never enables game guidance for a later ordinary prompt.
#[test]
fn tic_tac_toe_failed_setup_leaves_prompt_unchanged() -> Result<(), String> {
    let mut host = Harness::example("tic_tac_toe")?;
    host.observed().catalog.data.has_ui = true;
    host.observed().readers.control(
        &json!({"bind":{"replies":[{"method":"getBranch","args":[null],"value":[]}]}}).to_string(),
    );
    host.observed().actions.error = Some("session naming refused".into());
    let command = host.observed().callback("command tic-tac-toe")?;
    assert_eq!(
        super::example_support::invoke_command_on_component(&mut host, command, "", false),
        Err("session naming refused".into())
    );
    host.observed().actions.error = None;
    let key = host.observed().callback("event before_agent_start")?;
    let capabilities = super::host_family::guest::Capabilities {
        ctx: host.ordinary("/work").map_err(|e| e.to_string())?,
        signal: None,
    };
    let event = json!({"type":"before_agent_start","prompt":"ordinary prompt",
        "systemPrompt":"original instructions","systemPromptOptions":{"cwd":"/work"}});
    let exports = host.exports.clone();
    let delivered = super::block_on(host.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_event(
                accessor,
                super::host::borrow(key),
                event.to_string(),
                capabilities,
            )
            .await
    }))
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())??;
    assert_eq!(
        super::documents::Answer::of(super::host_family::delivery(delivered))?.ended,
        super::documents::Ended::Returned(None)
    );
    super::example_support::released_component(&mut host)?;
    Ok(())
}

/// Supplies branch entries to the registered reconstruction callback.
fn restore(host: &mut Harness, name: &str, branch: &Value) -> Result<(), String> {
    host.observed().readers.control(
        &json!({"bind":{"replies":[{"method":"getBranch","args":[null],"value":branch}]}})
            .to_string(),
    );
    let key = host.observed().callback(&format!("event {name}"))?;
    let ctx = host.ordinary("/work").map_err(|e| e.to_string())?;
    let exports = host.exports.clone();
    let event = if name == "session_start" {
        json!({"type":name,"reason":"resume"})
    } else {
        json!({"type":name,"newLeafId":"new","oldLeafId":"old"})
    };
    let outcome = super::block_on(host.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_event(
                accessor,
                super::host::borrow(key),
                event.to_string(),
                super::host_family::guest::Capabilities { ctx, signal: None },
            )
            .await
    }))
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())??;
    match outcome.decision {
        super::host_family::guest::Decision::Failed(error) => Err(error),
        super::host_family::guest::Decision::Returned(result) => result.map(|_| ()),
    }
}

/// An ordinary custom save on the current branch, not an observation of private game state.
fn save(details: &Value) -> Value {
    json!({"type":"custom","id":"save","parentId":null,"timestamp":"2026-01-01T00:00:00Z","customType":"tic-tac-toe-save","data":details})
}

/// Moves await the authored delay, clamp, retain failed cursors and persist detached snapshots.
#[test]
fn tic_tac_toe_atomic_moves_restore_and_persist_on_the_branch() -> Result<(), String> {
    let mut host = Harness::example("tic_tac_toe")?;
    let board = json!({"board":[["X"," "," "],[" "," "," "],[" "," "," "]],"agentCursorRow":0,"agentCursorCol":0,"status":"playing","currentTurn":"O"});
    restore(&mut host, "session_start", &json!([save(&board)]))?;
    assert_eq!(
        tool(&mut host, "tic_tac_toe_see_board", &json!({}))?["details"],
        board
    );
    assert!(
        tool(&mut host, "tic_tac_toe", &json!({"action":"play"}))
            .unwrap_err()
            .contains("already X")
    );
    for (action, row, col) in [
        ("move_up", 0, 0),
        ("move_left", 0, 0),
        ("move_down", 1, 0),
        ("move_down", 2, 0),
        ("move_down", 2, 0),
        ("move_right", 2, 1),
        ("move_right", 2, 2),
        ("move_right", 2, 2),
        ("move_up", 1, 2),
    ] {
        let result = tool(&mut host, "tic_tac_toe", &json!({"action":action}))?;
        assert_eq!(result["details"]["agentCursorRow"], row);
        assert_eq!(result["details"]["agentCursorCol"], col);
    }
    let played = tool(&mut host, "tic_tac_toe", &json!({"action":"play"}))?;
    assert_eq!(
        played["details"]["board"],
        json!([["X", " ", " "], [" ", " ", "O"], [" ", " ", " "]])
    );
    assert_eq!(played["details"]["agentCursorRow"], 0);
    assert_eq!(played["details"]["agentCursorCol"], 0);
    assert_eq!(played["details"]["currentTurn"], "X");
    assert_eq!(
        host.observed()
            .transcript
            .iter()
            .filter(|s| s.as_str() == "native-wait 0 300000000")
            .count(),
        9
    );
    persisted_and_replayed(&mut host, &board, &played)?;
    super::example_support::released_component(&mut host)?;
    Ok(())
}

/// Verifies the host received detached saves and replays only supplied branch snapshots.
fn persisted_and_replayed(host: &mut Harness, board: &Value, played: &Value) -> Result<(), String> {
    let saves: Vec<Value> = host
        .observed()
        .transcript
        .iter()
        .filter_map(|s| s.strip_prefix("entry tic-tac-toe-save "))
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(saves.len(), 11);
    assert_eq!(&saves[0], board);
    assert_eq!(saves[10], played["details"]);
    let message = json!({"type":"message","id":"result","parentId":null,"timestamp":"2026-01-01T00:00:00Z","message":{"role":"toolResult","toolCallId":"call","toolName":"tic_tac_toe","content":[],"details":played["details"],"isError":false,"timestamp":0}});
    restore(host, "session_tree", &json!([save(board), message]))?;
    assert_eq!(
        tool(host, "tic_tac_toe_see_board", &json!({}))?["details"],
        played["details"]
    );
    restore(host, "session_tree", &json!([]))?;
    assert_eq!(
        tool(host, "tic_tac_toe_see_board", &json!({}))?["details"]["board"],
        json!([[" ", " ", " "], [" ", " ", " "], [" ", " ", " "]])
    );
    Ok(())
}

/// Every row, column and diagonal can finish, and a full board without a line draws.
#[test]
fn tic_tac_toe_finishes_wins_and_draws_once() -> Result<(), String> {
    let mut host = Harness::example("tic_tac_toe")?;
    for (board, row, col, status) in FINISHES {
        let details = json!({"board":board,"agentCursorRow":row,"agentCursorCol":col,"status":"playing","currentTurn":"O"});
        restore(&mut host, "session_tree", &json!([save(&details)]))?;
        let before = host
            .observed()
            .transcript
            .iter()
            .filter(|s| s.starts_with("action sendMessage "))
            .count();
        let result = tool(&mut host, "tic_tac_toe", &json!({"action":"play"}))?;
        assert_eq!(result["details"]["status"], status);
        assert_eq!(result["details"]["agentCursorRow"], 0);
        assert_eq!(result["details"]["agentCursorCol"], 0);
        assert_eq!(
            tool(&mut host, "tic_tac_toe", &json!({"action":"play"})),
            Err(format!("Game is over ({status})."))
        );
        let messages: Vec<Value> = host
            .observed()
            .transcript
            .iter()
            .filter_map(|s| s.strip_prefix("action sendMessage "))
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert_eq!(messages.len(), before + 1);
        assert_eq!(
            messages.last().unwrap()[0]["value"]["customType"],
            "tic-tac-toe-game-over"
        );
        assert_eq!(
            messages.last().unwrap()[0]["value"]["details"],
            result["details"]
        );
    }
    super::example_support::released_component(&mut host)?;
    Ok(())
}

/// Independently worked final moves covering all eight lines and a draw.
const FINISHES: [([[&str; 3]; 3], usize, usize, &str); 9] = [
    (
        [["O", "O", " "], [" ", "X", " "], ["X", " ", " "]],
        0,
        2,
        "win_O",
    ),
    (
        [["X", " ", " "], ["O", "O", " "], ["X", " ", " "]],
        1,
        2,
        "win_O",
    ),
    (
        [["X", " ", " "], [" ", "X", " "], ["O", "O", " "]],
        2,
        2,
        "win_O",
    ),
    (
        [["O", "X", " "], ["O", "X", " "], [" ", " ", " "]],
        2,
        0,
        "win_O",
    ),
    (
        [["X", "O", " "], ["X", "O", " "], [" ", " ", " "]],
        2,
        1,
        "win_O",
    ),
    (
        [["X", " ", "O"], ["X", " ", "O"], [" ", " ", " "]],
        2,
        2,
        "win_O",
    ),
    (
        [["O", "X", " "], ["X", "O", " "], [" ", " ", " "]],
        2,
        2,
        "win_O",
    ),
    (
        [["X", "X", "O"], [" ", "O", " "], [" ", " ", " "]],
        2,
        0,
        "win_O",
    ),
    (
        [["X", "O", "X"], ["X", "O", "O"], ["O", "X", " "]],
        2,
        2,
        "draw",
    ),
];

/// Malformed saves cannot reach board indexing or replace the last accepted state.
#[test]
fn tic_tac_toe_rejects_invalid_snapshots_without_replacing_state() -> Result<(), String> {
    let mut host = Harness::example("tic_tac_toe")?;
    let initial = tool(&mut host, "tic_tac_toe_see_board", &json!({}))?;
    for (field, value) in [
        ("agentCursorRow", json!(3)),
        ("agentCursorCol", json!(9)),
        ("currentTurn", json!("Z")),
        ("status", json!("unknown")),
        (
            "board",
            json!([["Z", " ", " "], [" ", " ", " "], [" ", " ", " "]]),
        ),
    ] {
        let mut invalid = initial["details"].clone();
        invalid[field] = value;
        assert!(restore(&mut host, "session_tree", &json!([save(&invalid)])).is_err());
        assert_eq!(
            tool(&mut host, "tic_tac_toe_see_board", &json!({}))?,
            initial
        );
    }
    assert_eq!(
        tool(&mut host, "tic_tac_toe", &json!({"action":"jump"})),
        Err("Unknown action: jump".into())
    );
    super::example_support::released_component(&mut host)?;
    Ok(())
}
