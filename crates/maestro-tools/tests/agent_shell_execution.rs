//! Agent shell definition and execution through supplied operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::runtime;
use maestro_tools::{BashToolOptions, create_bash_tool_definition};
use serde_json::json;

#[test]
fn bash_definition_keeps_schema_and_prompt() {
    let tool = create_bash_tool_definition::<()>(
        "/work".into(),
        "/bin".into(),
        BashToolOptions::default(),
    );
    assert_eq!(
        json!({"name":tool.definition.name, "label":tool.label, "description":tool.definition.description, "promptSnippet":tool.prompt_snippet, "parameters":tool.definition.parameters}),
        agent_shell_support::fixture().metadata
    );
    cases(&["metadata_probe"]);
    assert!(tool.prompt_guidelines.is_none());
    assert!(tool.execution_mode.is_none());
    assert!(tool.prepare_arguments.is_none());
    assert!(tool.render_call.is_some());
    assert!(tool.render_result.is_some());
    assert!(tool.render_shell.is_none());
}

use crate::agent_shell_support;
use agent_shell_support::cases;

#[test]
fn bash_prefix_is_literal_and_precedes_spawn_hook() {
    cases(&[
        "prefix_empty",
        "prefix_whitespace",
        "prefix_newline",
        "hook_replaces",
        "hook_empty_env",
    ]);
}

#[test]
fn bash_hook_failure_precedes_updates_and_execution() {
    cases(&["hook_failure"]);
    agent_shell_support::hook_sequence();
}

#[test]
fn bash_operations_receive_signal_timeout_and_environment() {
    cases(&[
        "supplied_ops_bypass_bad_shell",
        "signal_absent",
        "signal_preaborted",
        "signal_live_abort_success",
        "timeout_0",
        "timeout_-1",
        "timeout_0.025",
        "timeout_1e-7",
        "timeout_1e+21",
    ]);
    #[cfg(unix)]
    agent_shell_support::environment_sequence();
}

#[test]
fn bash_empty_success_preserves_optional_details() {
    cases(&["empty_success", "empty_chunk", "whitespace_success"]);
}

#[test]
fn bash_status_preserves_null_and_numeric_exit_values() {
    cases(&[
        "exit_0",
        "exit_1",
        "exit_2",
        "exit_3",
        "exit_4",
        "exit_5",
        "exit_6",
        "nonzero_output",
        "nonzero_truncated",
        "missing_status_output",
        "missing_status_truncated",
    ]);
}

#[test]
fn bash_stream_keeps_raw_bytes_and_decodes_fragmented_utf8() {
    cases(&[
        "raw_controls",
        "utf8_split_euro",
        "utf8_split_supplementary",
        "utf8_invalid",
    ]);
    runtime::run_paused(async {
        let raw = [vec![0xff, 0x80], vec![b'A'; 51201]].concat();
        let bytes = raw.clone();
        let ops = agent_shell_support::Immediate(std::sync::Arc::new(move |_, _, options| {
            (options.on_data)(&bytes);
            Ok(maestro_tools::BashExecResult {
                exit_code: Some(0.0),
            })
        }));
        let tool = maestro_tools::create_bash_tool(
            "/work".into(),
            "/bin".into(),
            BashToolOptions {
                operations: Some(std::sync::Arc::new(ops)),
                ..BashToolOptions::default()
            },
        );
        let mut value = json!({"value":agent_shell_support::result_value((tool.execute)("call".into(), json!({"command":"literal"}), None, None).await.unwrap())});
        assert_eq!(agent_shell_support::normalize(&mut value, &mut []), [raw]);
    });
}

#[test]
fn bash_limits_select_tail_and_complete_raw_artifact() {
    cases(&[
        "bytes_exact",
        "bytes_over_partial",
        "bytes_over_complete_lines",
        "bytes_over_few_lines",
        "multibyte_partial",
        "lines_exact",
        "lines_over",
        "line_artifact",
    ]);
}

#[test]
fn bash_raw_only_spill_is_not_final_truncation() {
    cases(&["bom_raw_artifact"]);
}

#[test]
fn bash_abort_timeout_and_failure_keep_distinct_output() {
    cases(&[
        "empty_aborted",
        "output_aborted",
        "truncated_aborted",
        "empty_timeout:5",
        "output_timeout:5",
        "truncated_timeout:5",
        "empty_disk offline",
        "output_disk offline",
        "truncated_disk offline",
        "timeout_fraction",
    ]);
}

#[test]
fn bash_wrapped_factory_preserves_results_and_updates() {
    cases(&["wrapped_result"]);
}
