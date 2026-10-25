//! Shell rendering through the definition's public callbacks.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use super::{
    display_support::{self as display, Env, Setup},
    edit_render_support::Case,
};
use maestro_tools::output::render_utils::ToolRenderResultLike;
use maestro_tools::{BashToolOptions, create_bash_tool_definition};
use serde::Deserialize;
use serde_json::Value;

/// Strict independent corpus envelope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Foreground role palette.
    colors: Value,
    /// Independent public callback queries.
    cases: Vec<Case>,
}
/// One definition used by the public callback witnesses.
fn definition() -> maestro_tools::ToolDefinition<
    (),
    maestro_tools::BashToolInput,
    maestro_tools::BashToolDetails,
    maestro_tools::BashRenderState,
> {
    create_bash_tool_definition("/work".into(), String::new(), BashToolOptions::default())
}
/// Read a nonempty group, rejecting duplicate queries.
fn cases(test: &str) -> Vec<Case> {
    let corpus: Corpus =
        serde_json::from_str(include_str!("fixtures/bash-render-cases.json")).unwrap();
    assert_eq!(corpus.colors.as_object().unwrap().len(), 6);
    let groups: std::collections::BTreeSet<_> =
        corpus.cases.iter().map(|case| case.test.as_str()).collect();
    assert_eq!(
        groups,
        [
            "bash_call_formats_partial_arguments",
            "bash_default_themes_keep_role_colors",
            "bash_dropped_row_releases_refresh",
            "bash_elapsed_uses_one_decimal_seconds",
            "bash_error_partial_cancels_refresh_and_keeps_elapsed_label",
            "bash_expand_hint_changes_on_retained_render",
            "bash_expand_hint_uses_live_bindings",
            "bash_render_state_starts_once_and_stops_refresh",
            "bash_renderers_read_the_published_theme",
            "bash_renderers_reuse_matching_components",
            "bash_replay_omits_clock_until_execution_starts",
            "bash_result_cache_tracks_content_width_and_expansion",
            "bash_result_duration_follows_start_and_settlement",
            "bash_result_options_control_expansion_and_partial_label",
            "bash_result_orders_artifact_and_truncation_notices",
            "bash_result_sanitizes_trims_and_selects_tail",
            "bash_result_uses_shared_image_fallback",
        ]
        .into_iter()
        .collect()
    );
    let mut seen = std::collections::HashSet::new();
    for case in &corpus.cases {
        assert!(seen.insert((&case.op, case.input.to_string())));
    }
    let selected: Vec<_> = corpus
        .cases
        .into_iter()
        .filter(|case| case.test == test)
        .collect();
    assert!(!selected.is_empty());
    selected
}
/// The recorded render context shared by these cases.
fn render_context() -> serde_json::Value {
    serde_json::json!({"readme":"", "bindings":["ctrl+o"]})
}
/// Existing context setup with no filesystem or highlighting effects.
fn setup() -> Setup {
    Setup::from(&render_context())
}
/// Publish the corpus's six role colors with an optional shift.
fn env(shift: u8) -> Env {
    let corpus: Corpus =
        serde_json::from_str(include_str!("fixtures/bash-render-cases.json")).unwrap();
    let keys: Vec<_> = [
        "toolTitle",
        "toolOutput",
        "error",
        "muted",
        "warning",
        "dim",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for (index, key) in keys.iter().enumerate() {
        assert_eq!(corpus.colors[key], index + 1);
    }
    let mut env = Env::for_keys(&keys, shift);
    env.highlighter = display::Probe::new(true);
    env
}
#[test]
fn bash_call_formats_partial_arguments() {
    let env = env(0);
    let tool = definition();
    for case in cases("bash_call_formats_partial_arguments") {
        assert_eq!(case.op, "call");
        let input = case.input.as_object().unwrap();
        assert_eq!(input.len(), 2);
        let args = &case.input["args"];
        let ctx = display::context(args, &setup(), &env, None);
        let component = tool.render_call.as_ref().unwrap()(Some(args), &env.live, &ctx).unwrap();
        assert_eq!(
            serde_json::to_value(
                component.render(usize::try_from(case.input["width"].as_u64().unwrap()).unwrap())
            )
            .unwrap(),
            case.expected
        );
    }
}

#[test]
fn bash_elapsed_uses_one_decimal_seconds() {
    use display::manual_runtime::ms;
    for case in cases("bash_elapsed_uses_one_decimal_seconds") {
        assert_eq!(case.op, "duration");
        assert_eq!(case.input.as_object().unwrap().len(), 1);
        let env = env(0);
        let tool = definition();
        let args = serde_json::json!({});
        let mut setup = setup();
        setup.phase.execution_started = true;
        for result in env
            .runtime
            .advance_to(std::time::Duration::from_micros(999))
        {
            result.unwrap();
        }
        let ctx = display::context(&args, &setup, &env, None);
        tool.render_call.as_ref().unwrap()(Some(&args), &env.live, &ctx).unwrap();
        let delta = case.input["ms"].as_u64().unwrap();
        // A fractional origin at 50 ms rejects truncating after subtraction.
        let fraction = if delta == 50 { 1 } else { 999 };
        for result in env
            .runtime
            .advance_to(ms(delta) + std::time::Duration::from_micros(fraction))
        {
            result.unwrap();
        }
        let row = crate::display_support::native(
            tool.render_result.as_ref().unwrap(),
            &ToolRenderResultLike::from(&maestro_agent::AgentToolResult {
                content: vec![],
                details: None,
                terminate: None,
            }),
            maestro_tools::ToolRenderResultOptions {
                expanded: false,
                is_partial: false,
            },
            &env.live,
            &ctx,
        )
        .unwrap();
        let expected = display::padded_to(
            &format!(
                "\x1b[38;5;4mTook {}\x1b[39m",
                case.expected.as_str().unwrap()
            ),
            40,
        );
        assert_eq!(row.render(40), [" ".repeat(40), expected]);
    }
}

/// Recorded clock setup driven through public callbacks, not private state writes.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Clock {
    /// Initial execution observation.
    started_at: Option<u64>,
    /// Earlier settlement observation.
    ended_at: Option<u64>,
}
/// Strict result operands; independent sibling flags are deliberately inverted in context.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResultInput {
    /// Typed result content and details.
    result: RecordedResult,
    /// Expansion selected by result options.
    #[serde(default)]
    expanded: bool,
    /// Streaming selected by result options.
    #[serde(default)]
    partial: bool,
    /// Error selected by context.
    #[serde(default)]
    is_error: bool,
    /// Prior public observations.
    #[serde(default)]
    state: Clock,
    /// Current whole milliseconds.
    now: Option<u64>,
    /// Terminal viewport.
    width: usize,
    /// Current expand keys.
    bindings: Option<Vec<String>>,
    /// Terminal image protocol.
    images: Option<String>,
    /// Caller image visibility.
    show_images: Option<bool>,
}
/// Strict result record at the fixture boundary.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordedResult {
    /// Ordered shared content records.
    content: Vec<maestro_models::UserBlock>,
    /// Optional typed metadata.
    details: Option<RecordedDetails>,
}
/// Shell metadata operands.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordedDetails {
    /// Existing shared accounting record, validated before decoding.
    truncation: Option<Value>,
    /// Authored artifact identity.
    full_output_path: Option<String>,
}
impl RecordedResult {
    /// Convert at the boundary using the existing shared record owner.
    fn native(self) -> maestro_agent::AgentToolResult<maestro_tools::BashToolDetails> {
        let details = self.details.map(|details| {
            let truncation = details.truncation.map(truncation);
            maestro_tools::BashToolDetails {
                truncation,
                full_output_path: details.full_output_path,
            }
        });
        maestro_agent::AgentToolResult {
            content: self.content,
            details,
            terminate: None,
        }
    }
}
/// Reject unread accounting members before decoding the shared record.
fn truncation(record: Value) -> maestro_tools::TruncationResult {
    assert!(record.as_object().unwrap().keys().all(|key| {
        [
            "content",
            "truncated",
            "truncatedBy",
            "totalLines",
            "totalBytes",
            "outputLines",
            "outputBytes",
            "lastLinePartial",
            "firstLineExceedsLimit",
            "maxLines",
            "maxBytes",
        ]
        .contains(&key.as_str())
    }));
    serde_json::from_value(record).unwrap()
}
/// Advance controlled time, checking every delivered callback.
fn advance(env: &Env, time: u64) {
    for result in env.runtime.advance_to(display::manual_runtime::ms(time)) {
        result.unwrap();
    }
}
/// Empty result used only to establish an earlier settlement through its public boundary.
fn empty() -> maestro_agent::AgentToolResult<maestro_tools::BashToolDetails> {
    maestro_agent::AgentToolResult {
        content: vec![],
        details: None,
        terminate: None,
    }
}
/// Drive a complete result fixture, with clock setup through the call/result slots.
fn result_case(case: &Case) -> Vec<String> {
    assert_eq!(case.op, "result");
    let input: ResultInput = serde_json::from_value(case.input.clone()).unwrap();
    let mut setup = setup();
    setup.expanded = !input.expanded;
    setup.phase.is_partial = !input.partial;
    setup.is_error = input.is_error;
    setup.images = input.images.map(|protocol| {
        assert_eq!(protocol, "kitty");
        maestro_tui::ImageProtocol::Kitty
    });
    setup.show_images = input.show_images.unwrap_or(true);
    let keys = input
        .bindings
        .unwrap_or_else(|| display::expand_keys(&render_context()));
    let _bindings = display::bind(&keys);
    let env = env(0);
    let args = serde_json::json!({});
    let tool = definition();
    let mut ctx = display::context(&args, &setup, &env, None);
    if let Some(start) = input.state.started_at {
        advance(&env, start);
        ctx.phase.execution_started = true;
        tool.render_call.as_ref().unwrap()(Some(&args), &env.live, &ctx).unwrap();
    }
    if let Some(end) = input.state.ended_at {
        advance(&env, end);
        crate::display_support::native(
            tool.render_result.as_ref().unwrap(),
            &ToolRenderResultLike::from(&empty()),
            maestro_tools::ToolRenderResultOptions {
                expanded: false,
                is_partial: false,
            },
            &env.live,
            &ctx,
        )
        .unwrap();
    }
    advance(&env, input.now.unwrap_or(10000));
    let row = crate::display_support::native(
        tool.render_result.as_ref().unwrap(),
        &ToolRenderResultLike::from(&input.result.native()),
        maestro_tools::ToolRenderResultOptions {
            expanded: input.expanded,
            is_partial: input.partial,
        },
        &env.live,
        &ctx,
    )
    .unwrap();
    row.render(input.width)
}
/// Compare every independent query in a result group.
fn check_results(test: &str) {
    for case in cases(test) {
        assert_eq!(
            serde_json::to_value(result_case(&case)).unwrap(),
            case.expected
        );
    }
}
#[test]
fn bash_result_sanitizes_trims_and_selects_tail() {
    check_results("bash_result_sanitizes_trims_and_selects_tail");
}

#[test]
fn bash_result_uses_shared_image_fallback() {
    check_results("bash_result_uses_shared_image_fallback");
}

#[test]
fn bash_result_orders_artifact_and_truncation_notices() {
    check_results("bash_result_orders_artifact_and_truncation_notices");
}

#[test]
fn bash_result_duration_follows_start_and_settlement() {
    check_results("bash_result_duration_follows_start_and_settlement");
}

#[test]
fn bash_expand_hint_uses_live_bindings() {
    check_results("bash_expand_hint_uses_live_bindings");
}

#[test]
fn bash_result_options_control_expansion_and_partial_label() {
    check_results("bash_result_options_control_expansion_and_partial_label");
}

/// A sequence of public call/result/time effects.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Timeline {
    /// Ordered operations; repeated observations remain distinct.
    steps: Vec<Step>,
}
/// One controlled operation boundary.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    /// Time since the previous operation.
    advance: Option<u64>,
    /// Authored call arguments.
    call: Option<Value>,
    /// Execution admission.
    started: Option<bool>,
    /// Result publication.
    result: Option<RecordedResult>,
    /// Partial result selector.
    partial: Option<bool>,
    /// Error result selector.
    error: Option<bool>,
}
/// Source state observation paired with a public duration frame.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StateExpected {
    /// Observation category.
    kind: String,
    /// Expected first execution operation time.
    started_at: Option<u64>,
    /// Expected first settlement operation time.
    ended_at: Option<u64>,
    /// Owned deferred callbacks.
    timers: usize,
    /// Actual redraw notifications.
    invalidations: usize,
}
/// Complete frame and component identity observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FrameExpected {
    /// Slot whose callback just ran.
    kind: String,
    /// Whether it retained the matching component.
    same: bool,
    /// Exact styled and padded rows.
    rows: Vec<String>,
}
/// Compare source traces without exposing private timing fields.
fn timeline(test: &str) {
    for case in cases(test) {
        timeline_case(case);
    }
}
/// Replay one complete ordered trace.
fn timeline_case(case: Case) {
    use std::{cell::Cell, rc::Rc};
    assert_eq!(case.op, "timeline");
    let input: Timeline = serde_json::from_value(case.input).unwrap();
    let expected = case.expected.as_array().unwrap();
    let env = env(0);
    advance(&env, 10000);
    let args = serde_json::json!({});
    let setup = setup();
    let mut ctx = display::context(&args, &setup, &env, None);
    let invalidations = Rc::new(Cell::new(0));
    let count = Rc::clone(&invalidations);
    ctx.invalidate = Rc::new(move || count.set(count.get() + 1));
    let tool = definition();
    let mut call = None;
    let mut result = None;
    let mut index = 0;
    let mut observed_start = None;
    let mut observed_end = None;
    for step in input.steps {
        advance(&env, env.runtime.millis() + step.advance.unwrap_or(0));
        if let Some(args) = step.call {
            ctx.phase.execution_started = step.started.unwrap_or(false);
            if ctx.phase.execution_started && observed_start.is_none() {
                observed_start = Some(env.runtime.millis());
                observed_end = None;
            }
            ctx.last_component = call.clone();
            let next = tool.render_call.as_ref().unwrap()(Some(&args), &env.live, &ctx).unwrap();
            frame(&expected[index], "call", call.as_ref(), &next, 40);
            call = Some(next);
            index += 1;
        }
        if let Some(record) = step.result {
            ctx.is_error = step.error.unwrap_or(false);
            let partial = step.partial.unwrap_or(false);
            if !partial || ctx.is_error {
                observed_end.get_or_insert(env.runtime.millis());
            }
            ctx.last_component = result.clone();
            let next = crate::display_support::native(
                tool.render_result.as_ref().unwrap(),
                &ToolRenderResultLike::from(&record.native()),
                maestro_tools::ToolRenderResultOptions {
                    expanded: false,
                    is_partial: partial,
                },
                &env.live,
                &ctx,
            )
            .unwrap();
            frame(&expected[index], "result", result.as_ref(), &next, 40);
            result = Some(next);
            index += 1;
        }
        let times = (observed_start, observed_end);
        check_state(&expected[index], times, &env, invalidations.get());
        index += 1;
    }
    assert_eq!(index, expected.len());
}
/// Clock operands describe observed public operation times; duration frames prove actual timing.
fn check_state(
    value: &Value,
    operation_times: (Option<u64>, Option<u64>),
    env: &Env,
    invalidations: usize,
) {
    let state: StateExpected = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(state.kind, "state");
    assert_eq!((state.started_at, state.ended_at), operation_times);
    assert_eq!(
        (env.runtime.pending(), invalidations),
        (state.timers, state.invalidations)
    );
}
/// Check actual slot identity and the full ordered frame.
fn frame(
    expected: &Value,
    kind: &str,
    previous: Option<&maestro_tui::tui::ComponentHandle>,
    next: &maestro_tui::tui::ComponentHandle,
    width: usize,
) {
    let expected: FrameExpected = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(expected.kind, kind);
    assert_eq!(
        previous.is_none_or(|previous| std::rc::Rc::ptr_eq(previous, next)),
        expected.same
    );
    assert_eq!(next.render(width), expected.rows);
}
#[test]
fn bash_render_state_starts_once_and_stops_refresh() {
    timeline("bash_render_state_starts_once_and_stops_refresh");
}

#[test]
fn bash_error_partial_cancels_refresh_and_keeps_elapsed_label() {
    timeline("bash_error_partial_cancels_refresh_and_keeps_elapsed_label");
}

#[test]
fn bash_replay_omits_clock_until_execution_starts() {
    timeline("bash_replay_omits_clock_until_execution_starts");
}

/// Recorded cache action and independent resulting rows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheStage {
    /// Observable operation operands.
    input: CacheInput,
    /// Independent full frame.
    rows: Vec<String>,
}
/// Cache-relevant caller operations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheInput {
    /// New body, when different from the previous stage.
    text: String,
    /// Draw width.
    width: usize,
    /// Result expansion option.
    expanded: bool,
    /// Explicit component invalidation.
    invalidate: bool,
}
/// One literal text result using the shared record type.
fn text_result(text: &str) -> maestro_agent::AgentToolResult<maestro_tools::BashToolDetails> {
    maestro_agent::AgentToolResult::text_result(text.into(), None)
}
#[test]
fn bash_result_cache_tracks_content_width_and_expansion() {
    for case in cases("bash_result_cache_tracks_content_width_and_expansion") {
        assert_eq!(case.op, "cache");
        assert_eq!(case.input, serde_json::json!({}));
        let stages: Vec<CacheStage> = serde_json::from_value(case.expected).unwrap();
        let env = env(0);
        let setup = setup();
        let _bindings = display::bind(&display::expand_keys(&render_context()));
        let args = serde_json::json!({});
        let mut ctx = display::context(&args, &setup, &env, None);
        let tool = definition();
        let mut previous_input: Option<(String, bool)> = None;
        for stage in stages {
            let input = stage.input;
            if previous_input
                .as_ref()
                .map(|(text, expanded)| (text.as_str(), *expanded))
                != Some((input.text.as_str(), input.expanded))
            {
                let next = crate::display_support::native(
                    tool.render_result.as_ref().unwrap(),
                    &ToolRenderResultLike::from(&text_result(&input.text)),
                    maestro_tools::ToolRenderResultOptions {
                        expanded: input.expanded,
                        is_partial: false,
                    },
                    &env.live,
                    &ctx,
                )
                .unwrap();
                assert!(
                    ctx.last_component
                        .as_ref()
                        .is_none_or(|previous| std::rc::Rc::ptr_eq(previous, &next))
                );
                ctx.last_component = Some(next);
            }
            let row = ctx.last_component.as_ref().unwrap();
            if input.invalidate {
                row.invalidate();
            }
            assert_eq!(row.render(input.width), stage.rows);
            previous_input = Some((input.text, input.expanded));
        }
    }
}

#[test]
fn bash_renderers_read_the_published_theme() {
    for case in cases("bash_renderers_read_the_published_theme") {
        assert_eq!(case.op, "theme");
        assert_eq!(case.input, serde_json::json!({}));
        assert_eq!(case.expected.as_object().unwrap().len(), 2);
        let env = env(0);
        let probe = display::Probe::new(true);
        let mut env = env;
        env.highlighter = probe.clone();
        let setup = setup();
        let args = serde_json::json!({"command":"go"});
        let mut ctx = display::context(&args, &setup, &env, None);
        let tool = definition();
        let decoy = maestro_theme::LiveTheme::from_theme(std::rc::Rc::new(display::bare_theme()));
        let original = tool.render_call.as_ref().unwrap()(Some(&args), &decoy, &ctx).unwrap();
        assert_eq!(
            serde_json::to_value(original.render(30)).unwrap(),
            case.expected["original"]
        );
        env.state
            .set_theme_instance(self::env(20).live.get().unwrap())
            .unwrap();
        ctx.last_component = Some(original);
        let replacement = tool.render_call.as_ref().unwrap()(Some(&args), &decoy, &ctx).unwrap();
        assert_eq!(
            serde_json::to_value(replacement.render(30)).unwrap(),
            case.expected["replacement"]
        );
        let result = crate::display_support::native(
            tool.render_result.as_ref().unwrap(),
            &ToolRenderResultLike::from(&text_result("live")),
            maestro_tools::ToolRenderResultOptions {
                expanded: false,
                is_partial: false,
            },
            &decoy,
            &ctx,
        )
        .unwrap();
        assert_eq!(
            result.render(10),
            [
                String::new(),
                display::padded_to("\x1b[38;5;22mlive\x1b[39m", 10)
            ]
        );
        assert!(probe.calls.borrow().is_empty());
    }
}

#[test]
fn bash_renderers_reuse_matching_components() {
    use std::rc::Rc;
    for case in cases("bash_renderers_reuse_matching_components") {
        assert_eq!(case.op, "reuse");
        assert_eq!(case.input, serde_json::json!({}));
        assert_eq!(case.expected.as_object().unwrap().len(), 4);
        let env = env(0);
        let args = serde_json::json!({"command":"first"});
        let setup = setup();
        let mut ctx = display::context(
            &args,
            &setup,
            &env,
            Some(Rc::new(maestro_tui::Container::new())),
        );
        let tool = definition();
        let first = tool.render_call.as_ref().unwrap()(Some(&args), &env.live, &ctx).unwrap();
        assert!(!Rc::ptr_eq(ctx.last_component.as_ref().unwrap(), &first));
        ctx.last_component = Some(Rc::clone(&first));
        let second = tool.render_call.as_ref().unwrap()(
            Some(&serde_json::json!({"command":"second"})),
            &env.live,
            &ctx,
        )
        .unwrap();
        assert_eq!(
            Value::Bool(Rc::ptr_eq(&first, &second)),
            case.expected["callSame"]
        );
        assert_eq!(
            serde_json::to_value(second.render(20)).unwrap(),
            case.expected["callRows"]
        );
        ctx.last_component = Some(second);
        let options = maestro_tools::ToolRenderResultOptions {
            expanded: false,
            is_partial: false,
        };
        let render = tool.render_result.as_ref().unwrap();
        let old = text_result("old");
        let old = ToolRenderResultLike::from(&old);
        let first = crate::display_support::native(render, &old, options, &env.live, &ctx).unwrap();
        assert!(!Rc::ptr_eq(ctx.last_component.as_ref().unwrap(), &first));
        ctx.last_component = Some(Rc::clone(&first));
        let new = text_result("new");
        let new = ToolRenderResultLike::from(&new);
        let second =
            crate::display_support::native(render, &new, options, &env.live, &ctx).unwrap();
        assert_eq!(
            Value::Bool(Rc::ptr_eq(&first, &second)),
            case.expected["resultSame"]
        );
        assert_eq!(
            serde_json::to_value(second.render(20)).unwrap(),
            case.expected["resultRows"]
        );
    }
}

#[test]
fn bash_expand_hint_changes_on_retained_render() {
    for case in cases("bash_expand_hint_changes_on_retained_render") {
        assert_eq!(case.op, "hint_rebind");
        assert_eq!(case.input, serde_json::json!({}));
        assert_eq!(case.expected.as_object().unwrap().len(), 3);
        let env = env(0);
        let setup = setup();
        let _bindings = display::bind(&display::expand_keys(&render_context()));
        let args = serde_json::json!({});
        let ctx = display::context(&args, &setup, &env, None);
        let tool = definition();
        let row = crate::display_support::native(
            tool.render_result.as_ref().unwrap(),
            &ToolRenderResultLike::from(&text_result("1\n2\n3\n4\n5\n6")),
            maestro_tools::ToolRenderResultOptions {
                expanded: false,
                is_partial: false,
            },
            &env.live,
            &ctx,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(row.render(80)).unwrap(),
            case.expected["before"]
        );
        maestro_tui::set_keybindings(maestro_tui::KeybindingsManager::new(
            vec![(
                "app.tools.expand".into(),
                maestro_tui::KeybindingDefinition {
                    default_keys: maestro_tui::KeybindingKeys::Single("alt+x".into()),
                    description: None,
                },
            )],
            Vec::new(),
        ));
        assert_eq!(
            serde_json::to_value(row.render(80)).unwrap(),
            case.expected["after"]
        );
        env.state
            .set_theme_instance(self::env(20).live.get().unwrap())
            .unwrap();
        assert_eq!(
            serde_json::to_value(row.render(80)).unwrap(),
            case.expected["afterTheme"]
        );
        check_plain_fallback(&row, &case.expected["afterTheme"], &env);
    }
}

/// A retained body survives a later missing-theme failure with an unstyled live hint.
fn check_plain_fallback(row: &maestro_tui::tui::ComponentHandle, expected: &Value, env: &Env) {
    env.state
        .set_theme_instance(std::rc::Rc::new(display::bare_theme()))
        .unwrap();
    let fallback = row.render(80);
    assert_eq!(fallback[1], "... (1 earlier lines, alt+x to expand)");
    assert_eq!(
        &fallback[2..],
        serde_json::from_value::<Vec<String>>(expected.clone()).unwrap()[2..].as_ref()
    );
}

#[test]
fn bash_dropped_row_releases_refresh() {
    use std::{cell::Cell, rc::Rc};
    for case in cases("bash_dropped_row_releases_refresh") {
        assert_eq!(case.op, "drop");
        assert_eq!(case.input, serde_json::json!({}));
        assert_eq!(case.expected.as_object().unwrap().len(), 2);
        let env = env(0);
        let args = serde_json::json!({});
        let mut setup = setup();
        setup.phase.execution_started = true;
        let mut ctx = display::context(&args, &setup, &env, None);
        let count = Rc::new(Cell::new(0));
        let notifications = Rc::clone(&count);
        let sentinel = Rc::new(());
        let weak = Rc::downgrade(&sentinel);
        ctx.invalidate = Rc::new(move || {
            let _ = &sentinel;
            notifications.set(notifications.get() + 1);
        });
        let tool = definition();
        let call = tool.render_call.as_ref().unwrap()(Some(&args), &env.live, &ctx).unwrap();
        let row = crate::display_support::native(
            tool.render_result.as_ref().unwrap(),
            &ToolRenderResultLike::from(&text_result("running")),
            maestro_tools::ToolRenderResultOptions {
                expanded: false,
                is_partial: true,
            },
            &env.live,
            &ctx,
        )
        .unwrap();
        let alias = Rc::clone(&ctx.state);
        drop(ctx);
        assert_eq!(env.runtime.pending(), 1);
        assert!(weak.upgrade().is_some());
        drop(alias);
        assert!(weak.upgrade().is_none());
        assert_eq!(
            serde_json::json!({"timers":env.runtime.pending(),"invalidations":count.get()}),
            case.expected
        );
        advance(&env, 1000);
        assert_eq!(count.get(), 0);
        assert_eq!(
            row.render(10)[1],
            display::padded_to("\x1b[38;5;2mrunning\x1b[39m", 10)
        );
        assert!(!call.render(10).is_empty());
        assert!(tool.render_result.is_some());
    }
}

/// Brand query operands, including the independent expected role mapping.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrandInput {
    /// Shipped theme name.
    mode: String,
    /// Independently recorded role values.
    palette: Value,
    /// Viewport columns.
    width: usize,
}
#[test]
fn bash_default_themes_keep_role_colors() {
    for case in cases("bash_default_themes_keep_role_colors") {
        assert_eq!(case.op, "brand");
        let input: BrandInput = serde_json::from_value(case.input).unwrap();
        assert_eq!(case.expected.as_object().unwrap().len(), 2);
        let theme = display::shipped_theme(&input.mode, Some(maestro_theme::ColorMode::Truecolor));
        let recorded = super::edit_render_support::theme(&input.palette, &serde_json::json!({}));
        for key in input.palette.as_object().unwrap().keys() {
            let color = maestro_theme::ThemeColor::Named(key.clone());
            assert_eq!(
                theme.fg(&color, "sample").unwrap(),
                recorded.fg(&color, "sample").unwrap()
            );
        }
        let env = Env::with(theme, display::Probe::new(true));
        let args = serde_json::json!({"command":"echo hello","timeout":2});
        let mut setup = setup();
        setup.phase.execution_started = true;
        let _bindings = display::bind(&display::expand_keys(&render_context()));
        let ctx = display::context(&args, &setup, &env, None);
        let tool = definition();
        advance(&env, 9000);
        let call = tool.render_call.as_ref().unwrap()(Some(&args), &env.live, &ctx).unwrap();
        assert_eq!(
            serde_json::to_value(call.render(input.width)).unwrap(),
            case.expected["header"]
        );
        advance(&env, 10250);
        let mut result = text_result("1\n2\n3\n4\n5\n6");
        result.details = Some(maestro_tools::BashToolDetails {
            full_output_path: Some("/tmp/output.log".into()),
            truncation: Some(maestro_tools::truncate_head(
                &(1..=20)
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
                maestro_tools::TruncationOptions {
                    max_lines: Some(5),
                    max_bytes: None,
                },
            )),
        });
        let row = crate::display_support::native(
            tool.render_result.as_ref().unwrap(),
            &ToolRenderResultLike::from(&result),
            maestro_tools::ToolRenderResultOptions {
                expanded: false,
                is_partial: false,
            },
            &env.live,
            &ctx,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(row.render(input.width)).unwrap(),
            case.expected["body"]
        );
    }
}
