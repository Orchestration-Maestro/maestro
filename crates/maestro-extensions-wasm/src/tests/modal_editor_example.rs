//! The modal editor example exercised through its built component on a host custom editor.
use serde_json::{Value, json};
use wasmtime::component::Resource;

use super::host::{Harness, borrow};
use super::host_family::guest;
use super::renderer_host::{Call, Reply};

/// The escape byte the host's key matcher names `escape`.
const ESCAPE: &str = "\x1b";

/// Host helper answers: only the escape byte matches `escape`, and the bottom line `abc`
/// measures `width` cells.
fn helpers(width: usize) -> Value {
    let mut helpers = json!({ "keys.matches_key": false, "visible_width": width });
    helpers[format!("keys.matches_key {}", json!([ESCAPE, "escape"]))] = json!(true);
    for room in [0, 12] {
        helpers[format!("truncate_to_width {}", json!(["abc", room, "", false]))] =
            json!(&"ab"[..room.min(2)]);
    }
    helpers
}

/// Starts the built example and delivers one session start.
fn started() -> Result<Harness, String> {
    let mut host = Harness::example("modal_editor")?;
    host.observed().actions.value = helpers(20);
    let handler = host
        .callback("event session_start")
        .map_err(|error| error.to_string())?;
    let capabilities = guest::Capabilities {
        ctx: host.ordinary("/work").map_err(|error| error.to_string())?,
        signal: None,
    };
    let exports = host.exports.clone();
    let event = super::documents::document("session_start").to_string();
    super::block_on(host.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_event(accessor, borrow(handler), event, capabilities)
            .await
    }))
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())??;
    Ok(host)
}

/// Calls the component of the editor the example built.
fn editor(host: &mut Harness, call: Call<'_>) -> Result<Reply, String> {
    let built = host
        .observed()
        .ui
        .editor
        .as_ref()
        .map(|slot| slot.component);
    let component = built.ok_or("no authored editor")?;
    let emitter = host
        .store
        .data_mut()
        .table
        .push(super::bus_host::Emitter { key: 0 })
        .map_err(|error| error.to_string())?;
    let reply = super::block_on(super::component_driver::component_call(
        host,
        Resource::new_borrow(component),
        call,
        &emitter,
    ))?;
    host.store
        .data_mut()
        .table
        .delete(emitter)
        .map_err(|error| error.to_string())?;
    reply
}

/// Types each input into the example's editor and returns the inputs that reached the host
/// custom editor.
fn typed(host: &mut Harness, inputs: &[&str]) -> Result<Vec<String>, String> {
    let from = host.observed().transcript.len();
    for data in inputs {
        assert_eq!(editor(host, Call::Input(data))?, Reply::Done, "{data:?}");
    }
    Ok(host.observed().transcript[from..]
        .iter()
        .filter_map(|line| {
            let (_, data) = line.strip_prefix("widget ")?.split_once(" input ")?;
            serde_json::from_str(data).ok()
        })
        .collect())
}

/// Renders the example's editor at `width`.
fn rendered(host: &mut Harness, width: u32) -> Result<Vec<String>, String> {
    match editor(host, Call::Render(width))? {
        Reply::Lines(lines) => Ok(lines),
        other => Err(format!("expected lines, got {other:?}")),
    }
}

/// Each session start replaces the host's editor with one built on the custom editor.
#[test]
fn modal_editor_session_start_installs_a_custom_editor() -> Result<(), String> {
    let mut host = started()?;
    let requests: Vec<_> = host
        .observed()
        .transcript
        .iter()
        .filter(|line| line.contains("setEditorComponent") || line.starts_with("editor built"))
        .cloned()
        .collect();
    assert_eq!(
        requests,
        ["ui 0 setEditorComponent [true]", "editor built custom=[]"]
    );
    Ok(())
}

/// Escape enters normal mode once and then reaches the editor; `i` returns to insert mode.
#[test]
fn modal_editor_escape_enters_normal_mode_and_then_interrupts() -> Result<(), String> {
    let mut host = started()?;
    assert_eq!(
        typed(&mut host, &["q", ESCAPE, "q", ESCAPE, "i", "q", ESCAPE])?,
        ["q", ESCAPE, "q"]
    );
    Ok(())
}

/// Normal-mode keys send the editor's navigation and deletion input; `a` moves right first.
#[test]
fn modal_editor_normal_keys_send_navigation_input() -> Result<(), String> {
    let mut host = started()?;
    assert_eq!(
        typed(
            &mut host,
            &[ESCAPE, "h", "j", "k", "l", "0", "$", "x", "a", "w"]
        )?,
        [
            "\x1b[D", "\x1b[B", "\x1b[A", "\x1b[C", "\x01", "\x05", "\x1b[3~", "\x1b[C", "w"
        ]
    );
    Ok(())
}

/// Normal mode passes control input and multi-character input on, and drops one printable
/// character, astral and delete included.
#[test]
fn modal_editor_normal_mode_passes_control_input_and_drops_characters() -> Result<(), String> {
    let mut host = started()?;
    assert_eq!(
        typed(
            &mut host,
            &[
                ESCAPE, "\x03", "q", "é", "😀", "\x7f", "\x04", "\x1b[A", "paste"
            ]
        )?,
        ["\x03", "\x04", "\x1b[A", "paste"]
    );
    Ok(())
}

/// The bottom line ends with the mode, truncated to make room, and stays as it is when
/// narrower than the label.
#[test]
fn modal_editor_bottom_line_shows_the_mode() -> Result<(), String> {
    let mut host = started()?;
    typed(&mut host, &["abc"])?;
    let border = "<borderMuted>edit|20".to_owned();
    assert_eq!(
        rendered(&mut host, 20)?,
        [border.clone(), "ab INSERT ".into()]
    );
    typed(&mut host, &[ESCAPE])?;
    assert_eq!(rendered(&mut host, 20)?, [border, "ab NORMAL ".into()]);
    assert_eq!(rendered(&mut host, 4)?[1], " NORMAL ");
    host.observed().actions.value = helpers(7);
    assert_eq!(rendered(&mut host, 20)?[1], "abc");
    Ok(())
}

/// The editor keeps every operation of the custom editor it extends.
#[test]
fn modal_editor_keeps_the_custom_editor_operations() -> Result<(), String> {
    let mut host = started()?;
    let built = host.observed().ui.editor.as_ref().map(|slot| slot.editor);
    let editor = built.ok_or("no authored editor")?;
    let requests = json!([
        { "op": "setText", "text": "a" },
        { "op": "insertTextAtCursor", "text": "!" },
        { "op": "getText" },
        { "op": "getExpandedText" },
        { "op": "borderColor" },
        { "op": "setBorderColor", "role": "borderAccent" },
        { "op": "setPaddingX", "padding": 2.0 },
        { "op": "addToHistory", "text": "past" },
        { "op": "setAutocompleteMaxVisible", "max": 4.0 },
        { "op": "setAutocompleteProvider" },
        { "op": "setOnChange", "callback": "change" },
        { "op": "onChange" },
        { "op": "setOnSubmit", "callback": "submit" },
    ]);
    let mut answers = Vec::new();
    for request in requests.as_array().ok_or("expected requests")? {
        let op = super::editor_host::op(request.clone())?;
        let answer = super::block_on(super::editor_driver::request(&mut host, editor, op));
        answers.push(answer.map_err(|error| error.to_string())??);
    }
    let done = json!("done");
    assert_eq!(
        answers,
        [
            done.clone(),
            done.clone(),
            json!({ "text": "a!" }),
            json!({ "text": "a!" }),
            json!({ "style": "<borderMuted>border" }),
            done.clone(),
            done.clone(),
            done.clone(),
            done.clone(),
            done.clone(),
            done.clone(),
            json!({ "handler": true }),
            done,
        ]
    );
    Ok(())
}
