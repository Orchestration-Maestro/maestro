//! Extension domain exports retain their original type owners.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use maestro_extensions::*;
use maestro_session::Presence;
use serde_json::json;

/// A finished tool call event for `name` with no details or usage.
fn result_event(name: &str) -> ToolResultEvent {
    ToolResultEvent {
        tool_call_id: "id".into(),
        tool_name: name.into(),
        input: json!({}),
        content: vec![],
        details: Presence::Missing,
        is_error: false,
        usage: Presence::Missing,
    }
}

#[test]
fn domain_exports_reuse_owned_types_and_toolkit_handles() {
    let _: Option<SlashCommandInfo> = None::<maestro_resources::SlashCommandInfo>;
    let _: Option<SlashCommandSource> = None::<maestro_resources::SlashCommandSource>;
    let _: Option<SourceInfo> = None::<maestro_resources::SourceInfo>;
    let _: Option<AgentToolResult> = None::<maestro_agent::AgentToolResult>;
    let _: Option<AgentToolUpdateCallback> = None::<maestro_agent::AgentToolUpdateCallback>;
    let _: Option<ToolInfo> = None::<maestro_models::ToolInfo>;
    let _: Option<ToolExecutionMode> = None::<maestro_agent::ToolExecutionMode>;
    let _: Option<ToolRenderResultOptions> = None::<maestro_tools::ToolRenderResultOptions>;
    let _: Option<ExecOptions> = None::<maestro_tools::ExecOptions>;
    let _: Option<ExecResult> = None::<maestro_tools::ExecResult>;
    let action: AppKeybinding = "custom.action".into();
    let manager: KeybindingsManager = maestro_tui::KeybindingsManager::new(
        vec![(
            action.clone(),
            maestro_tui::KeybindingDefinition {
                default_keys: maestro_tui::KeybindingKeys::Single("ctrl+a".into()),
                description: None,
            },
        )],
        vec![],
    );
    let retained = manager.clone();
    assert!(retained.matches("\x01", &action));
    manager.set_user_bindings(vec![(
        action.clone(),
        Some(maestro_tui::KeybindingKeys::Single("ctrl+b".into())),
    )]);
    assert_eq!(retained.get_keys(&action), ["ctrl+b"]);
    assert!(!retained.matches("\x01", &action));
    assert!(retained.matches("\x02", &action));
    let predicates: [fn(&ToolResultEvent) -> bool; 7] = [
        is_bash_tool_result,
        is_read_tool_result,
        is_edit_tool_result,
        is_write_tool_result,
        is_grep_tool_result,
        is_find_tool_result,
        is_ls_tool_result,
    ];
    let names = ["bash", "read", "edit", "write", "grep", "find", "ls"];
    for name in names.into_iter().chain(["custom", " Bash ", ""]) {
        let event = result_event(name);
        for (expected, predicate) in names.iter().zip(predicates) {
            assert_eq!(predicate(&event), *expected == name);
        }
        let call = ToolCallEvent {
            tool_call_id: "id".into(),
            tool_name: name.into(),
            input: json!({}),
        };
        assert!(is_tool_call_event_type(name, &call));
        assert!(!is_tool_call_event_type(&format!("{name}x"), &call));
    }
}
