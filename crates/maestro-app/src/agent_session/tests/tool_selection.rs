//! The tool registry, active-tool selection and the rebuilt system prompt, with the SDK
//! tools replaced before each refresh as the recorded cases did.
use crate::agent_session::tools::RefreshToolRegistryOptions;
use crate::resources::instructions::build_system_prompt;
use crate::tests::session_harness::{
    Harness, HarnessOptions, case, harness, on_task_set, sdk_definition, sdk_tools, tool_definition,
};
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

/// Replaces the SDK tools with the recorded ones and refreshes the registry with the
/// recorded options.
fn refresh(h: &Harness, refresh: &Value) {
    let text = |value: &Value| value.as_str().map(str::to_owned);
    let mut tools = Vec::new();
    for spec in refresh["tools"].as_array().unwrap() {
        let name = spec["name"].as_str().unwrap();
        let guidelines: Option<Vec<String>> =
            serde_json::from_value(spec["promptGuidelines"].clone()).unwrap();
        let guidelines: Option<Vec<&str>> = guidelines
            .as_ref()
            .map(|lines| lines.iter().map(String::as_str).collect());
        let snippet = text(&spec["promptSnippet"]);
        tools.push(sdk_definition(tool_definition(
            name,
            snippet.as_deref(),
            guidelines.as_deref(),
        )));
    }
    h.session.state.borrow_mut().custom_tools = tools;
    let options = RefreshToolRegistryOptions {
        active_tool_names: serde_json::from_value(refresh["activeToolNames"].clone()).unwrap(),
        include_all_extension_tools: refresh["includeAllExtensionTools"].as_bool().unwrap(),
    };
    h.session.refresh_tool_registry(options).unwrap();
}

/// Runs each recorded refresh or selection and compares the active names, the rebuilt
/// prompt and the registry.
fn check_case(h: &Harness, name: &str) {
    let session = &h.session;
    let expected = case("tool_selection", name);
    for step in expected["steps"].as_array().unwrap() {
        match (step.get("select"), step.get("refresh")) {
            (Some(names), None) => session
                .set_active_tools_by_name(
                    &serde_json::from_value::<Vec<String>>(names.clone()).unwrap(),
                )
                .unwrap(),
            (None, Some(registry)) => refresh(h, registry),
            _ => panic!("a step is either a selection or a refresh: {step}"),
        }
        assert_eq!(
            json!(session.get_active_tool_names()),
            step["activeToolNames"]
        );
        let built = session.system_prompt() == expected_prompt(&h.cwd, &step["options"]);
        assert_eq!(json!(built), step["systemPromptIsBase"], "{step}");
    }
    assert_eq!(json!(session.get_all_tools()), expected["registry"]);
    let alpha = session
        .get_tool_definition("alpha")
        .map(|tool| tool.metadata().definition.name);
    assert_eq!(
        json!({"alpha": alpha, "missing": session.get_tool_definition("missing").map(|_| ())}),
        expected["definitions"]
    );
}

/// A session with the loader prompt texts, no built-in tools and the recorded SDK tools.
async fn tool_session(system_prompt: Option<&'static str>, append: &[&str]) -> Harness {
    harness(HarnessOptions {
        models: vec![("plain", false)],
        system_prompt,
        append_system_prompt: append.iter().map(|text| (*text).to_owned()).collect(),
        custom_tools: sdk_tools(),
        base_tools_override: Some(indexmap::IndexMap::new()),
        ..HarnessOptions::default()
    })
    .await
}

#[test]
fn supplied_tool_order_skips_unknown_names() {
    on_task_set(|_| async {
        let h = tool_session(None, &[]).await;
        check_case(&h, "supplied order filters unknown names");
    });
}

#[test]
fn loader_prompt_and_appended_sections_rebuild_prompt() {
    on_task_set(|_| async {
        let h = tool_session(Some("Custom base"), &["one", "two"]).await;
        check_case(&h, "loader prompt and appended sections join");
    });
}

#[test]
fn later_refresh_reactivates_tools_and_rebuilds_prompt() {
    on_task_set(|_| async {
        let h = tool_session(None, &[]).await;
        check_case(
            &h,
            "a later refresh reactivates tools and rebuilds the prompt",
        );
    });
}
