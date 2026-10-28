//! Pane input through the daemon's handler: right-click routing and the text, key and combined
//! input commands, with the bytes observed by a real shell.
#![cfg(target_os = "linux")]
use crate::support::api::{
    TestApp, answer, daemon_with_workspaces, failure, request, root_pane, run_in_pane, split_pane,
    wait_for_line, with_live_daemon,
};
use maestro_protocol::schema::{ResponseResult, SplitDirection};
use maestro_workspace::terminal_id::TerminalId;
use serde_json::json;

/// Makes the child enable bracketed paste before it reads.
const BRACKETED_PASTE: &str = "printf '\\033[?2004h'; ";

/// Makes the child push the kitty keyboard protocol flags 7 before it reads.
const KITTY_KEYBOARD: &str = "printf '\\033[>7u'; ";

/// A live workspace whose only pane prints the next `count` bytes it receives as a hex line.
///
/// `preamble` runs in the shell first. Returns the pane handle and its terminal.
async fn watching(app: &mut TestApp, count: usize, preamble: &str) -> (String, TerminalId) {
    let created = answer(app, json!({"type": "workspace.create"})).await;
    let Ok(ResponseResult::WorkspaceCreated {
        root_pane: created_pane,
        ..
    }) = created
    else {
        panic!("expected a created workspace: {created:?}");
    };
    let pane = root_pane(app, 0);
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    let script = format!(
        "{preamble}stty -echo -icanon -isig -icrnl -inlcr -ixon -iexten; echo READ\"Y\"; \
         od -An -vtx1 -w32 -N{count} | {{ read line; echo \"G\"\"OT $line\"; }}\n"
    );
    run_in_pane(app, &terminal, &script, "READY").await;
    (created_pane.pane_id, terminal)
}

/// The bytes the watched pane received, as space-separated hex.
async fn received(app: &TestApp, terminal: &TerminalId) -> String {
    let text = wait_for_line(app, terminal, "GOT ").await;
    text.lines()
        .find_map(|line| line.strip_prefix("GOT "))
        .unwrap()
        .trim()
        .to_owned()
}

/// Sends `keys` to `pane` and checks that the daemon accepted them.
async fn send_keys(app: &mut TestApp, pane: &str, keys: &[&str]) {
    let wire = json!({"type": "pane.send_keys", "pane_id": pane, "keys": keys});
    assert_eq!(answer(app, wire).await, Ok(ResponseResult::Ok {}));
}

#[test]
fn control_chords_are_sent_as_their_control_bytes() {
    with_live_daemon(
        "pane_input::control_chords_are_sent_as_their_control_bytes",
        async |app| {
            let (pane, terminal) = watching(app, 4, "").await;
            send_keys(app, &pane, &["ctrl+h", "ctrl+j", "ctrl+k", "ctrl+l"]).await;
            assert_eq!(received(app, &terminal).await, "08 0a 0b 0c");
        },
    );
}

#[test]
fn legacy_control_c_spellings_and_a_literal_plus_are_accepted() {
    with_live_daemon(
        "pane_input::legacy_control_c_spellings_and_a_literal_plus_are_accepted",
        async |app| {
            let (pane, terminal) = watching(app, 4, "").await;
            send_keys(app, &pane, &["C-c", "c-c", "ctrl+c", "+"]).await;
            assert_eq!(received(app, &terminal).await, "03 03 03 2b");
        },
    );
}

#[test]
fn shift_tab_is_sent_as_the_backtab_sequence() {
    with_live_daemon(
        "pane_input::shift_tab_is_sent_as_the_backtab_sequence",
        async |app| {
            let (pane, terminal) = watching(app, 3, "").await;
            send_keys(app, &pane, &["shift+tab"]).await;
            assert_eq!(received(app, &terminal).await, "1b 5b 5a");
        },
    );
}

#[test]
fn shifted_punctuation_is_text_under_the_kitty_protocol() {
    with_live_daemon(
        "pane_input::shifted_punctuation_is_text_under_the_kitty_protocol",
        async |app| {
            let (pane, terminal) = watching(app, 1, KITTY_KEYBOARD).await;
            send_keys(app, &pane, &["shift+?"]).await;
            assert_eq!(received(app, &terminal).await, "3f");
        },
    );
}

#[test]
fn sent_text_is_not_framed_even_when_the_pane_asks_for_bracketed_paste() {
    with_live_daemon(
        "pane_input::sent_text_is_not_framed_even_when_the_pane_asks_for_bracketed_paste",
        async |app| {
            let (pane, terminal) = watching(app, 6, BRACKETED_PASTE).await;
            let wire = json!({"type": "pane.send_text", "pane_id": pane, "text": "A != B"});
            assert_eq!(answer(app, wire).await, Ok(ResponseResult::Ok {}));
            assert_eq!(received(app, &terminal).await, "41 20 21 3d 20 42");
        },
    );
}

#[test]
fn sent_input_brackets_the_text_and_follows_it_with_the_keys_in_one_write() {
    with_live_daemon(
        "pane_input::sent_input_brackets_the_text_and_follows_it_with_the_keys_in_one_write",
        async |app| {
            let (pane, terminal) = watching(app, 19, BRACKETED_PASTE).await;
            let wire = json!({
                "type": "pane.send_input", "pane_id": pane, "text": "A != B", "keys": ["Enter"]
            });
            assert_eq!(answer(app, wire).await, Ok(ResponseResult::Ok {}));
            assert_eq!(
                received(app, &terminal).await,
                "1b 5b 32 30 30 7e 41 20 21 3d 20 42 1b 5b 32 30 31 7e 0d"
            );
        },
    );
}

#[test]
fn sent_input_accepts_key_chords_without_text() {
    with_live_daemon(
        "pane_input::sent_input_accepts_key_chords_without_text",
        async |app| {
            let (pane, terminal) = watching(app, 1, "").await;
            let wire = json!({"type": "pane.send_input", "pane_id": pane, "keys": ["ctrl+j"]});
            assert_eq!(answer(app, wire).await, Ok(ResponseResult::Ok {}));
            assert_eq!(received(app, &terminal).await, "0a");
        },
    );
}

#[test]
fn a_refused_key_writes_nothing_not_even_the_text_or_keys_before_it() {
    with_live_daemon(
        "pane_input::a_refused_key_writes_nothing_not_even_the_text_or_keys_before_it",
        async |app| {
            let (pane, terminal) = watching(app, 1, "").await;
            let keys = json!({
                "type": "pane.send_keys", "pane_id": pane, "keys": ["ctrl+h", "not-a-key"]
            });
            assert_eq!(
                answer(app, keys).await,
                failure("invalid_key", "unsupported key not-a-key")
            );
            let prefixed = json!({
                "type": "pane.send_input", "pane_id": pane, "text": "hello",
                "keys": ["ctrl+h", " prefix+h "]
            });
            assert_eq!(
                answer(app, prefixed).await,
                failure("invalid_key", "unsupported key  prefix+h ")
            );
            let sentinel = json!({"type": "pane.send_text", "pane_id": pane, "text": "Z"});
            assert_eq!(answer(app, sentinel).await, Ok(ResponseResult::Ok {}));
            assert_eq!(received(app, &terminal).await, "5a");
        },
    );
}

#[test]
fn input_a_busy_pane_cannot_take_is_reported_as_a_send_failure() {
    with_live_daemon(
        "pane_input::input_a_busy_pane_cannot_take_is_reported_as_a_send_failure",
        async |app| {
            let created = answer(app, json!({"type": "workspace.create"})).await;
            let Ok(ResponseResult::WorkspaceCreated { root_pane, .. }) = created else {
                panic!("expected a created workspace: {created:?}");
            };
            let terminal = app.state.topology.workspaces[0]
                .terminal_id(crate::support::api::root_pane(app, 0))
                .unwrap()
                .clone();
            run_in_pane(app, &terminal, "echo READ\"Y\"; exec sleep 600\n", "READY").await;
            let wire = json!({
                "type": "pane.send_text", "pane_id": root_pane.pane_id, "text": "x".repeat(1024)
            });
            let mut taken = 0;
            let failure = loop {
                match answer(app, wire.clone()).await {
                    Ok(_) => taken += 1,
                    Err(error) => break error,
                }
                assert!(taken < 100_000, "the pane never refused input");
            };
            assert_eq!(failure.code, "pane_send_failed");
            assert_eq!(failure.message, "no available capacity");
        },
    );
}

#[test]
fn input_to_a_pane_that_cannot_be_written_to_is_refused() {
    let mut app = daemon_with_workspaces(&["/one"]);
    for command in ["pane.send_text", "pane.send_keys", "pane.send_input"] {
        for pane in ["w1:p1", "w1:p9"] {
            let wire = json!({"type": command, "pane_id": pane, "text": "x", "keys": ["a"]});
            assert_eq!(
                request(&mut app, wire),
                failure("pane_not_found", &format!("pane {pane} not found")),
                "{command} {pane}"
            );
        }
    }
}

#[test]
fn input_routing_changes_only_the_named_pane() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let target = root_pane(&app, 0);
    let other = split_pane(&mut app, 0, target, SplitDirection::Right);
    let routed = |app: &TestApp, pane| {
        app.state.topology.workspaces[0]
            .pane_state(pane)
            .unwrap()
            .right_click_passthrough
    };
    let wire = json!({"type": "pane.input.set", "pane_id": "w1:p1", "right_click": "pane"});
    assert_eq!(request(&mut app, wire), Ok(ResponseResult::Ok {}));
    assert!(routed(&app, target));
    assert!(!routed(&app, other));
    let back = json!({"type": "pane.input.set", "pane_id": "w1:p1", "right_click": "maestro"});
    assert_eq!(request(&mut app, back), Ok(ResponseResult::Ok {}));
    assert!(!routed(&app, target));
    let missing = json!({"type": "pane.input.set", "pane_id": "w1:p9", "right_click": "pane"});
    assert_eq!(
        request(&mut app, missing),
        failure("pane_not_found", "pane w1:p9 not found")
    );
}
