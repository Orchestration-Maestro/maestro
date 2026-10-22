//! The tool registry, active-tool selection and the rebuilt system prompt.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, case, harness, on_task_set, sdk_tools,
};
use maestro_app::resources::system_prompt::build_system_prompt;
use maestro_models::system_prompt::BuildSystemPromptOptions;
use maestro_session::Presence;
use serde_json::{Value, json};

/// The prompt the recorded options and this session's directory build.
fn expected_prompt(cwd: &str, options: &Value) -> String {
    let text = |key: &str| {
        options[key]
            .as_str()
            .map_or(Presence::Missing, |text| Presence::Present(text.to_owned()))
    };
    build_system_prompt(&BuildSystemPromptOptions {
        cwd: cwd.to_owned(),
        skills: Presence::Present(Vec::new()),
        context_files: Presence::Present(Vec::new()),
        custom_prompt: text("customPrompt"),
        append_system_prompt: text("appendSystemPrompt"),
        selected_tools: Presence::Present(
            serde_json::from_value(options["selectedTools"].clone()).unwrap(),
        ),
        tool_snippets: Presence::Present(
            serde_json::from_value(options["toolSnippets"].clone()).unwrap(),
        ),
        prompt_guidelines: Presence::Present(
            serde_json::from_value(options["promptGuidelines"].clone()).unwrap(),
        ),
    })
    .unwrap()
}

/// Selects each recorded call (the first selects every registered tool, as the session's
/// first activation does) and compares the active names and the rebuilt prompt.
fn check_steps(h: &Harness, name: &str) {
    let session = &h.session;
    let expected = case("tool_selection", name);
    for step in expected["steps"].as_array().unwrap() {
        let call: Vec<String> = match &step["call"] {
            Value::Null => session
                .get_all_tools()
                .into_iter()
                .map(|tool| tool.name)
                .collect(),
            call => serde_json::from_value(call.clone()).unwrap(),
        };
        session.set_active_tools_by_name(&call).unwrap();
        assert_eq!(
            json!(session.get_active_tool_names()),
            step["activeToolNames"]
        );
        assert_eq!(
            session.system_prompt(),
            expected_prompt(&h.cwd, &step["options"]),
            "{call:?}"
        );
    }
}

/// A session over the recorded tools with the loader prompt texts.
async fn tool_session(system_prompt: Option<&'static str>, append: &[&str]) -> Harness {
    let h = harness(HarnessOptions {
        models: vec![("plain", false)],
        system_prompt,
        append_system_prompt: append.iter().map(|text| (*text).to_owned()).collect(),
        ..HarnessOptions::default()
    })
    .await;
    h.session.set_tool_definitions(sdk_tools());
    h
}

#[test]
fn supplied_tool_order_skips_unknown_names() {
    on_task_set(async {
        let h = tool_session(None, &[]).await;
        check_steps(&h, "supplied order filters unknown names");
    });
}

#[test]
fn loader_prompt_and_appended_sections_rebuild_prompt() {
    on_task_set(async {
        let h = tool_session(Some("Custom base"), &["one", "two"]).await;
        check_steps(&h, "loader prompt and appended sections join");
    });
}

#[test]
fn registry_lists_tools_with_sources() {
    let expected = case("tool_selection", "supplied order filters unknown names");
    on_task_set(async {
        let h = tool_session(None, &[]).await;
        assert_eq!(json!(h.session.get_all_tools()), expected["registry"]);
        let alpha = h
            .session
            .get_tool_definition("alpha")
            .map(|tool| tool.metadata().definition.name);
        assert_eq!(
            json!({"alpha": alpha, "missing": h.session.get_tool_definition("missing").map(|_| ())}),
            expected["definitions"]
        );
    });
}
