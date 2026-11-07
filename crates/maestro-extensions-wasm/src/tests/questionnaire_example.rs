//! The shipped questionnaire exercised through its built component and retained UI callbacks.
use super::{
    example_support::{invoke_tool_on_component, result_value},
    host::{Harness, borrow},
};
use serde_json::{Value, json};

#[path = "../../../../examples/extensions/questionnaire.rs"]
mod questionnaire;

/// The factory exported by the component, also run through the in-process adapter.
struct Questionnaire;
impl crate::Extension for Questionnaire {
    fn load(api: crate::ExtensionAPI) -> crate::ExtensionFuture<'static, ()> {
        questionnaire::factory(api)
    }
}
/// Replays the same UI scripts through native author code at the registered tool seam.
fn replay_native(rows: &[Value]) -> Result<(), String> {
    let (host, key, exports) = super::block_on(
        super::example_support::registered::<Questionnaire>("tool questionnaire"),
    )?;
    crate::component_adapter::install(std::rc::Rc::new(crate::component_adapter::Builder(
        host.clone(),
    )));
    host.observe().actions.value = corpus()["replies"].clone();
    host.observe().themes.theme("dark").replies =
        serde_json::from_value(corpus()["themes"].clone()).unwrap();
    for row in rows {
        host.observe().catalog.data.has_ui = row["ui"].as_bool().unwrap();
        host.observe().ui.custom_script = Some(super::custom_host::Script {
            width: None,
            inputs: serde_json::from_value(row["inputs"].clone()).unwrap(),
            frames: serde_json::from_value(row["frames"].clone()).unwrap(),
            ..Default::default()
        });
        let actual = super::block_on(exports.invoke_tool(
            key,
            crate::bindings::exports::maestro::extension::guest::ToolInvocation {
                tool_call_id: "controlled".into(),
                params: json!({"questions":row["questions"]}).to_string(),
            },
            crate::component_adapter::ToolCapabilities {
                ctx: host.context("/work"),
                signal: None,
                update: None,
            },
        ))?;
        assert_eq!(result_value(&actual), row["result"], "{}", row["name"]);
    }
    let identities = host.observed().identities();
    for key in identities {
        crate::component_adapter::release(key);
    }
    super::example_support::release_all(
        &host,
        &[
            "render-call questionnaire",
            "render-result questionnaire",
            "tool questionnaire",
        ],
    );
    Ok(())
}

/// Independent reference executions with literal host helper and theme replies.
fn corpus() -> Value {
    serde_json::from_str(include_str!("questionnaire_expected.json")).unwrap()
}
/// Instantiates the shipped component with controlled, operand-specific host replies.
fn host() -> Result<Harness, String> {
    let mut host = Harness::example("questionnaire")?;
    let corpus = corpus();
    host.observed().actions.value = corpus["replies"].clone();
    host.observed().themes.theme("dark").replies =
        serde_json::from_value(corpus["themes"].clone()).unwrap();
    Ok(host)
}
/// Invokes the registered tool, serving its UI script without a provider or real terminal.
fn execute(host: &mut Harness, row: &Value) -> Result<Value, String> {
    host.observed().catalog.data.has_ui = row["ui"].as_bool().unwrap();
    host.observed().ui.custom_script = Some(super::custom_host::Script {
        width: None,
        inputs: serde_json::from_value(row["inputs"].clone()).unwrap(),
        frames: serde_json::from_value(row["frames"].clone()).unwrap(),
        ..Default::default()
    });
    let key = host
        .callback("tool questionnaire")
        .map_err(|e| e.to_string())?;
    let caps = super::host_family::guest::ToolCapabilities {
        ctx: host.ordinary("/work").map_err(|e| e.to_string())?,
        signal: None,
        update: None,
    };
    let encoded =
        invoke_tool_on_component(host, key, &json!({"questions":row["questions"]}), caps)?;
    Ok(result_value(&encoded))
}
/// Replays a behavior group and compares every frame and the complete structured answer.
fn replay(group: &str) -> Result<(), String> {
    let rows: Vec<Value> = corpus()["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["group"] == group)
        .cloned()
        .collect();
    replay_native(&rows)?;
    let mut host = host()?;
    for row in &rows {
        assert_eq!(execute(&mut host, row)?, row["result"], "{}", row["name"]);
    }
    Ok(())
}
/// Extra schema-permitted question and option fields survive in structured details.
#[test]
fn questionnaire_retains_supplied_question_metadata() -> Result<(), String> {
    replay("metadata")
}
/// Headless and empty input return cancelled details without opening custom UI.
#[test]
fn questionnaire_headless_and_empty_return_errors() -> Result<(), String> {
    replay("headless")
}
/// Option navigation clamps and one question submits the selected one-based answer immediately.
#[test]
fn questionnaire_selected_answer_finishes_single_question() -> Result<(), String> {
    replay("selected")
}
/// Escape discards editor text; resubmission uses shared whitespace and the blank fallback.
#[test]
fn questionnaire_custom_answers_use_inline_editor() -> Result<(), String> {
    replay("custom")
}
/// An empty identifier is a string value, not an absent inline-editor question.
#[test]
fn questionnaire_custom_answer_accepts_empty_identifier() -> Result<(), String> {
    let mut row = corpus()["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "blank")
        .unwrap()
        .clone();
    row["name"] = json!("empty-identifier");
    row["questions"][0]["id"] = json!("");
    row["result"]["details"]["questions"][0]["id"] = json!("");
    row["result"]["details"]["answers"][0]["id"] = json!("");
    replay_native(std::slice::from_ref(&row))?;
    assert_eq!(execute(&mut host()?, &row)?, row["result"]);
    Ok(())
}
/// Tabs wrap, unanswered submission stays open, and revising answers retains insertion order.
#[test]
fn questionnaire_tabs_require_all_answers_and_allow_revisions() -> Result<(), String> {
    replay("tabs")
}
/// Cancellation works from a question and Submit, retaining partial answers.
#[test]
fn questionnaire_cancellation_retains_partial_answers() -> Result<(), String> {
    replay("cancel")
}

/// A question with no selectable options ignores Enter rather than dereferencing a missing option.
#[test]
fn questionnaire_empty_option_list_remains_cancellable() -> Result<(), String> {
    let mut host = host()?;
    let mut row = corpus()["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "partial")
        .unwrap()
        .clone();
    row["inputs"]
        .as_array_mut()
        .unwrap()
        .insert(1, json!("enter"));
    let unchanged = row["frames"][1].clone();
    row["frames"].as_array_mut().unwrap().insert(2, unchanged);
    replay_native(std::slice::from_ref(&row))?;
    assert_eq!(execute(&mut host, &row)?, row["result"]);
    Ok(())
}

/// Renders a registered tool slot through the component export and then its returned component.
fn render(
    host: &mut Harness,
    slot: &str,
    args: &Value,
    result: Option<&Value>,
) -> Result<Vec<String>, String> {
    use crate::bindings::host_side::maestro::extension::host as wire;
    let key = host
        .callback(&format!("render-{slot} questionnaire"))
        .map_err(|e| e.to_string())?;
    let info = super::ui_host::render_info(&json!({"toolCallId":"q", "args":args,"cwd":"/work",
        "executionStarted":true,"argsComplete":true,"isPartial":false,
        "expanded":false,"showImages":false,"isError":false}));
    let render = wire::ToolRender {
        slot: if slot == "call" {
            wire::ToolRenderSlot::Call
        } else {
            wire::ToolRenderSlot::Result
        },
        row: None,
        args: Some(args.to_string()),
        info,
        options: Some(wire::ToolRenderResultOptions {
            expanded: false,
            is_partial: false,
        }),
        result: result.map(ToString::to_string),
    };
    invoke_render(host, key, render)
}
/// Invokes the renderer with independently owned theme, invalidator and emitter capabilities.
fn invoke_render(
    host: &mut Harness,
    key: u32,
    render: crate::bindings::host_side::maestro::extension::host::ToolRender,
) -> Result<Vec<String>, String> {
    let state = host.store.data_mut();
    let theme = state
        .push(super::renderer_host::Theme("dark".into()))
        .map_err(|e| e.to_string())?;
    let invalidator = state
        .push(super::ui_host::Invalidator("q".into()))
        .map_err(|e| e.to_string())?;
    let emitter = state
        .table
        .push(super::bus_host::Emitter { key: 0 })
        .map_err(|e| e.to_string())?;
    let rendered = super::block_on(host.exports.func_invoke_tool_render().call_async(
        &mut host.store,
        (
            borrow(key),
            render,
            theme,
            invalidator,
            None,
            wasmtime::component::Resource::new_borrow(emitter.rep()),
        ),
    ))
    .map_err(|e| e.to_string())?
    .0?;
    let component = rendered.component?;
    let lines = component_lines(host, component.rep(), emitter.rep());
    super::block_on(host.release(component.rep())).map_err(|e| e.to_string())?;
    if let Some(row) = rendered.row {
        super::block_on(host.release(row.rep())).map_err(|e| e.to_string())?;
    }
    host.store
        .data_mut()
        .table
        .delete(emitter)
        .map_err(|e| e.to_string())?;
    lines
}
/// Reads the retained component's rows through its generated export.
fn component_lines(
    host: &mut Harness,
    component: u32,
    emitter: u32,
) -> Result<Vec<String>, String> {
    let function = host
        .exports
        .func_component_render()
        .func()
        .typed::<(
            wasmtime::component::Resource<super::host::Identity>,
            u32,
            wasmtime::component::Resource<super::bus_host::Emitter>,
        ), (Result<Vec<String>, String>,)>(&host.store)
        .map_err(|e| e.to_string())?;
    let (lines,) = super::block_on(host.store.run_concurrent(async |accessor| {
        function
            .call_concurrent(
                accessor,
                (
                    borrow(component),
                    80,
                    wasmtime::component::Resource::new_borrow(emitter),
                ),
            )
            .await
    }))
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    lines
}
/// Both renderers preserve counts, supplied labels, cancellation and custom/selected summaries.
#[test]
fn questionnaire_renderers_show_call_and_answer_summaries() -> Result<(), String> {
    let mut host = host()?;
    for row in corpus()["rows"].as_array().unwrap() {
        let args = json!({"questions":row["questions"]});
        assert_eq!(
            render(&mut host, "call", &args, None)?,
            [format!("text {} 0 0|80", row["call"].as_str().unwrap())]
        );
        assert_eq!(
            render(&mut host, "result", &args, Some(&row["result"]))?,
            [format!("text {} 0 0|80", row["rendered"].as_str().unwrap())]
        );
    }
    assert_eq!(
        render(
            &mut host,
            "result",
            &json!({}),
            Some(&json!({"content":[{"type":"text","text":"fallback"}]}))
        )?,
        ["text fallback 0 0|80"]
    );
    Ok(())
}
