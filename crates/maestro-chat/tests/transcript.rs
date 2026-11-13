#![cfg(unix)]
//! Transcript rows, through a streamed response, a replayed session and the chat region.
use crate::interactive_mode_support::{
    Setup, World, assistant, assistant_record,
    isolation::isolated,
    transcript_corpus::{cases, palette},
};
use maestro_chat::Region;
use maestro_text::is_whitespace_scalar;
use maestro_theme::{ColorMode, Theme, ThemeOptions};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::Path, rc::Rc, sync::Arc};

/// The notice under a response its provider cut at the output limit.
const INCOMPLETE: &str = "Error: Model stopped because it reached the maximum output token limit. The response may be incomplete.";

/// A mode in `directory` whose terminal is wide enough for the notice on one row.
fn wide(directory: &Path) -> World {
    let setup = Setup {
        columns: 200,
        ..Setup::default()
    };
    setup.build(directory)
}

/// Whether the transcript of `world` shows the incomplete-response notice.
fn incomplete(world: &World) -> bool {
    world.region(Region::Chat).join("\n").contains(INCOMPLETE)
}

#[test]
fn transcript_length_stop_shows_the_incomplete_response_error() {
    const TEST: &str = "transcript_length_stop_shows_the_incomplete_response_error";
    isolated(TEST, async |directory| {
        let partial = json!({"type": "text", "text": "PARTIAL"});
        let cut_call = json!({"type": "toolCall", "id": "1", "name": "read",
            "arguments": {"z": 1, "a": 2}});
        for content in [json!([partial]), json!([partial, cut_call])] {
            // The response as it streams in.
            let world = wide(directory);
            world.start().await;
            assert!(!incomplete(&world), "nothing is answered yet");
            // The first request is cut at the limit; any later one ends at once.
            let answers = [assistant(&content, "length"), assistant(&json!([]), "stop")];
            world.script(move |request, stream| {
                stream.end(Some(Arc::clone(&answers[request.min(1)])));
            });
            world.prompt("run").await;
            assert!(incomplete(&world), "streamed {content}");
            // The same response replayed from the stored session.
            let world = wide(directory);
            let session = world.host.session();
            let record = assistant_record(&content, "length");
            let stored = session
                .session_manager()
                .borrow_mut()
                .append_message(record);
            stored.unwrap();
            world.start().await;
            assert!(incomplete(&world), "replayed {content}");
        }
    });
}

/// What a recorded case leaves in the transcript and the editor history.
#[derive(Deserialize)]
struct Shown {
    /// The transcript rows at width 100.
    rows: Vec<String>,
    /// Editor history entries, oldest first.
    history: Vec<String>,
}

/// A started mode 100 columns wide with images hidden, publishing the recorded palette.
async fn recorded(directory: &Path) -> World {
    let setup = Setup {
        columns: 100,
        configure: Box::new(|settings| settings.set_show_images(false)),
    };
    let world = setup.build(directory);
    world.themes.set_theme_instance(palette()).unwrap();
    world.start().await;
    world
}

/// Stores `record` as a message of the session of `world`.
fn store(world: &World, record: Value) {
    let session = world.host.session();
    let stored = session
        .session_manager()
        .borrow_mut()
        .append_message(record);
    stored.unwrap();
}

/// Replays the stored session into the transcript, as a resumed session is shown, and
/// draws the one frame that asks for.
fn replay(world: &World) {
    world.mode.render_current_session_state().unwrap();
    assert_eq!(world.runtime.pending(), 1, "the replay asks for one frame");
    world.runtime.settle().unwrap();
}

/// The editor history, oldest first, recalled with the Up key from the empty editor; the
/// editor is left on the oldest entry.
fn history(world: &World) -> Vec<String> {
    let mut recalled: Vec<String> = Vec::new();
    loop {
        world.terminal.send_input("\x1b[A");
        let text = world.mode.view().editor_text();
        if recalled
            .last()
            .map_or(text.is_empty(), |last| *last == text)
        {
            break;
        }
        recalled.push(text);
    }
    recalled.reverse();
    recalled
}

/// The recorded history as the editor admits it: each entry trimmed, and kept when nonempty.
fn admitted(history: &[String]) -> Vec<String> {
    let trimmed = history
        .iter()
        .map(|text| text.trim_matches(is_whitespace_scalar).to_owned());
    trimmed.filter(|text| !text.is_empty()).collect()
}

/// A stored user record with `content`.
fn user(content: &Value) -> Value {
    json!({"role": "user", "content": content, "timestamp": 0})
}

/// The rows the thinking shortcut's notice adds after it rebuilds an empty transcript.
async fn thinking_notice(directory: &Path) -> Vec<String> {
    let world = recorded(directory).await;
    world.terminal.send_input("\x14");
    world.region(Region::Chat)
}

/// A user message with its content and whether it joins the history.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UserInput {
    /// The operation name.
    op: String,
    /// Text or ordered text and image blocks.
    content: Value,
    /// Whether the history is populated.
    history: bool,
}

#[test]
fn transcript_user_text_keeps_order_whitespace_and_history() {
    const TEST: &str = "transcript_user_text_keeps_order_whitespace_and_history";
    isolated(TEST, async |directory| {
        let notice = thinking_notice(directory).await;
        for (input, expected) in cases::<UserInput, Shown>(TEST) {
            assert_eq!(input.op, "user");
            let world = recorded(directory).await;
            store(&world, user(&input.content));
            if input.history {
                // A resumed session shows the message and fills the history.
                replay(&world);
                assert_eq!(world.region(Region::Chat), expected.rows);
            } else {
                // The thinking shortcut rebuilds the transcript and leaves the history alone.
                world.terminal.send_input("\x14");
                let rows = [expected.rows, notice.clone()].concat();
                assert_eq!(world.region(Region::Chat), rows);
            }
            assert_eq!(history(&world), admitted(&expected.history));
        }
    });
}

/// A skill block with an optional trailing user text.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillInput {
    /// The operation name.
    op: String,
    /// Text after the skill block.
    tail: String,
    /// Whether rows start expanded.
    expanded: bool,
}

/// Expands tool output and summaries with the expansion shortcut when `expanded`.
fn expand(world: &World, expanded: bool) {
    if expanded {
        world.terminal.send_input("\x0f");
        world.runtime.settle().unwrap();
    }
}

#[test]
fn transcript_skill_block_and_trailing_user_keep_distinct_rows() {
    const TEST: &str = "transcript_skill_block_and_trailing_user_keep_distinct_rows";
    let skill =
        "<skill name=\"build\" location=\"/skills/build/SKILL.md\">\nDo **work**.\n</skill>";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<SkillInput, Shown>(TEST) {
            assert_eq!(input.op, "skill");
            let world = recorded(directory).await;
            expand(&world, input.expanded);
            store(
                &world,
                user(&Value::String(format!("{skill}{}", input.tail))),
            );
            replay(&world);
            assert_eq!(world.region(Region::Chat), expected.rows);
            assert_eq!(history(&world), admitted(&expected.history));
        }
    });
}

/// One message of an application or model role.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RoleInput {
    /// The operation name.
    op: String,
    /// The message role.
    role: String,
    /// Whether rows start expanded.
    expanded: bool,
}

/// The recorded message of `role`.
fn role(role: &str) -> Value {
    let usage = serde_json::json!({"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0,
        "totalTokens": 0, "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0,
        "total": 0}});
    match role {
        "custom" => serde_json::json!({"role": role, "customType": "note",
            "content": "CUSTOM_BODY", "display": true, "timestamp": 0}),
        "branchSummary" => serde_json::json!({"role": role, "summary": "BRANCH_BODY",
            "fromId": "from", "timestamp": 0}),
        "compactionSummary" => serde_json::json!({"role": role, "summary": "COMPACTION_BODY",
            "tokensBefore": 1234, "timestamp": 0}),
        "bashExecution" => serde_json::json!({"role": role, "command": "echo controlled",
            "output": "OUTPUT", "exitCode": 0, "cancelled": false, "truncated": false,
            "timestamp": 0}),
        "assistant" => serde_json::json!({"role": role, "content": [
            {"type": "thinking", "thinking": "REASON"}, {"type": "text", "text": "ANSWER"}],
            "api": "controlled", "provider": "controlled", "model": "controlled",
            "usage": usage, "stopReason": "stop", "timestamp": 0}),
        "toolResult" => serde_json::json!({"role": role, "toolCallId": "call",
            "toolName": "unknown_tool", "content": [{"type": "text",
            "text": "HISTORICAL_RESULT"}], "isError": false, "timestamp": 0}),
        other => panic!("unrecorded role {other}"),
    }
}

#[test]
fn transcript_message_roles_use_existing_components() {
    const TEST: &str = "transcript_message_roles_use_existing_components";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<RoleInput, Shown>(TEST) {
            assert_eq!(input.op, "role");
            let world = recorded(directory).await;
            expand(&world, input.expanded);
            store(&world, role(&input.role));
            replay(&world);
            assert_eq!(world.region(Region::Chat), expected.rows, "{}", input.role);
            assert_eq!(history(&world), admitted(&expected.history));
        }
    });
}

#[test]
fn transcript_registered_toggle_actions_release_the_mode() {
    const TEST: &str = "transcript_registered_toggle_actions_release_the_mode";
    isolated(TEST, async |directory| {
        let World {
            mode,
            terminal,
            runtime,
            themes,
            host,
        } = recorded(directory).await;
        let collapsed = mode.view().region(Region::Header);
        terminal.send_input("\x0f");
        assert_ne!(mode.view().region(Region::Header), collapsed);
        assert_eq!(runtime.pending(), 1);
        terminal.send_input("\x0f");
        assert_eq!(mode.view().region(Region::Header), collapsed);
        runtime.settle().unwrap();
        let settings = Rc::clone(host.session().settings_manager());
        let hidden = settings.borrow().get_hide_thinking_block();
        let empty = Theme::new([], [], ColorMode::Truecolor, ThemeOptions::default()).unwrap();
        themes.set_theme_instance(Rc::new(empty)).unwrap();
        terminal.send_input("\x14");
        assert_eq!(settings.borrow().get_hide_thinking_block(), !hidden);
        let errors = runtime.drain_futures();
        let [error] = <[_; 1]>::try_from(errors).ok().unwrap();
        assert!(
            error.downcast_ref::<maestro_theme::ThemeError>().is_some(),
            "{error}"
        );
        let weak = Rc::downgrade(&mode);
        drop(mode);
        assert!(weak.upgrade().is_none());
        runtime.settle().ok();
        // Keys typed after the mode is gone reach no action of it.
        for key in ["\x0f", "\x14"] {
            terminal.send_input(key);
        }
        assert_eq!(runtime.pending(), 0);
        assert!(runtime.drain_futures().is_empty());
    });
}
