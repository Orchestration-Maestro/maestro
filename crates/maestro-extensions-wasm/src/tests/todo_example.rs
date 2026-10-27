//! The shipped todo factory exercised through its registered callbacks.
use crate::{Extension, ExtensionAPI, ExtensionFuture};
use serde_json::{Value, json};

#[path = "../../../../examples/extensions/todo.rs"]
mod todo;

/// The same factory exported by the shipped component.
struct Todos;
impl Extension for Todos {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        todo::factory(api)
    }
}

mod host;
use host::Host;

/// Checks both visible text and the independently saved state, including omission of errors.
fn answer(actual: &Value, text: &str, details: &Value) {
    assert_eq!(
        actual,
        &json!({"content":[{"type":"text","text":text}],"details":details})
    );
}

/// Every action preserves authored text, returns snapshots and resets identifiers on clear.
#[test]
fn todo_actions_keep_snapshots_and_validate_identifiers() -> Result<(), String> {
    for component in [false, true] {
        let mut host = Host::new(component)?;
        answer(
            &host.tool(&json!({"action":"list"}))?,
            "No todos",
            &json!({"action":"list","todos":[],"nextId":1.0}),
        );
        for params in [json!({"action":"add"}), json!({"action":"add","text":""})] {
            answer(
                &host.tool(&params)?,
                "Error: text required for add",
                &json!({"action":"add","todos":[],"nextId":1.0,"error":"text required"}),
            );
        }
        let added = host.tool(&json!({"action":"add","text":"  ship 雪  "}))?;
        let open = json!([{"id":1.0,"text":"  ship 雪  ","done":false}]);
        answer(
            &added,
            "Added todo #1:   ship 雪  ",
            &json!({"action":"add","todos":open,"nextId":2.0}),
        );
        answer(
            &host.tool(&json!({"action":"toggle"}))?,
            "Error: id required for toggle",
            &json!({"action":"toggle","todos":open,"nextId":2.0,"error":"id required"}),
        );
        for id in [-1.0, 0.0, 1.5, 99.0] {
            answer(
                &host.tool(&json!({"action":"toggle","id":id}))?,
                &format!("Todo #{id} not found"),
                &json!({"action":"toggle","todos":open,"nextId":2.0,"error":format!("#{id} not found")}),
            );
        }
        completed_and_clear(&mut host, &added, &open)?;
    }
    Ok(())
}

/// Completion does not change earlier snapshots, and clear restarts identifiers.
fn completed_and_clear(host: &mut Host, added: &Value, open: &Value) -> Result<(), String> {
    answer(
        &host.tool(&json!({"action":"toggle","id":1}))?,
        "Todo #1 completed",
        &json!({"action":"toggle","todos":[{"id":1.0,"text":"  ship 雪  ","done":true}],"nextId":2.0}),
    );
    answer(
        &host.tool(&json!({"action":"list"}))?,
        "[x] #1:   ship 雪  ",
        &json!({"action":"list","todos":[{"id":1.0,"text":"  ship 雪  ","done":true}],"nextId":2.0}),
    );
    assert_eq!(
        &added["details"]["todos"], open,
        "earlier result remains detached after toggle"
    );
    answer(
        &host.tool(&json!({"action":"toggle","id":1.0}))?,
        "Todo #1 uncompleted",
        &json!({"action":"toggle","todos":open,"nextId":2.0}),
    );
    answer(
        &host.tool(&json!({"action":"other"}))?,
        "Unknown action: other",
        &json!({"action":"list","todos":open,"nextId":2.0,"error":"unknown action: other"}),
    );
    answer(
        &host.tool(&json!({"action":"clear"}))?,
        "Cleared 1 todos",
        &json!({"action":"clear","todos":[],"nextId":1.0}),
    );
    answer(
        &host.tool(&json!({"action":"add","text":" "}))?,
        "Added todo #1:  ",
        &json!({"action":"add","todos":[{"id":1.0,"text":" ","done":false}],"nextId":2.0}),
    );
    Ok(())
}

/// A persisted tool result whose details are ordinary JSON, not an external state file.
fn entry(id: &str, name: &str, details: Option<Value>) -> Value {
    let mut message = json!({"role":"toolResult","toolCallId":id,"toolName":name,"content":[],"isError":false,"timestamp":1});
    if let Some(details) = details {
        message["details"] = details;
    }
    json!({"type":"message","id":id,"parentId":null,"timestamp":"2026-01-01T00:00:00Z","message":message})
}

/// Session start and tree navigation replace local state with the last current-branch snapshot.
#[test]
fn todo_restores_only_current_branch_on_session_and_tree_changes() -> Result<(), String> {
    for component in [false, true] {
        let mut host = Host::new(component)?;
        host.tool(&json!({"action":"add","text":"local only"}))?;
        let first =
            json!({"action":"add","todos":[{"id":1.0,"text":"first","done":false}],"nextId":2.0});
        let last = json!({"action":"toggle","todos":[{"id":7.0,"text":"latest","done":true}],"nextId":8.0});
        let branch = json!([
            {"type":"custom","id":"custom","parentId":null,"timestamp":"2026-01-01T00:00:00Z","customType":"todo","data":last},
            entry("one", "todo", Some(first.clone())),
            {"type":"message","id":"user","parentId":null,"timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"not state","timestamp":1}},
            entry("two", "todo", Some(last.clone())),
            entry("foreign", "other", Some(first.clone())),
            entry("no-details", "todo", None)
        ]);
        host.event("session_start", &branch)?;
        answer(
            &host.tool(&json!({"action":"list"}))?,
            "[x] #7: latest",
            &json!({"action":"list","todos":last["todos"],"nextId":8.0}),
        );
        let added = host.tool(&json!({"action":"add","text":"next"}))?;
        assert_eq!(added["content"][0]["text"], "Added todo #8: next");
        host.event(
            "session_tree",
            &json!([entry("old", "todo", Some(first.clone()))]),
        )?;
        answer(
            &host.tool(&json!({"action":"list"}))?,
            "[ ] #1: first",
            &json!({"action":"list","todos":first["todos"],"nextId":2.0}),
        );
        host.event("session_tree", &json!([]))?;
        answer(
            &host.tool(&json!({"action":"list"}))?,
            "No todos",
            &json!({"action":"list","todos":[],"nextId":1.0}),
        );
        host.event(
            "session_start",
            &json!([entry(
                "clear",
                "todo",
                Some(json!({"action":"clear","todos":[],"nextId":1.0}))
            )]),
        )?;
        assert_eq!(
            host.tool(&json!({"action":"add","text":"restarted"}))?["content"][0]["text"],
            "Added todo #1: restarted"
        );
    }
    Ok(())
}

/// Supplies literal theme replies; expected rendered text stays independent of this palette.
fn palette(host: &mut Host, pairs: &[(&str, &str)]) {
    host.observe(|o| {
        o.themes
            .theme("dark")
            .replies
            .push(super::theme_host::Reply {
                method: "bold".into(),
                operands: json!(["todo "]),
                answer: json!({"ok":"<bold>todo "}),
            });
        for (role, text) in pairs {
            o.themes
                .theme("dark")
                .replies
                .push(super::theme_host::Reply {
                    method: "fg".into(),
                    operands: json!([role, text]),
                    answer: json!({"ok":format!("<{role}>{text}")}),
                });
        }
    });
}

/// Literal palette operands used by call and result rendering.
fn tool_palette(host: &mut Host) {
    palette(
        host,
        &[
            ("toolTitle", "<bold>todo "),
            ("muted", "add"),
            ("muted", ""),
            ("dim", "\"task\""),
            ("accent", "#2"),
            ("dim", "No todos"),
            ("error", "Error: text required"),
            ("success", "✓ Added "),
            ("accent", "#1"),
            ("muted", "task"),
            ("success", "✓ "),
            ("muted", "Todo #1 completed"),
            ("muted", "Cleared all todos"),
            ("muted", "6 todo(s):"),
            ("success", "✓"),
            ("dim", "○"),
            ("dim", "task"),
            ("dim", "... 1 more"),
            ("accent", "#3"),
            ("accent", "#4"),
            ("accent", "#5"),
            ("accent", "#6"),
        ],
    );
}

/// Expanded lists retain every item; collapsed lists retain five and a remainder.
fn rendered_list(host: &mut Host) -> Result<(), String> {
    for _ in 0..5 {
        host.tool(&json!({"action":"add","text":"task"}))?;
    }
    let list = host.tool(&json!({"action":"list"}))?;
    let first_five = "<muted>6 todo(s):\n<success>✓ <accent>#1 <dim>task\n<dim>○ <accent>#2 <muted>task\n<dim>○ <accent>#3 <muted>task\n<dim>○ <accent>#4 <muted>task\n<dim>○ <accent>#5 <muted>task";
    assert_eq!(
        host.render("result", &json!(null), Some(list.clone()), false)?,
        format!("{first_five}\n<dim>... 1 more")
    );
    assert_eq!(
        host.render("result", &json!(null), Some(list), true)?,
        format!("{first_five}\n<dim>○ <accent>#6 <muted>task")
    );
    Ok(())
}

/// Call rows, all result actions, errors, fallback content and collapsed lists keep source text.
#[test]
fn todo_renderers_preserve_call_and_result_output() -> Result<(), String> {
    for component in [false, true] {
        let mut host = Host::new(component)?;
        tool_palette(&mut host);
        assert_eq!(
            host.render(
                "call",
                &json!({"action":"add","text":"task","id":2}),
                None,
                false
            )?,
            "<toolTitle><bold>todo <muted>add <dim>\"task\" <accent>#2"
        );
        assert_eq!(
            host.render("call", &json!(null), None, false)?,
            "<toolTitle><bold>todo <muted>"
        );
        for result in [
            json!({"content":[{"type":"text","text":"raw"}]}),
            json!({"content":[],"details":null}),
        ] {
            let expected = if result["content"].as_array().unwrap().is_empty() {
                ""
            } else {
                "raw"
            };
            assert_eq!(
                host.render("result", &json!(null), Some(result), false)?,
                expected
            );
        }
        let empty = host.tool(&json!({"action":"list"}))?;
        assert_eq!(
            host.render("result", &json!(null), Some(empty), false)?,
            "<dim>No todos"
        );
        let error = host.tool(&json!({"action":"add"}))?;
        assert_eq!(
            host.render("result", &json!(null), Some(error), false)?,
            "<error>Error: text required"
        );
        let added = host.tool(&json!({"action":"add","text":"task"}))?;
        assert_eq!(
            host.render("result", &json!(null), Some(added), false)?,
            "<success>✓ Added <accent>#1 <muted>task"
        );
        let toggled = host.tool(&json!({"action":"toggle","id":1}))?;
        assert_eq!(
            host.render("result", &json!(null), Some(toggled), true)?,
            "<success>✓ <muted>Todo #1 completed"
        );
        rendered_list(&mut host)?;
        let cleared = host.tool(&json!({"action":"clear"}))?;
        assert_eq!(
            host.render("result", &json!(null), Some(cleared), false)?,
            "<success>✓ <muted>Cleared all todos"
        );
    }
    Ok(())
}

/// Exact helper requests with canned answers, including ignored input and both close keys.
fn helper_replies() -> Value {
    let mut replies = serde_json::Map::new();
    for (input, output) in [
        ("─── Todos ──────────", "─── Todos ──────────"),
        (
            "  No todos yet. Ask the agent to add some!",
            "  No todos yet. A...",
        ),
        ("  Press Escape to close", "  Press Escape to..."),
        ("  1/2 completed", "  1/2 completed"),
        ("  ✓ #1 done", "  ✓ #1 done"),
        ("  ○ #2 open", "  ○ #2 open"),
    ] {
        replies.insert(
            format!("truncate_to_width {}", json!([input, 20, "...", false])),
            json!(output),
        );
    }
    for (data, key, matched) in [
        ("x", "escape", false),
        ("x", "ctrl+c", false),
        ("\x1b", "escape", true),
        ("\x03", "escape", false),
        ("\x03", "ctrl+c", true),
    ] {
        replies.insert(
            format!("keys.matches_key {}", json!([data, key])),
            json!(matched),
        );
    }
    Value::Object(replies)
}

/// Supplies width-helper answers for exact operands, without implementing terminal algorithms.
fn list_ui(host: &mut Host, populated: bool, close: &str) {
    palette(
        host,
        &[
            ("borderMuted", "───"),
            ("accent", " Todos "),
            ("borderMuted", "──────────"),
            ("dim", "No todos yet. Ask the agent to add some!"),
            ("dim", "Press Escape to close"),
            ("muted", "1/2 completed"),
            ("success", "✓"),
            ("accent", "#1"),
            ("dim", "done"),
            ("dim", "○"),
            ("accent", "#2"),
            ("text", "open"),
        ],
    );
    let rows: Vec<String> = if populated {
        vec![
            "",
            "─── Todos ──────────",
            "",
            "  1/2 completed",
            "",
            "  ✓ #1 done",
            "  ○ #2 open",
            "",
            "  Press Escape to...",
            "",
        ]
    } else {
        vec![
            "",
            "─── Todos ──────────",
            "",
            "  No todos yet. A...",
            "",
            "  Press Escape to...",
            "",
        ]
    }
    .into_iter()
    .map(str::to_owned)
    .collect();
    host.observe(|o| {
        o.catalog.data.has_ui = true;
        o.actions.value = helper_replies();
        for reply in &mut o.themes.theme("dark").replies {
            if reply.method == "fg" {
                reply.answer = json!({"ok":reply.operands[1]});
            }
        }
        o.ui.custom_script = Some(super::custom_host::Script {
            width: None,
            inputs: vec!["x".into(), close.into()],
            frames: vec![rows; 3],
            ..Default::default()
        });
    });
}

/// Headless commands notify; interactive lists show the snapshot and close on either key.
#[test]
fn todo_command_handles_headless_and_interactive_lists() -> Result<(), String> {
    for component in [false, true] {
        let mut host = Host::new(component)?;
        host.observe(|o| {
            assert_eq!(
                o.registrations.commands,
                [(
                    "todos".into(),
                    Some("Show all todos on the current branch".into()),
                    false
                )]
            );
        });
        host.command()?;
        host.observe(|o| {
            assert_eq!(
                super::example_support::observations(&o.transcript),
                [super::example_support::Observation::Notify {
                    message: "/todos requires interactive mode".into(),
                    kind: Some("error".into())
                }]
            );
        });
        host.observe(|o| o.ui.reject("notify", "notification refused"));
        assert_eq!(host.command(), Err("notification refused".into()));
        // Use a fresh host so a deliberately rejected UI does not affect interactive calls.
        drop(host);
        let mut host = Host::new(component)?;
        list_ui(&mut host, false, "\x1b");
        host.command()?;
        host.tool(&json!({"action":"add","text":"done"}))?;
        host.tool(&json!({"action":"toggle","id":1}))?;
        host.tool(&json!({"action":"add","text":"open"}))?;
        list_ui(&mut host, true, "\x03");
        host.command()?;
        host.observe(|o| o.ui.reject("custom", "view refused"));
        assert_eq!(host.command(), Err("view refused".into()));
    }
    Ok(())
}

/// Numeric identifiers keep the JSON boundary's exponent and signed-zero display.
#[test]
fn todo_identifier_text_uses_json_number_format() -> Result<(), String> {
    for component in [false, true] {
        let mut host = Host::new(component)?;
        palette(
            &mut host,
            &[
                ("toolTitle", "<bold>todo "),
                ("muted", "toggle"),
                ("accent", "#1e+21"),
                ("accent", "#1e-7"),
                ("accent", "#0"),
            ],
        );
        for (id, text) in [(1e21, "1e+21"), (1e-7, "1e-7"), (-0.0, "0")] {
            let params = json!({"action":"toggle","id":id});
            answer(
                &host.tool(&params)?,
                &format!("Todo #{text} not found"),
                &json!({"action":"toggle","todos":[],"nextId":1.0,"error":format!("#{text} not found")}),
            );
            assert_eq!(
                host.render("call", &params, None, false)?,
                format!("<toolTitle><bold>todo <muted>toggle <accent>#{text}")
            );
        }
    }
    Ok(())
}
