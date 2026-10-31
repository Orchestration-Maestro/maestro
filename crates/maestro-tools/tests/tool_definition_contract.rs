//! Tool definitions: wrapping, conversion and retained render contracts.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::display_support;

use display_support::NoLanguages;
use futures_util::FutureExt;
use maestro_agent::{
    AgentTool, AgentToolResult, AgentToolUpdateCallback, ExecuteTool, PrepareArguments,
    ToolExecutionMode,
};
use maestro_models::{Cancellation, DiagnosticErrorInfo, TextContent, Tool, UserBlock};
use maestro_theme::{ColorMode, LiveTheme, Theme, ThemeDirectories, ThemeOptions, ThemeState};
use maestro_tools::{
    ToolContextFactory, ToolDefinition, ToolRenderContext, ToolRenderPhase,
    ToolRenderResultOptions, ToolRenderShell, create_tool_definition_from_agent_tool, define_tool,
    wrap_tool_definition, wrap_tool_definitions,
};
use maestro_tui::{
    TerminalImage,
    tui::{Component, ComponentHandle},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

/// Typed arguments of the controlled tools.
#[derive(Deserialize)]
struct Args {
    /// Distinguishing operand.
    value: u32,
}

/// What one execution saw: id, operand, whether its signal was aborted, and context.
type Seen = (String, u32, Option<bool>, Option<u32>);

/// Text content for a result.
fn text(text: &str) -> Vec<UserBlock> {
    vec![UserBlock::Text(TextContent::new(text.into()))]
}

/// A caller failure with every field set.
fn failure(message: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("ReadFailure".into()),
        message: message.into(),
        stack: Some("controlled stack".into()),
        code: Some(maestro_models::DiagnosticCode::Text("E_CONTROL".into())),
    }
}

/// A factory that hands out 1, 2, 3, ... and counts its calls.
fn counting_factory() -> ToolContextFactory<u32> {
    let next = AtomicU32::new(0);
    Arc::new(move || Ok(next.fetch_add(1, Ordering::SeqCst) + 1))
}

/// A definition that logs each execution, reports one progress update and answers by operand.
fn definition(name: &str, log: &Arc<Mutex<Vec<Seen>>>) -> ToolDefinition<u32, Args, Value, ()> {
    let log = Arc::clone(log);
    ToolDefinition {
        definition: Tool {
            name: name.into(),
            description: "Transform input".into(),
            parameters: json!({"type": "object"}).into(),
        },
        label: "Transform".into(),
        prompt_snippet: Some("snippet".into()),
        prompt_guidelines: Some(vec!["guideline".into()]),
        render_shell: Some(ToolRenderShell::SelfRendered),
        prepare_arguments: Some(Arc::new(|args| Ok(args.clone()))),
        execution_mode: Some(ToolExecutionMode::Parallel),
        execute: Arc::new(move |id, args, signal, update, context| {
            let aborted = signal.map(|signal| signal.is_aborted());
            log.lock().unwrap().push((id, args.value, aborted, context));
            if let Some(update) = update {
                update(AgentToolResult {
                    content: text("progress"),
                    details: Some(json!({"stage": 1})),
                    terminate: None,
                    usage: None,
                });
            }
            let details = (args.value != 6).then_some(Value::Null);
            Box::pin(async move {
                Ok(AgentToolResult {
                    content: text("done"),
                    details,
                    terminate: None,
                    usage: None,
                })
            })
        }),
        render_call: None,
        render_result: None,
    }
}

/// Run an erased tool to completion; every controlled body is ready at once.
fn execute(
    tool: &AgentTool,
    id: &str,
    value: u32,
    signal: Option<Cancellation>,
    partials: Option<&Arc<Mutex<Vec<Option<Value>>>>>,
) -> Result<AgentToolResult, DiagnosticErrorInfo> {
    let update = partials.map(|partials| {
        let partials = Arc::clone(partials);
        Arc::new(move |partial: AgentToolResult| partials.lock().unwrap().push(partial.details))
            as AgentToolUpdateCallback
    });
    (tool.execute)(id.into(), json!({"value": value}), signal, update)
        .now_or_never()
        .unwrap()
}

#[test]
fn wrapper_forwards_metadata_preparation_progress_and_fresh_context() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let source = definition("transform", &log);
    let prepare: PrepareArguments = source.prepare_arguments.clone().unwrap();
    let tool = wrap_tool_definition(source, Some(counting_factory()));
    assert_eq!(tool.definition.name, "transform");
    assert_eq!(tool.definition.description, "Transform input");
    assert_eq!(*tool.definition.parameters, json!({"type": "object"}));
    assert_eq!(tool.label, "Transform");
    assert_eq!(tool.execution_mode, Some(ToolExecutionMode::Parallel));
    assert!(Arc::ptr_eq(
        tool.prepare_arguments.as_ref().unwrap(),
        &prepare
    ));

    let partials = Arc::new(Mutex::new(Vec::new()));
    let first = execute(&tool, "one", 6, None, Some(&partials)).unwrap();
    let aborted = Cancellation::new();
    aborted.abort();
    let second = execute(&tool, "two", 9, Some(aborted), Some(&partials)).unwrap();
    assert_eq!(
        *log.lock().unwrap(),
        [
            ("one".to_owned(), 6, None, Some(1)),
            ("two".to_owned(), 9, Some(true), Some(2)),
        ]
    );
    assert_eq!(
        *partials.lock().unwrap(),
        vec![Some(json!({"stage": 1})), Some(json!({"stage": 1}))]
    );
    assert_eq!(first.details, None);
    assert_eq!(second.details, Some(Value::Null));

    let bare = wrap_tool_definition(definition("bare", &log), None);
    execute(&bare, "three", 7, None, None).unwrap();
    assert_eq!(log.lock().unwrap()[2], ("three".to_owned(), 7, None, None));
}

#[test]
fn wrapper_keeps_errors_and_context_failure_order() {
    let invoked = Arc::new(AtomicU32::new(0));
    let counted = Arc::clone(&invoked);
    let mut failing = definition("failing", &Arc::new(Mutex::new(Vec::new())));
    failing.execute = Arc::new(move |_, _, _, _, _| {
        counted.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(failure("body failed")) })
    });
    let tool = wrap_tool_definition(failing, Some(counting_factory()));
    assert_eq!(
        execute(&tool, "id", 1, None, None).err(),
        Some(failure("body failed"))
    );
    assert_eq!(invoked.load(Ordering::SeqCst), 1);

    let counted = Arc::clone(&invoked);
    let mut unreachable_body = definition("unreachable", &Arc::new(Mutex::new(Vec::new())));
    unreachable_body.execute = Arc::new(move |_, _, _, _, _| {
        counted.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(failure("body ran")) })
    });
    let factory: ToolContextFactory<u32> = Arc::new(|| Err(failure("no context")));
    let tool = wrap_tool_definition(unreachable_body, Some(factory));
    assert_eq!(
        execute(&tool, "id", 1, None, None).err(),
        Some(failure("no context"))
    );
    assert_eq!(invoked.load(Ordering::SeqCst), 1);
}

#[test]
fn wrapper_plural_keeps_input_order_and_duplicates() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let tools = wrap_tool_definitions(
        vec![
            definition("transform", &log),
            definition("other", &log),
            definition("transform", &log),
        ],
        Some(counting_factory()),
    );
    let names: Vec<_> = tools
        .iter()
        .map(|tool| tool.definition.name.as_str())
        .collect();
    assert_eq!(names, ["transform", "other", "transform"]);
    for (index, tool) in tools.iter().enumerate().rev() {
        execute(tool, &format!("call-{index}"), 1, None, None).unwrap();
    }
    let contexts: Vec<_> = log
        .lock()
        .unwrap()
        .iter()
        .map(|seen| (seen.0.clone(), seen.3))
        .collect();
    assert_eq!(
        contexts,
        [
            ("call-2".to_owned(), Some(1)),
            ("call-1".to_owned(), Some(2)),
            ("call-0".to_owned(), Some(3))
        ]
    );
}

/// What the controlled agent tool saw: id, arguments and whether its signal was aborted.
type Call = (String, Value, Option<bool>);

/// An agent tool that logs each call, reports one progress update and answers with stage 2.
fn agent_tool(log: &Arc<Mutex<Vec<Call>>>, prepare: &PrepareArguments) -> AgentTool {
    let seen = Arc::clone(log);
    let body: ExecuteTool = Arc::new(move |id, args, signal, update| {
        seen.lock()
            .unwrap()
            .push((id, args, signal.map(|signal| signal.is_aborted())));
        if let Some(update) = update {
            update(AgentToolResult {
                content: text("progress"),
                details: Some(json!({"stage": 1})),
                terminate: None,
                usage: None,
            });
        }
        Box::pin(async {
            Ok(AgentToolResult {
                content: text("done"),
                details: Some(json!({"stage": 2})),
                terminate: None,
                usage: None,
            })
        })
    });
    AgentTool {
        description: None,
        definition: Tool {
            name: "transform".into(),
            description: "Transform input".into(),
            parameters: json!({"type": "object"}).into(),
        },
        label: "Transform".into(),
        prepare_arguments: Some(Arc::clone(prepare)),
        execute: body,
        execution_mode: Some(ToolExecutionMode::Parallel),
    }
}

#[test]
fn agent_tool_conversion_forwards_execution_without_render_metadata() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let prepare: PrepareArguments = Arc::new(|args| Ok(args.clone()));
    let tool = agent_tool(&log, &prepare);
    let converted = create_tool_definition_from_agent_tool::<u32>(tool).unwrap();
    assert_eq!(converted.definition.name, "transform");
    assert_eq!(converted.label, "Transform");
    assert_eq!(converted.execution_mode, Some(ToolExecutionMode::Parallel));
    assert!(Arc::ptr_eq(
        converted.prepare_arguments.as_ref().unwrap(),
        &prepare
    ));
    assert!(converted.prompt_snippet.is_none() && converted.prompt_guidelines.is_none());
    assert!(
        converted.render_shell.is_none()
            && converted.render_call.is_none()
            && converted.render_result.is_none()
    );

    let signal = Cancellation::new();
    signal.abort();
    let partials = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&partials);
    let update: AgentToolUpdateCallback =
        Arc::new(move |partial| captured.lock().unwrap().push(partial.details));
    let result = (converted.execute)(
        "converted".into(),
        json!({"value": 10}),
        Some(signal),
        Some(update),
        Some(41),
    )
    .now_or_never()
    .unwrap()
    .unwrap();
    assert_eq!(result.details, Some(json!({"stage": 2})));
    assert_eq!(*partials.lock().unwrap(), [Some(json!({"stage": 1}))]);
    assert_eq!(
        *log.lock().unwrap(),
        [("converted".to_owned(), json!({"value": 10}), Some(true))]
    );
}

/// Preparation that adds ten to `value`.
fn add_ten() -> PrepareArguments {
    Arc::new(|args| Ok(json!({"value": args["value"].as_u64().unwrap() + 10})))
}

#[test]
fn wrapper_preserves_transforming_preparation() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut source = definition("transform", &log);
    source.prepare_arguments = Some(add_ten());
    let tool = wrap_tool_definition(source, None);
    let prepared = (tool.prepare_arguments.as_ref().unwrap())(&mut json!({"value": 3})).unwrap();
    assert_eq!(prepared, json!({"value": 13}));
    assert!(log.lock().unwrap().is_empty());
    (tool.execute)("id".into(), prepared, None, None)
        .now_or_never()
        .unwrap()
        .unwrap();
    assert_eq!(*log.lock().unwrap(), [("id".to_owned(), 13, None, None)]);
}

#[test]
fn agent_tool_conversion_preserves_transforming_preparation() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let converted =
        create_tool_definition_from_agent_tool::<u32>(agent_tool(&log, &add_ten())).unwrap();
    let prepared =
        (converted.prepare_arguments.as_ref().unwrap())(&mut json!({"value": 8})).unwrap();
    assert_eq!(prepared, json!({"value": 18}));
    assert!(log.lock().unwrap().is_empty());
    (converted.execute)("id".into(), prepared, None, None, None)
        .now_or_never()
        .unwrap()
        .unwrap();
    assert_eq!(
        *log.lock().unwrap(),
        [("id".to_owned(), json!({"value": 18}), None)]
    );
}

#[test]
fn agent_tool_conversion_preserves_execution_errors() {
    let invoked = Arc::new(AtomicU32::new(0));
    let counted = Arc::clone(&invoked);
    let mut tool = agent_tool(&Arc::new(Mutex::new(Vec::new())), &add_ten());
    tool.execute = Arc::new(move |_, _, _, _| {
        counted.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(failure("reverse failed")) })
    });
    let converted = create_tool_definition_from_agent_tool::<u32>(tool).unwrap();
    let error = (converted.execute)("id".into(), json!({"value": 1}), None, None, None)
        .now_or_never()
        .unwrap()
        .err();
    assert_eq!(error, Some(failure("reverse failed")));
    assert_eq!(invoked.load(Ordering::SeqCst), 1);
}

/// A rendered row recording nothing; only its identity matters.
struct Row;

impl Component for Row {
    fn render(&self, _: usize) -> Vec<String> {
        Vec::new()
    }
}

/// Typed call arguments of the execution; renderers never see this type.
struct CallArgs;

/// What the result renderer saw.
#[derive(Debug, PartialEq)]
struct Observed {
    args: Value,
    calls: u32,
    id: String,
    options: (bool, bool),
    flags: [bool; 6],
    cwd: String,
    readme: String,
}

/// The six row flags in the order execution started, args complete, partial, expanded, images, error.
type Flags = [bool; 6];

/// A definition whose renderers share row state and log what the result renderer saw.
fn render_definition(
    observed: &Arc<Mutex<Vec<Observed>>>,
) -> ToolDefinition<(), CallArgs, Value, u32> {
    let seen = Arc::clone(observed);
    ToolDefinition {
        definition: Tool {
            name: "render".into(),
            description: String::new(),
            parameters: Value::Null.into(),
        },
        label: "render".into(),
        prompt_snippet: None,
        prompt_guidelines: None,
        render_shell: Some(ToolRenderShell::Default),
        prepare_arguments: None,
        execution_mode: None,
        execute: Arc::new(|_, _, _, _, _| Box::pin(async { Err(failure("unused")) })),
        render_call: Some(Arc::new(|args, _, context| {
            if args.unwrap()["fail"] == json!(true) {
                return Err(failure("call failed"));
            }
            *context.state.borrow_mut() += 1;
            (context.invalidate)();
            Ok(Rc::new(Row))
        })),
        render_result: Some(maestro_tools::ToolRenderResult::<
            Value,
            u32,
            maestro_models::Threaded,
        >::Typed(Arc::new(move |_, options, _, context| {
            *context.state.borrow_mut() += 1;
            seen.lock().unwrap().push(Observed {
                args: context.args.cloned().unwrap(),
                calls: *context.state.borrow(),
                id: context.tool_call_id.clone(),
                options: (options.expanded, options.is_partial),
                flags: [
                    context.phase.execution_started,
                    context.phase.args_complete,
                    context.phase.is_partial,
                    context.expanded,
                    context.show_images,
                    context.is_error,
                ],
                cwd: context.cwd.clone(),
                readme: context.environment.readme_path.clone(),
            });
            Ok(context.last_component.clone().unwrap())
        }))),
    }
}

/// A row context with the given flags over borrowed arguments and shared state.
fn row_context<'a>(
    flags: Flags,
    args: &'a Value,
    state: &Rc<RefCell<u32>>,
    invalidations: &Rc<RefCell<u32>>,
) -> ToolRenderContext<'a, u32> {
    let counter = Rc::clone(invalidations);
    let themes = ThemeState::new(ThemeDirectories {
        themes_dir: String::new(),
        custom_themes_dir: String::new(),
    });
    ToolRenderContext {
        renderer_state: None,
        args: Some(args),
        tool_call_id: "stable".into(),
        invalidate: Rc::new(move || *counter.borrow_mut() += 1),
        last_component: None,
        state: Rc::clone(state),
        cwd: "/work".into(),
        phase: ToolRenderPhase {
            execution_started: flags[0],
            args_complete: flags[1],
            is_partial: flags[2],
        },
        expanded: flags[3],
        show_images: flags[4],
        is_error: flags[5],
        environment: Rc::new(maestro_tools::ToolRenderEnvironment {
            runtime: Rc::new(display_support::manual_runtime::ManualRuntime::default()),
            readme_path: "/opt/maestro/README.md".into(),
            live_theme: themes.theme(),
            highlighter: Rc::new(NoLanguages),
            terminal_image: TerminalImage::default(),
        }),
    }
}

/// A failing call reports its error, and typed execution takes its own parameter type.
fn assert_failure_and_typed_execution(
    tool: &ToolDefinition<(), CallArgs, Value, u32>,
    theme: &LiveTheme,
    invalidations: &Rc<RefCell<u32>>,
) {
    let failing = json!({"fail": true});
    let context = row_context(
        [false; 6],
        &failing,
        &Rc::new(RefCell::new(0)),
        invalidations,
    );
    let error = (tool.render_call.as_ref().unwrap())(Some(&failing), theme, &context)
        .err()
        .unwrap();
    assert_eq!(error.message, "call failed");
    let outcome = (tool.execute)("id".into(), CallArgs, None, None, None)
        .now_or_never()
        .unwrap();
    assert_eq!(outcome.err().unwrap().message, "unused");
}

#[test]
fn tool_definition_borrows_each_calls_arguments_and_keeps_row_state() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let tool = define_tool(render_definition(&observed));
    let theme = LiveTheme::from_theme(Rc::new(
        Theme::new(
            Vec::new(),
            Vec::new(),
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    ));
    let empty = AgentToolResult {
        content: Vec::new(),
        details: None,
        terminate: None,
        usage: None,
    };
    let invalidations = Rc::new(RefCell::new(0));
    for flipped in 0..6 {
        let flags: Flags = std::array::from_fn(|index| index == flipped);
        let (first, second) = (
            json!({"path": 7, "stage": flipped}),
            json!({"path": null, "stage": flipped + 10}),
        );
        let state = Rc::new(RefCell::new(0));
        let mut context = row_context(flags, &first, &state, &invalidations);
        let row: ComponentHandle =
            (tool.render_call.as_ref().unwrap())(Some(&first), &theme, &context).unwrap();
        context.last_component = Some(Rc::clone(&row));
        let options = ToolRenderResultOptions {
            expanded: !flags[3],
            is_partial: !flags[2],
        };
        let later = ToolRenderContext {
            renderer_state: None,
            args: Some(&second),
            ..context
        };
        let again = crate::display_support::native(
            tool.render_result.as_ref().unwrap(),
            &maestro_tools::output::render_utils::ToolRenderResultLike::from(&empty),
            options,
            &theme,
            &later,
        )
        .unwrap();
        assert!(Rc::ptr_eq(&row, &again));
        assert_eq!(*state.borrow(), 2);
        let last = observed
            .lock()
            .unwrap()
            .last()
            .map(|seen| (seen.flags, seen.options, seen.args.clone()));
        assert_eq!(last, Some((flags, (!flags[3], !flags[2]), second)));
    }
    assert_eq!(*invalidations.borrow(), 6);
    check_render_observations(&observed);
    assert_failure_and_typed_execution(&tool, &theme, &invalidations);
}

#[test]
fn tool_context_preserves_missing_arguments_and_environment_handles() {
    let env =
        display_support::Env::with(Rc::new(display_support::bare_theme()), Rc::new(NoLanguages));
    let args = Value::Null;
    let setup = display_support::Setup {
        readme: "/opt/maestro/README.md".into(),
        expanded: false,
        cwd: "/work".into(),
        images: None,
        show_images: false,
        is_error: false,
        phase: ToolRenderPhase {
            execution_started: true,
            args_complete: true,
            is_partial: false,
        },
        shift: 0,
    };
    let mut context = display_support::context::<Value>(&args, &setup, &env, None);
    let expected = Rc::clone(&context.environment);
    let selected = env.live.get().unwrap();
    let observations = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&observations);
    let callback: maestro_tools::ToolRenderCall<Value> = Rc::new(move |args, _, context| {
        seen.borrow_mut()
            .push((args.is_some(), context.args.is_some()));
        assert!(Rc::ptr_eq(&expected, &context.environment));
        assert!(Rc::ptr_eq(&expected.runtime, &context.environment.runtime));
        assert!(Rc::ptr_eq(
            &expected.highlighter,
            &context.environment.highlighter
        ));
        assert!(Rc::ptr_eq(
            &selected,
            &context.environment.live_theme.get().unwrap()
        ));
        assert_eq!(context.environment.readme_path, "/opt/maestro/README.md");
        context
            .environment
            .terminal_image
            .set_capabilities(maestro_tui::TerminalCapabilities {
                images: Some(maestro_tui::ImageProtocol::Kitty),
                true_color: true,
                hyperlinks: true,
            });
        Ok(Rc::new(Row))
    });
    assert!(context.args.is_some_and(Value::is_null));
    callback(Some(&args), &env.live, &context).unwrap();
    context.args = None;
    callback(None, &env.live, &context).unwrap();
    assert_eq!(*observations.borrow(), [(true, true), (false, false)]);
    assert_eq!(
        env.terminal.get_capabilities().images,
        Some(maestro_tui::ImageProtocol::Kitty)
    );
}

#[test]
fn repl_tool_conversion_captures_description_once() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../maestro-app/tests/fixtures/javascript_execution/fixtures.json"
    ))
    .unwrap();
    let state = Arc::new(Mutex::new(
        corpus["dynamic"]["changed"].as_str().unwrap().to_owned(),
    ));
    let mut tool = agent_tool(&Arc::new(Mutex::new(vec![])), &add_ten());
    let current = Arc::clone(&state);
    tool.description = Some(Arc::new(move || Ok(current.lock().unwrap().clone())));
    let converted = create_tool_definition_from_agent_tool::<u32>(tool).unwrap();
    *state.lock().unwrap() = corpus["dynamic"]["later"].as_str().unwrap().into();
    let wrapped = wrap_tool_definition(converted, None);
    assert_eq!(
        wrapped.definition.description,
        corpus["dynamic"]["snapshotDescription"]
    );
    assert!(wrapped.description.is_none());
}

/// Every recorded call kept the unchanged context fields.
fn check_render_observations(observed: &Arc<Mutex<Vec<Observed>>>) {
    assert!(
        observed
            .lock()
            .unwrap()
            .iter()
            .all(|seen| seen.id == "stable"
                && seen.cwd == "/work"
                && seen.readme == "/opt/maestro/README.md"
                && seen.calls == 2)
    );
}
