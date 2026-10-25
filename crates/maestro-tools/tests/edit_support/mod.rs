//! Recorded edit inputs and expectations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
pub use crate::write_support::Row;
use serde::Deserialize;
use std::collections::BTreeMap;
/// The recorded groups.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Inputs and complete expected outputs grouped by witness.
    tests: BTreeMap<String, Vec<Row>>,
}
/// The exact groups owned by the delivery witnesses.
const GROUPS: [&str; 26] = [
    "edit_definition_keeps_schema_and_prompt_metadata",
    "edit_preparation_preserves_nonobjects",
    "edit_preparation_keeps_complete_arguments",
    "edit_preparation_appends_top_level_pair",
    "edit_preparation_keeps_partial_pairs",
    "edit_preparation_parses_only_array_json",
    "edit_arguments_use_preparation_before_shared_validation",
    "edit_execution_returns_written_bytes_and_numbered_details",
    "edit_execution_matches_one_original_snapshot",
    "edit_execution_keeps_compact_distant_hunks",
    "edit_matching_failures_leave_original_bytes",
    "edit_execution_uses_shared_fuzzy_comparison_base",
    "edit_execution_preserves_unrelated_exact_text",
    "edit_execution_restores_bom_and_first_newline_style",
    "edit_execution_decodes_bytes_lossily",
    "edit_execution_resolves_authored_paths",
    "edit_execution_requires_read_and_write_access",
    "edit_execution_inherits_matching_corrections",
    "edit_execution_binds_expanded_paths_before_polling",
    "edit_operation_failures_stop_later_stages",
    "edit_native_failures_do_not_create_targets",
    "edit_abort_drops_each_pending_stage_and_releases_queue",
    "edit_synchronous_abort_wins_over_stage_result",
    "edit_preaborted_call_starts_no_operations",
    "edit_parallel_calls_preserve_both_changes",
    "edit_and_write_use_one_queue",
];
/// Load a nonempty group.
pub fn rows(test: &str) -> Vec<Row> {
    let mut corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/edit-execution-cases.json")).unwrap();
    assert_eq!(
        corpus
            .tests
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        GROUPS.into_iter().collect()
    );
    let mut seen = std::collections::HashSet::new();
    for (group, rows) in &corpus.tests {
        let operation = if group.starts_with("edit_preparation_") {
            "prepare"
        } else {
            group.as_str()
        };
        for row in rows {
            assert!(
                seen.insert((operation, serde_json::to_string(&row.input).unwrap())),
                "duplicate operation/input"
            );
        }
    }
    let rows = corpus.tests.remove(test).unwrap();
    assert!(!rows.is_empty());
    rows
}

use crate::{
    read_support::content_json,
    write_support::{Journal, Mode, ScriptedWrite},
};
use maestro_agent::{AgentTool, AgentToolResult};
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo};
use maestro_tools::{
    EditOperations, EditToolDetails, EditToolInput, EditToolOptions, ToolDefinition,
    create_edit_tool_definition,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
/// A typed edit definition.
pub type Tool = ToolDefinition<(), EditToolInput, EditToolDetails, maestro_tools::EditRenderState>;
/// File stages for a supplied edit adapter.
pub struct Operations {
    /// Shared scripted behavior, including pending-future drop witnesses.
    pub script: ScriptedWrite,
    /// Actual start and successful settlement events.
    pub events: Journal,
    /// Stage policies, in access/read/write order.
    pub modes: [Mode; 3],
    /// Current bytes observed by the read and changed by the write.
    pub bytes: Mutex<Vec<u8>>,
}
impl Operations {
    /// An adapter holding `bytes` with successful stages.
    pub fn new(bytes: &[u8]) -> Self {
        Self {
            events: Journal::default(),
            script: ScriptedWrite::new(&Journal::default()),
            modes: [Mode::Succeed, Mode::Succeed, Mode::Succeed],
            bytes: Mutex::new(bytes.to_vec()),
        }
    }
    /// Replace a stage's behavior.
    pub fn at(mut self, stage: &str, mode: Mode, signal: Option<&Cancellation>) -> Self {
        self.modes[index(stage)] = mode;
        if let Some(signal) = signal {
            self.script = self.script.signal(signal);
        }
        self
    }
    /// Record the real start and settlement around the reused scripted future.
    fn stage(
        &self,
        name: &str,
        operands: &[&str],
        mode: &Mode,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        self.events.push(format!("{name}:start"));
        let step = self.script.step(name, operands, mode);
        let name = name.to_owned();
        let events = self.events.clone();
        Box::pin(async move {
            step.await?;
            events.push(format!("{name}:end"));
            Ok(())
        })
    }
}
impl EditOperations for Operations {
    fn access(&self, path: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        self.stage("access", &[&path], &self.modes[0])
    }
    fn read_file(&self, path: String) -> BoxFuture<Result<Vec<u8>, DiagnosticErrorInfo>> {
        let step = self.stage("read", &[&path], &self.modes[1]);
        let bytes = self.bytes.lock().unwrap().clone();
        Box::pin(async move {
            step.await?;
            Ok(bytes)
        })
    }
    fn write_file(
        &self,
        path: String,
        content: String,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        let step = self.stage("write", &[&path, &content], &self.modes[2]);
        if matches!(self.modes[2], Mode::Succeed | Mode::AbortThenSucceed) {
            *self.bytes.lock().unwrap() = content.into_bytes();
        }
        step
    }
}
/// Index of a file stage.
pub fn index(stage: &str) -> usize {
    match stage {
        "access" => 0,
        "read" => 1,
        "write" => 2,
        _ => panic!("unknown stage"),
    }
}
/// A definition over supplied operations.
pub fn tool(ops: &Arc<impl EditOperations + 'static>, cwd: &str) -> Tool {
    create_edit_tool_definition(
        cwd.into(),
        EditToolOptions {
            operations: Some(ops.clone()),
        },
    )
}
/// The wrapped agent tool over supplied operations.
pub fn agent_tool(ops: &Arc<Operations>, cwd: &str) -> AgentTool {
    maestro_tools::create_edit_tool(
        cwd.into(),
        EditToolOptions {
            operations: Some(ops.clone()),
        },
    )
}
/// Start a typed execution, adopting its queue position immediately.
pub fn start(
    tool: &Tool,
    input: EditToolInput,
    signal: Option<Cancellation>,
) -> BoxFuture<Result<AgentToolResult<EditToolDetails>, DiagnosticErrorInfo>> {
    (tool.execute)("id".into(), input, signal, None, None)
}
/// One replacement.
pub fn input(path: &str, old: &str, new: &str) -> EditToolInput {
    EditToolInput {
        path: path.into(),
        edits: vec![maestro_tools::edit_diff::Edit {
            old_text: old.into(),
            new_text: new.into(),
        }],
    }
}
/// Complete externally observable successful result.
pub fn result_json(result: &AgentToolResult<EditToolDetails>) -> Value {
    assert!(result.terminate.is_none());
    json!({"content": content_json(result), "details": result.details.as_ref().unwrap()})
}
/// Strict recorded filesystem invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileInput {
    /// Authored path.
    pub path: String,
    /// Initial raw bytes.
    pub bytes: Vec<u8>,
    /// Ordered replacement entries.
    pub edits: Vec<Replacement>,
    /// Optional Unix mode used by the permission witness.
    pub mode: Option<u32>,
}
/// A recorded replacement, rejecting unused fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Replacement {
    /// Matching operand.
    old_text: String,
    /// Literal replacement.
    new_text: String,
}
impl FileInput {
    /// Move recorded operands into the production input record.
    pub fn arguments(self) -> EditToolInput {
        EditToolInput {
            path: self.path,
            edits: self
                .edits
                .into_iter()
                .map(|r| maestro_tools::edit_diff::Edit {
                    old_text: r.old_text,
                    new_text: r.new_text,
                })
                .collect(),
        }
    }
}
/// Recorded filesystem outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileExpected {
    /// Whether execution succeeded.
    pub ok: bool,
    /// Complete result, only for successes.
    pub result: Option<Value>,
    /// Target bytes after settlement.
    pub bytes: Vec<u8>,
}
