//! Controlled artifact command observations.
use super::*;
use crate as maestro_app;
use crate::tests::runtime::run_io as run;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    future::Future,
    sync::Mutex,
    task::{Context, Waker},
};

/// Strict recorded command corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Corpus {
    /// Model input schema.
    schema: Value,
    /// Public tool names.
    declaration: Declaration,
    /// Complete description inputs and outputs.
    descriptions: Vec<Description>,
    /// Sequential command scenarios.
    cases: Vec<Case>,
    /// Raw preparation routes.
    domains: Vec<Domain>,
    /// Surviving map and record observations.
    aliases: Aliases,
    /// Captured-view timer observations.
    timer: Timer,
    /// Tool execution after caller release.
    retained_tool: RetainedTool,
    /// Shared runtime descriptions.
    constants: Constants,
}
/// Recorded name and label.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declaration {
    /// Recorded name.
    name: String,
    /// Recorded label.
    label: String,
}
/// Model guidance with supplied operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Description {
    /// Recorded input.
    input: Vec<String>,
    /// Recorded output.
    output: String,
}
/// A sequence and its complete post-operation observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Recorded id.
    id: String,
    /// Recorded steps.
    steps: Vec<Step>,
    /// Recorded observations.
    observations: Vec<Observation>,
}
/// Command or controlled presentation operation.
#[derive(Deserialize)]
#[serde(untagged)]
enum Step {
    /// Ordinary tool input.
    Command(CommandInput),
    /// Presentation setup or reconstruction-only invocation.
    Control(Control),
}
/// Strict command input from the recording.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CommandInput {
    /// Recorded command.
    command: String,
    /// Recorded filename.
    filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    old_str: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new_str: Option<String>,
}
/// Strict controlled effects from the recording.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Control {
    /// Controlled `Open` operation.
    Open { filename: String },
    /// Controlled `RemoveElement` operation.
    RemoveElement { filename: String },
    /// Controlled `Unmount` operation.
    Unmount { filename: String },
    /// Controlled `Logs` operation.
    Logs { filename: String, value: Vec<Log> },
    /// Controlled `DeleteDuringNotify` operation.
    DeleteDuringNotify { filename: String },
    /// Controlled `DirectCreate` operation.
    DirectCreate {
        args: CommandInput,
        options: ControlOptions,
    },
}
/// Independent reconstruction policies.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ControlOptions {
    /// Recorded silent.
    silent: bool,
    /// Recorded skip wait.
    skip_wait: bool,
}
/// Formatted log operand supplied by the host.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Log {
    /// Recorded type.
    r#type: String,
    /// Recorded text.
    text: String,
}
/// Full returned value, state and ordered effect transcript.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    /// Recorded output.
    output: Value,
    /// Recorded state.
    state: State,
    /// Recorded events.
    events: Vec<Value>,
}
/// Current ordered records and active selection.
#[derive(Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
struct State {
    /// Recorded artifacts.
    artifacts: Vec<Record>,
    /// Recorded active.
    active: Option<String>,
}
/// Independent serialized record observation.
#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Record {
    /// Recorded filename.
    filename: String,
    /// Recorded content.
    content: String,
    /// Recorded created at.
    created_at: String,
    /// Recorded updated at.
    updated_at: String,
}
/// Raw and prepared argument values.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Domain {
    /// Recorded input.
    input: Value,
    /// Recorded accepted.
    accepted: bool,
    /// Recorded prepared.
    prepared: Option<Value>,
}
/// Boolean identity observation kept distinct from command policies.
#[derive(Deserialize)]
#[serde(transparent)]
struct Flag(bool);
/// Alias identity and lifetime results.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Aliases {
    /// Recorded initial after create.
    initial_after_create: Vec<String>,
    /// Recorded after create after second.
    after_create_after_second: Vec<String>,
    /// Recorded after second after delete.
    after_second_after_delete: Vec<String>,
    /// Recorded current keys.
    current_keys: Vec<String>,
    /// Recorded update same.
    update_same: Flag,
    /// Recorded record same.
    record_same: Flag,
    /// Recorded rewrite same.
    rewrite_same: Flag,
    /// Recorded rewrite record same.
    rewrite_record_same: Flag,
    /// Recorded create replaces.
    create_replaces: Flag,
    /// Recorded delete replaces.
    delete_replaces: Flag,
    /// Recorded record after delete.
    record_after_delete: String,
    /// Recorded external get.
    external_get: Value,
    /// Recorded external deleted record logs.
    external_deleted_record_logs: Value,
}
/// Wait boundary and the complete interleaved transcript.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Timer {
    /// Recorded before boundary pending.
    before_boundary_pending: bool,
    /// Recorded output.
    output: Value,
    /// Recorded state.
    state: State,
    /// Recorded events.
    events: Vec<Value>,
}
/// Retained executable result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedTool {
    /// Recorded output.
    output: Value,
}
/// Shared prompt text.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Constants {
    /// Read-only artifact guidance.
    #[serde(rename = "ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO")]
    artifacts: String,
    /// Attachment guidance.
    #[serde(rename = "ATTACHMENTS_RUNTIME_DESCRIPTION")]
    attachments: String,
}
/// Load the independent recording.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!(
        "../../../../tests/fixtures/artifact_commands.json"
    ))
    .unwrap()
}
/// Retained controlled view state.
struct View {
    /// Recorded html.
    html: bool,
    /// Recorded mounted.
    mounted: bool,
    /// Recorded content.
    content: String,
    /// Recorded logs.
    logs: Arc<Mutex<String>>,
}
/// Presentation operations and explicitly controlled clock.
#[derive(Default)]
struct Host {
    /// Weak observer avoids owning its caller.
    selection_probe: Mutex<Option<std::sync::Weak<ArtifactsPanel>>>,
    /// Recorded clock.
    clock: Mutex<i64>,
    /// Recorded events.
    events: Arc<Mutex<Vec<Value>>>,
    /// Recorded views.
    views: Mutex<IndexMap<String, View>>,
    /// Recorded notification removed.
    notification_removed: std::sync::atomic::AtomicBool,
    /// Recorded delete on change.
    delete_on_change: Mutex<Option<(std::sync::Weak<ArtifactsPanel>, String)>>,
    /// Recorded timer.
    timer: Mutex<Option<Arc<Mutex<TestClock>>>>,
}
/// Snapshot records without retaining any guard through caller code.
fn records(artifacts: &Artifacts) -> Vec<Record> {
    let handles: Vec<_> = artifacts.read().unwrap().values().cloned().collect();
    handles
        .iter()
        .map(|handle| {
            let record: &maestro_app::Artifact = &handle.read().unwrap();
            Record {
                filename: record.filename.clone(),
                content: record.content.clone(),
                created_at: record
                    .created_at
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                updated_at: record
                    .updated_at
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            }
        })
        .collect()
}
impl ArtifactPresentation for Host {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        let mut clock = self.clock.lock().unwrap();
        let date = chrono::DateTime::from_timestamp_millis(1_700_000_000_000 + *clock).unwrap();
        *clock += 1;
        date
    }
    fn get_or_create_artifact_element(
        &self,
        filename: &str,
        content: &str,
        panel: ArtifactsPanelState,
    ) {
        self.events.lock().unwrap().push(json!([
            "upsert",
            filename,
            content,
            records(&panel.artifacts())
        ]));
        let mut views = self.views.lock().unwrap();
        let view = views.entry(filename.into()).or_insert_with(|| View {
            html: filename
                .rsplit('.')
                .next()
                .unwrap()
                .eq_ignore_ascii_case("html"),
            mounted: true,
            content: String::new(),
            logs: Arc::new(Mutex::new(format!("No logs for {filename}"))),
        });
        if view.content != content {
            *view.logs.lock().unwrap() = format!("No logs for {filename}");
        }
        view.content = content.into();
    }
    fn show_artifact(&self, filename: &str) {
        self.events.lock().unwrap().push(json!(["show", filename]));
        let panel = self
            .selection_probe
            .lock()
            .unwrap()
            .as_ref()
            .and_then(std::sync::Weak::upgrade);
        if let Some(panel) = panel {
            assert!(panel.state.0.try_write().is_ok());
            assert!(panel.artifacts().try_write().is_ok());
            self.events
                .lock()
                .unwrap()
                .push(json!(["before", panel.active_filename()]));
        }
    }
    fn remove_artifact_element(&self, filename: &str) {
        if self.views.lock().unwrap().shift_remove(filename).is_some() {
            self.events
                .lock()
                .unwrap()
                .push(json!(["remove", filename]));
        }
    }
    fn reload_all_html_artifacts(&self, panel: ArtifactsPanelState) {
        let artifacts = panel.artifacts();
        let views = self.views.lock().unwrap();
        for (filename, view) in views.iter().filter(|(_, v)| v.html && v.mounted) {
            self.events.lock().unwrap().extend([
                json!(["providers", filename, records(&artifacts)]),
                json!(["refresh", filename, view.content]),
            ]);
        }
    }
    fn get_logs(&self, filename: &str) -> Result<ArtifactLogReader, ArtifactLogError> {
        let views = self.views.lock().unwrap();
        let view = views.get(filename).ok_or(ArtifactLogError::Missing)?;
        if !view.html {
            return Err(ArtifactLogError::NotHtml);
        }
        let (logs, events, filename) = (
            Arc::clone(&view.logs),
            Arc::clone(&self.events),
            filename.to_owned(),
        );
        Ok(Arc::new(move || {
            let logs = logs.lock().unwrap().clone();
            events.lock().unwrap().push(json!(["logs", filename, logs]));
            logs
        }))
    }
    fn wait(&self, duration: std::time::Duration) -> maestro_models::BoxFuture<()> {
        self.events
            .lock()
            .unwrap()
            .push(json!(["wait", duration.as_millis()]));
        let timer = self.timer.lock().unwrap().clone();
        let gate = timer.map(|timer| {
            let (release, gate) = tokio::sync::oneshot::channel();
            timer.lock().unwrap().pending = Some((duration, release));
            gate
        });
        Box::pin(async move {
            if let Some(gate) = gate {
                gate.await.unwrap();
            }
        })
    }
    fn request_update(&self) {
        self.events.lock().unwrap().push(json!(["request_update"]));
        let panel = self
            .selection_probe
            .lock()
            .unwrap()
            .as_ref()
            .and_then(std::sync::Weak::upgrade);
        if let Some(panel) = panel {
            assert!(panel.state.0.try_write().is_ok());
            self.events
                .lock()
                .unwrap()
                .push(json!(["after", panel.active_filename()]));
        }
    }
    fn on_artifacts_change(&self) {
        if self
            .notification_removed
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return;
        }
        self.events.lock().unwrap().push(json!(["changed"]));
        let delete = self.delete_on_change.lock().unwrap().take();
        if let Some((panel, filename)) = delete {
            self.notification_removed
                .store(true, std::sync::atomic::Ordering::SeqCst);
            panel.upgrade().unwrap().delete(&filename).unwrap();
        }
    }
    fn on_open(&self) {
        self.events.lock().unwrap().push(json!(["open"]));
    }
}
/// Current state observed by the command owner.
fn state(panel: &ArtifactsPanel) -> State {
    State {
        artifacts: records(&panel.artifacts()),
        active: panel.state.0.read().unwrap().active.clone(),
    }
}
/// Execute through the retained public tool boundary.
async fn call(tool: &AgentTool, args: Value) -> Value {
    observe((tool.execute)("call".into(), args, None, None)).await
}
/// Require a command result without details or termination, then serialize it.
async fn observe<E: std::fmt::Debug>(
    result: impl Future<Output = Result<AgentToolResult<Value>, E>>,
) -> Value {
    let result = result.await.unwrap();
    assert!(result.details.is_none() && result.terminate.is_none());
    serde_json::to_value(result).unwrap()
}
/// One command through either caller representation's executable tool.
type Call<'a> = &'a dyn Fn(Value) -> std::pin::Pin<Box<dyn Future<Output = Value> + 'a>>;
/// Replay disjoint command scenarios, retaining each step's complete observations.
async fn rows(ids: &[&str]) {
    let cases: Vec<_> = corpus()
        .cases
        .into_iter()
        .filter(|case| ids.contains(&case.id.as_str()))
        .collect();
    assert_eq!(cases.len(), ids.len());
    for case in cases {
        let host = Arc::new(Host::default());
        let panel = Arc::new(ArtifactsPanel::new(host.clone()));
        let tool = panel.tool();
        assert_eq!(case.steps.len(), case.observations.len());
        for (step, expected) in case.steps.into_iter().zip(case.observations) {
            let output = step_output(&panel, &host, &tool, step).await;
            assert_eq!(output, expected.output, "{} output", case.id);
            assert_eq!(state(&panel), expected.state, "{} state", case.id);
            assert_eq!(
                std::mem::take(&mut *host.events.lock().unwrap()),
                expected.events,
                "{} events",
                case.id
            );
        }
    }
}
/// Apply a command or controlled host setup operation.
async fn step_output(
    panel: &Arc<ArtifactsPanel>,
    host: &Arc<Host>,
    tool: &AgentTool,
    step: Step,
) -> Value {
    let Step::Control(control) = step else {
        let Step::Command(command) = step else {
            unreachable!()
        };
        return call(tool, serde_json::to_value(command).unwrap()).await;
    };
    match control {
        Control::Open { filename } => panel.open_artifact(&filename),
        Control::RemoveElement { filename } => {
            host.views.lock().unwrap().shift_remove(&filename);
        }
        Control::Unmount { filename } => {
            host.views
                .lock()
                .unwrap()
                .get_mut(&filename)
                .unwrap()
                .mounted = false;
        }
        Control::Logs { filename, value } => {
            let text = value
                .into_iter()
                .map(|log| format!("[{}] {}", log.r#type, log.text))
                .collect::<Vec<_>>()
                .join("\n");
            *host.views.lock().unwrap()[&filename].logs.lock().unwrap() = text;
        }
        Control::DeleteDuringNotify { filename } => {
            *host.delete_on_change.lock().unwrap() = Some((Arc::downgrade(panel), filename));
        }
        Control::DirectCreate { args, options } => {
            let params =
                serde_json::from_value::<ArtifactsParams>(serde_json::to_value(args).unwrap())
                    .unwrap();
            return json!(
                panel
                    .execute(
                        &params,
                        Options {
                            silent: options.silent,
                            skip_wait: options.skip_wait
                        }
                    )
                    .await
                    .unwrap_or_else(|error| error)
            );
        }
    }
    Value::Null
}
/// Public records retain independent clock observations.
#[test]
fn artifact_create_retains_records_and_timestamps() {
    run(async {
        rows(&["create_and_get"]).await;
    });
}
/// Guidance preserves every supplied runtime description byte.
#[test]
fn artifact_description_matches_schema_and_runtime_guidance() {
    let corpus = corpus();
    for row in corpus.descriptions {
        let descriptions: Vec<_> = row.input.iter().map(String::as_str).collect();
        assert_eq!(
            super::super::prompts::artifacts_tool_description(&descriptions),
            row.output
        );
    }
    assert_eq!(
        maestro_app::ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO,
        corpus.constants.artifacts
    );
    assert_eq!(
        maestro_app::ATTACHMENTS_RUNTIME_DESCRIPTION,
        corpus.constants.attachments
    );
}
/// Validate raw object routes before executing typed commands.
#[test]
fn artifact_schema_routes_prepare_before_typed_execution() {
    run(async {
        let host = Arc::new(Host::default());
        let tool = ArtifactsPanel::new(host.clone()).tool();
        for row in corpus().domains {
            let prepared = row.input.as_object().and_then(|arguments| {
                maestro_models::validate_tool_arguments(
                    &tool.definition,
                    &maestro_models::ToolCall {
                        id: "call".into(),
                        name: "artifacts".into(),
                        arguments: arguments.clone().into(),
                        thought_signature: None,
                    },
                )
                .ok()
            });
            assert_eq!(prepared.is_some(), row.accepted);
            assert_eq!(prepared.as_ref().map(|value| json!(value)), row.prepared);
            if let Some(prepared) = prepared {
                let value = prepared;
                let params: maestro_app::ArtifactsParams =
                    serde_json::from_value(value.clone()).unwrap();
                assert_eq!(
                    call(&tool, value).await,
                    json!({"content":[{"type":"text", "text": format!("Error: File {} not found. No files have been created yet.", params.filename)}]})
                );
            } else {
                assert!(
                    (tool.execute)("call".into(), row.input, None, None)
                        .await
                        .is_err()
                );
            }
        }
        assert!(host.events.lock().unwrap().is_empty());
    });
}

/// Recorded create missing content behavior.
#[test]
fn artifact_create_rejects_missing_empty_and_duplicate_inputs() {
    run(async {
        rows(&[
            "create_missing_content",
            "create_empty_content",
            "create_empty_filename",
            "create_duplicate",
        ])
        .await;
    });
}

/// Recorded whitespace and utf8 behavior.
#[test]
fn artifact_names_and_content_remain_literal() {
    run(async {
        rows(&["whitespace_and_utf8"]).await;
    });
}

/// Recorded first occurrence behavior.
#[test]
fn artifact_update_changes_first_match_and_retains_creation() {
    run(async {
        rows(&["first_occurrence"]).await;
    });
}

/// Recorded erase to empty behavior.
#[test]
fn artifact_update_accepts_empty_replacement() {
    run(async {
        rows(&["erase_to_empty"]).await;
    });
}

/// Recorded update missing old behavior.
#[test]
fn artifact_update_preserves_validation_precedence() {
    run(async {
        rows(&[
            "update_missing_old",
            "update_empty_old",
            "update_missing_new",
            "update_not_found_text",
        ])
        .await;
    });
}

/// Recorded update empty store behavior.
#[test]
fn artifact_missing_messages_preserve_insertion_order() {
    run(async {
        rows(&[
            "update_empty_store",
            "update_ordered_store",
            "rewrite_empty_store",
            "rewrite_ordered_store",
            "get_empty_store",
            "get_ordered_store",
            "delete_empty_store",
            "delete_ordered_store",
            "logs_empty_store",
            "logs_ordered_store",
        ])
        .await;
    });
}

/// Recorded rewrite result behavior.
#[test]
fn artifact_rewrite_returns_empty_text_and_preserves_creation() {
    run(async {
        rows(&["rewrite_result"]).await;
    });
}

/// Recorded rewrite missing behavior.
#[test]
fn artifact_rewrite_rejects_missing_and_empty_content() {
    run(async {
        rows(&["rewrite_missing", "rewrite_empty"]).await;
    });
}

/// Recorded delete active first behavior.
#[test]
fn artifact_delete_selects_first_remaining_and_recreation_appends() {
    run(async {
        rows(&["delete_active_first"]).await;
    });
}

/// Recorded delete inactive behavior.
#[test]
fn artifact_delete_leaves_other_active_selection() {
    run(async {
        rows(&["delete_inactive"]).await;
    });
}

/// Recorded open existing and missing behavior.
#[test]
fn artifact_open_notifies_only_for_existing_file() {
    run(async {
        rows(&["open_existing_and_missing"]).await;
    });
}

/// Recorded non html logs behavior.
#[test]
fn artifact_logs_distinguish_non_html_and_missing_element() {
    run(async {
        rows(&["non_html_logs", "missing_view_logs"]).await;
    });
}

/// Recorded html no logs behavior.
#[test]
fn artifact_html_wait_preserves_no_logs_message() {
    run(async {
        rows(&["html_no_logs"]).await;
    });
}

/// Recorded html live logs behavior.
#[test]
fn artifact_logs_read_current_buffer_without_wait() {
    run(async {
        rows(&["html_live_logs"]).await;
    });
}

/// Recorded html update rewrite wait behavior.
#[test]
fn artifact_html_mutations_clear_stale_logs_before_wait() {
    run(async {
        rows(&["html_update_rewrite_wait"]).await;
    });
}

/// Recorded dependency refresh order behavior.
#[test]
fn artifact_changes_refresh_current_html_after_callbacks() {
    run(async {
        rows(&["dependency_refresh_order"]).await;
    });
}

/// Recorded create silent false skip true behavior.
#[test]
fn artifact_internal_options_keep_independent_effects() {
    run(async {
        rows(&[
            "create_silent_false_skip_true",
            "create_silent_true_skip_false",
            "create_silent_true_skip_true",
            "update_silent_false_skip_true",
            "update_silent_true_skip_false",
            "update_silent_true_skip_true",
            "rewrite_silent_false_skip_true",
            "rewrite_silent_true_skip_false",
            "rewrite_silent_true_skip_true",
        ])
        .await;
    });
}

/// Recorded dollar match behavior.
#[test]
fn artifact_update_inserts_replacement_metacharacters_literally() {
    run(async {
        rows(&[
            "dollar_match",
            "dollar_prefix",
            "dollar_suffix",
            "dollar_dollar",
            "dollar_numeric",
            "literal_mixed",
        ])
        .await;
    });
}

/// Recorded unicode first match behavior.
#[test]
fn artifact_update_keeps_unicode_and_newlines() {
    run(async {
        rows(&["unicode_first_match"]).await;
    });
}

/// Recorded extensionless html behavior.
#[test]
fn artifact_html_discriminator_uses_last_dot_segment() {
    run(async {
        rows(&["extensionless_html"]).await;
    });
}

/// Recorded reentrant delete behavior.
#[test]
fn artifact_callbacks_can_reenter_without_held_state_locks() {
    run(async {
        rows(&["reentrant_delete"]).await;
    });
}
/// The executable declares the recorded schema and ignores caller controls.
#[test]
fn artifact_tool_declares_six_commands_and_plain_results() {
    run(async {
        let corpus = corpus();
        let host = Arc::new(Host::default());
        let panel = maestro_app::ArtifactsPanel::new(host);
        let tool = panel.tool();
        assert_eq!(*tool.definition.parameters, corpus.schema);
        assert_eq!(tool.definition.name, corpus.declaration.name);
        assert_eq!(tool.label, corpus.declaration.label);
        let cancellation = maestro_models::Cancellation::new();
        cancellation.abort();
        let result = (tool.execute)(
            "call".into(),
            json!({"command":"get", "filename":"missing"}),
            Some(cancellation),
            Some(Arc::new(|_| panic!("unexpected progress"))),
        )
        .await
        .unwrap();
        assert!(result.details.is_none() && result.terminate.is_none());
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            json!({"content":[{"type":"text", "text":"Error: File missing not found. No files have been created yet."}]})
        );
    });
}
/// Surviving aliases observe mutation before a shallow map replacement in both representations.
#[test]
fn artifact_handles_keep_map_replacements_and_record_identity() {
    let expected = corpus().aliases;
    let panel = maestro_app::ArtifactsPanel::new(Arc::new(Host::default()));
    let tool = panel.tool();
    run(check_aliases(
        &|| panel.artifacts(),
        &|args| Box::pin(call(&tool, args)),
        &expected,
    ));
    let local = LocalArtifactsPanel::new_local(std::rc::Rc::new(Host::default()));
    let tool = local.tool();
    run(check_aliases(
        &|| local.artifacts(),
        &|args| Box::pin(observe((tool.execute)("call".into(), args, None, None))),
        &expected,
    ));
}
/// Replay the recorded alias observations through one panel's map handle and tool.
async fn check_aliases(artifacts: &dyn Fn() -> Artifacts, call: Call<'_>, expected: &Aliases) {
    let initial = artifacts();
    call(json!({"command":"create","filename":"a.txt","content":"a"})).await;
    let after_create = artifacts();
    let record = Arc::clone(&after_create.read().unwrap()["a.txt"]);
    assert_eq!(keys(&initial), expected.initial_after_create);
    assert_eq!(
        !Arc::ptr_eq(&initial, &after_create),
        expected.create_replaces.0
    );
    check_edit_aliases(artifacts, call, &after_create, &record, expected).await;
    record.write().unwrap().content = "external".into();
    assert_eq!(
        call(json!({"command":"get","filename":"a.txt"})).await,
        expected.external_get
    );
    check_replacement_aliases(artifacts, call, &after_create, &record, expected).await;
}
/// A second create and a delete replace the map while surviving aliases keep their view.
async fn check_replacement_aliases(
    artifacts: &dyn Fn() -> Artifacts,
    call: Call<'_>,
    after_create: &Artifacts,
    record: &Arc<RwLock<Artifact>>,
    expected: &Aliases,
) {
    call(json!({"command":"create","filename":"b.txt","content":"b"})).await;
    let after_second = artifacts();
    assert_eq!(keys(after_create), expected.after_create_after_second);
    call(json!({"command":"delete","filename":"a.txt"})).await;
    assert_eq!(keys(&after_second), expected.after_second_after_delete);
    assert_eq!(keys(&artifacts()), expected.current_keys);
    assert_eq!(
        !Arc::ptr_eq(&after_second, &artifacts()),
        expected.delete_replaces.0
    );
    assert_eq!(record.read().unwrap().content, expected.record_after_delete);
    artifacts().write().unwrap().shift_remove("b.txt");
    assert_eq!(
        call(json!({"command":"logs","filename":"b.txt"})).await,
        expected.external_deleted_record_logs
    );
}
/// Ordered map keys without record locking.
fn keys(artifacts: &Artifacts) -> Vec<String> {
    artifacts.read().unwrap().keys().cloned().collect()
}
/// The tool captures state, not the caller's panel handle.
#[test]
fn artifact_tool_retains_state_after_caller_drop() {
    run(async {
        let host = Arc::new(Host::default());
        let panel = Arc::new(maestro_app::ArtifactsPanel::new(host.clone()));
        let weak = Arc::downgrade(&panel);
        let tool = panel.tool();
        drop(panel);
        assert!(weak.upgrade().is_none());
        assert_eq!(
            call(
                &tool,
                json!({"command":"create","filename":"retained.txt","content":"retained"})
            )
            .await,
            corpus().retained_tool.output
        );
        let weak_host = Arc::downgrade(&host);
        drop(host);
        drop(tool);
        assert!(weak_host.upgrade().is_none());
    });
}
/// Keep the same view reader across removal and replacement at the wait boundary.
#[test]
fn artifact_html_wait_keeps_captured_reader_until_boundary() {
    run(async {
        let expected = corpus().timer;
        let observed = timer_witness(false).await;
        assert_eq!(observed.0, expected.before_boundary_pending);
        assert_eq!(observed.1, expected.output);
        assert_eq!(observed.2, expected.state);
        assert_eq!(observed.3, expected.events);
        let stronger = timer_witness(true).await;
        assert!(stronger.0);
        assert_eq!(
            stronger.1,
            json!({"content":[{"type":"text","text":"Created file p.html\n[log] late log"}]})
        );
        let replacement = stronger
            .3
            .iter()
            .position(|event| event[0] == "upsert" && event[2] == "new instance")
            .unwrap();
        let read = stronger
            .3
            .iter()
            .position(|event| event[0] == "logs")
            .unwrap();
        assert!(replacement < read);
    });
}
/// Drive a single retained future through an explicit elapsed-time boundary.
async fn timer_witness(recreate_before_completion: bool) -> (bool, Value, State, Vec<Value>) {
    let host = Arc::new(Host::default());
    let panel = ArtifactsPanel::new(host.clone());
    let tool = panel.tool();
    let clock = Arc::new(Mutex::new(TestClock::default()));
    *host.timer.lock().unwrap() = Some(Arc::clone(&clock));
    let mut future = Box::pin(call(
        &tool,
        json!({"command":"create","filename":"p.html","content":"x"}),
    ));
    let mut context = Context::from_waker(Waker::noop());
    assert!(future.as_mut().poll(&mut context).is_pending());
    let captured = Arc::clone(&host.views.lock().unwrap()["p.html"].logs);
    advance(&clock, 1499);
    let pending = future.as_mut().poll(&mut context).is_pending();
    *captured.lock().unwrap() = "[log] late log".into();
    panel.delete("p.html").unwrap();
    let mut output = None;
    if !recreate_before_completion {
        advance(&clock, 1);
        output = Some(future.as_mut().await);
    }
    let params = serde_json::from_value(
        json!({"command":"create","filename":"p.html","content":"new instance"}),
    )
    .unwrap();
    panel
        .execute(
            &params,
            Options {
                silent: false,
                skip_wait: true,
            },
        )
        .await
        .unwrap();
    *host.views.lock().unwrap()["p.html"].logs.lock().unwrap() = "[log] wrong new log".into();
    if recreate_before_completion {
        assert!(future.as_mut().poll(&mut context).is_pending());
        advance(&clock, 1);
    }
    let output = match output {
        Some(output) => output,
        None => future.await,
    };
    let events = std::mem::take(&mut *host.events.lock().unwrap());
    (pending, output, state(&panel), events)
}

/// An injected wait with explicit clock advancement, not a test deadline.
#[derive(Default)]
struct TestClock {
    /// Elapsed time of this single wait.
    elapsed: Duration,
    /// Duration and completion notification installed by the consuming operation.
    pending: Option<(Duration, tokio::sync::oneshot::Sender<()>)>,
}
/// Release the waiter only when the supplied duration has elapsed.
fn advance(clock: &Mutex<TestClock>, millis: u64) {
    let release = {
        let mut clock = clock.lock().unwrap();
        clock.elapsed += Duration::from_millis(millis);
        if clock
            .pending
            .as_ref()
            .is_some_and(|(duration, _)| clock.elapsed >= *duration)
        {
            clock.pending.take().map(|(_, release)| release)
        } else {
            None
        }
    };
    if let Some(release) = release {
        release.send(()).unwrap();
    }
}

/// Retained map and record identities survive both in-place edit commands.
async fn check_edit_aliases(
    artifacts: &dyn Fn() -> Artifacts,
    call: Call<'_>,
    after_create: &Artifacts,
    record: &Arc<RwLock<Artifact>>,
    expected: &Aliases,
) {
    call(json!({"command":"update","filename":"a.txt","old_str":"a","new_str":"b"})).await;
    assert_eq!(
        Arc::ptr_eq(after_create, &artifacts()),
        expected.update_same.0
    );
    assert_eq!(
        Arc::ptr_eq(record, &artifacts().read().unwrap()["a.txt"]),
        expected.record_same.0
    );
    call(json!({"command":"rewrite","filename":"a.txt","content":"rewrite"})).await;
    assert_eq!(
        Arc::ptr_eq(after_create, &artifacts()),
        expected.rewrite_same.0
    );
    assert_eq!(
        Arc::ptr_eq(record, &artifacts().read().unwrap()["a.txt"]),
        expected.rewrite_record_same.0
    );
}

/// Read the recorded classifier cases from the shared browser corpus.
fn filename_cases() -> Vec<(String, FileType)> {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../maestro-web/tests/fixtures/generic_artifacts.json"
    ))
    .unwrap();
    fixture["classifier"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            let expected = match case["output"].as_str().unwrap() {
                "html" => FileType::Html,
                "svg" => FileType::Svg,
                "markdown" => FileType::Markdown,
                "image" => FileType::Image,
                "pdf" => FileType::Pdf,
                "excel" => FileType::Excel,
                "docx" => FileType::Docx,
                "text" => FileType::Text,
                "generic" => FileType::Generic,
                _ => panic!("unknown recorded view"),
            };
            (case["input"]["filename"].as_str().unwrap().into(), expected)
        })
        .collect()
}
/// Canonical supported suffixes select their matching view.
#[test]
fn artifact_extensions_select_the_registered_view() {
    for (name, expected) in filename_cases().into_iter().take(70) {
        assert_eq!(get_file_type(&name), expected, "{name}");
    }
}
/// Filename boundaries retain literal suffix and Unicode lowercase semantics.
#[test]
fn artifact_names_use_the_last_literal_segment() {
    for (name, expected) in filename_cases().into_iter().skip(70) {
        assert_eq!(get_file_type(&name), expected, "{name}");
    }
}
/// Both affinity representations execute the same retained command policy.
#[test]
fn artifact_local_and_threaded_tools_share_command_policy() {
    let local_host = std::rc::Rc::new(Host::default());
    let threaded_host = Arc::new(Host::default());
    let local = LocalArtifactsPanel::new_local(local_host.clone());
    let threaded = ArtifactsPanel::new(threaded_host.clone());
    let local_tool = local.tool();
    let threaded_tool = threaded.tool();
    assert_eq!(local_tool.label, threaded_tool.label);
    assert_eq!(
        serde_json::to_value(&local_tool.definition).unwrap(),
        serde_json::to_value(&threaded_tool.definition).unwrap()
    );
    local.open_artifact("absent");
    let local_map = local.artifacts();
    assert!(local_map.read().unwrap().is_empty());
    drop(local);
    drop(threaded);
    for args in [
        json!({"command":"create","filename":"a.zip","content":"Zg=="}),
        json!({"command":"update","filename":"a.zip","old_str":"Zg==","new_str":"Zm8="}),
        json!({"command":"rewrite","filename":"a.zip","content":"AAEC//79"}),
        json!({"command":"get","filename":"a.zip"}),
        json!({"command":"logs","filename":"a.zip"}),
        json!({"command":"delete","filename":"a.zip"}),
        json!({"command":"get","filename":"absent"}),
    ] {
        let local = run((local_tool.execute)(
            "call".into(),
            args.clone(),
            None,
            None,
        ))
        .unwrap();
        let threaded = run((threaded_tool.execute)("call".into(), args, None, None)).unwrap();
        assert_eq!(
            serde_json::to_value(local).unwrap(),
            serde_json::to_value(threaded).unwrap()
        );
    }
    assert_eq!(
        *local_host.events.lock().unwrap(),
        *threaded_host.events.lock().unwrap()
    );
}

/// Cloned callers and tool deletion share the same literal selection slot.
#[test]
fn artifact_selection_shares_the_command_slot() {
    run(async {
        let panel = ArtifactsPanel::new(Arc::new(Host::default()));
        assert_eq!(panel.active_filename(), None);
        let alias = panel.clone();
        for name in ["unknown", ""] {
            alias.show_artifact(name);
            assert_eq!(panel.active_filename().as_deref(), Some(name));
        }
        let tool = panel.tool();
        for name in ["z.zip", "a.zip", "m.zip"] {
            call(
                &tool,
                json!({"command":"create","filename":name,"content":"YQ=="}),
            )
            .await;
            assert_eq!(panel.active_filename().as_deref(), Some(name));
        }
        alias.show_artifact("a.zip");
        call(&tool, json!({"command":"delete","filename":"a.zip"})).await;
        assert_eq!(panel.active_filename().as_deref(), Some("z.zip"));
        call(&tool, json!({"command":"delete","filename":"m.zip"})).await;
        assert_eq!(panel.active_filename().as_deref(), Some("z.zip"));
        call(&tool, json!({"command":"delete","filename":"z.zip"})).await;
        assert_eq!(panel.active_filename(), None);
    });
}

/// Tab selection avoids the host-open notification and unknown open is inert.
#[test]
fn artifact_tab_selection_skips_host_open() {
    run(async {
        let host = Arc::new(Host::default());
        let panel = ArtifactsPanel::new(host.clone());
        call(
            &panel.tool(),
            json!({"command":"create","filename":"a.zip","content":"YQ=="}),
        )
        .await;
        host.events.lock().unwrap().clear();
        panel.show_artifact("a.zip");
        assert_eq!(panel.active_filename().as_deref(), Some("a.zip"));
        assert_eq!(
            *host.events.lock().unwrap(),
            [json!(["show", "a.zip"]), json!(["request_update"])]
        );
        host.events.lock().unwrap().clear();
        panel.open_artifact("missing");
        assert_eq!(panel.active_filename().as_deref(), Some("a.zip"));
        assert!(host.events.lock().unwrap().is_empty());
        panel.open_artifact("a.zip");
        assert_eq!(
            *host.events.lock().unwrap(),
            [
                json!(["show", "a.zip"]),
                json!(["request_update"]),
                json!(["open"])
            ]
        );
    });
}

/// Presentation observes preassignment and repaint observes completed selection.
#[test]
fn artifact_selection_callbacks_release_state_guards() {
    let host = Arc::new(Host::default());
    let panel = Arc::new(ArtifactsPanel::new(host.clone()));
    *host.selection_probe.lock().unwrap() = Some(Arc::downgrade(&panel));
    panel.show_artifact("literal");
    assert_eq!(
        *host.events.lock().unwrap(),
        [
            json!(["show", "literal"]),
            json!(["before", null]),
            json!(["request_update"]),
            json!(["after", "literal"])
        ]
    );
}
