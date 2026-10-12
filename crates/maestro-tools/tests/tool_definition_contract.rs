//! Tool definitions: wrapping, conversion and retained render contracts.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use futures_util::FutureExt;
use maestro_agent::{
    AgentTool, AgentToolResult, AgentToolUpdateCallback, ExecuteTool, PrepareArguments,
    ToolExecutionMode,
};
use maestro_models::{Cancellation, DiagnosticErrorInfo, TextContent, Tool, UserBlock};
use maestro_theme::{ColorMode, Theme, ThemeOptions};
use maestro_tools::{
    ToolContextFactory, ToolDefinition, ToolRenderContext, ToolRenderPhase,
    ToolRenderResultOptions, ToolRenderShell, create_tool_definition_from_agent_tool, define_tool,
    wrap_tool_definition, wrap_tool_definitions,
};
use maestro_tui::tui::{Component, ComponentHandle};
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
    vec![UserBlock::Text(TextContent {
        text: text.into(),
        text_signature: None,
    })]
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
            parameters: json!({"type": "object"}),
        },
        label: "Transform".into(),
        prompt_snippet: Some("snippet".into()),
        prompt_guidelines: Some(vec!["guideline".into()]),
        render_shell: Some(ToolRenderShell::SelfRendered),
        prepare_arguments: Some(Arc::new(Ok)),
        execution_mode: Some(ToolExecutionMode::Parallel),
        execute: Arc::new(move |id, args, signal, update, context| {
            let aborted = signal.map(|signal| signal.is_aborted());
            log.lock().unwrap().push((id, args.value, aborted, context));
            if let Some(update) = update {
                update(AgentToolResult {
                    content: text("progress"),
                    details: Some(json!({"stage": 1})),
                    terminate: None,
                });
            }
            let details = (args.value != 6).then_some(Value::Null);
            Box::pin(async move {
                Ok(AgentToolResult {
                    content: text("done"),
                    details,
                    terminate: None,
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
    assert_eq!(tool.definition.parameters, json!({"type": "object"}));
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
            });
        }
        Box::pin(async {
            Ok(AgentToolResult {
                content: text("done"),
                details: Some(json!({"stage": 2})),
                terminate: None,
            })
        })
    });
    AgentTool {
        definition: Tool {
            name: "transform".into(),
            description: "Transform input".into(),
            parameters: json!({"type": "object"}),
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
    let prepare: PrepareArguments = Arc::new(Ok);
    let tool = agent_tool(&log, &prepare);
    let converted = create_tool_definition_from_agent_tool::<u32>(tool);
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

/// A rendered row recording nothing; only its identity matters.
struct Row;

impl Component for Row {
    fn render(&self, _: usize) -> Vec<String> {
        Vec::new()
    }
}

/// Typed call arguments of the render tests.
struct CallArgs {
    /// Edited by the call renderer.
    path: String,
}

/// What the result renderer saw.
#[derive(Debug, PartialEq)]
struct Observed {
    path: String,
    calls: u32,
    id: String,
    options: (bool, bool),
    flags: [bool; 6],
    cwd: String,
}

/// The six row flags in the order execution started, args complete, partial, expanded, images, error.
type Flags = [bool; 6];

/// A definition whose renderers share row state and log what the result renderer saw.
fn render_definition(
    observed: &Rc<RefCell<Vec<Observed>>>,
) -> ToolDefinition<(), CallArgs, Value, u32> {
    let seen = Rc::clone(observed);
    ToolDefinition {
        definition: Tool {
            name: "render".into(),
            description: String::new(),
            parameters: Value::Null,
        },
        label: "render".into(),
        prompt_snippet: None,
        prompt_guidelines: None,
        render_shell: Some(ToolRenderShell::Default),
        prepare_arguments: None,
        execution_mode: None,
        execute: Arc::new(|_, _, _, _, _| Box::pin(async { Err(failure("unused")) })),
        render_call: Some(Rc::new(|args, _, context| {
            args.borrow_mut().path = "after".into();
            *context.state.borrow_mut() += 1;
            (context.invalidate)();
            Rc::new(Row)
        })),
        render_result: Some(Rc::new(move |_, options, _, context| {
            *context.state.borrow_mut() += 1;
            seen.borrow_mut().push(Observed {
                path: context.args.borrow().path.clone(),
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
            });
            context.last_component.clone().unwrap()
        })),
    }
}

/// A row context with the given flags over shared arguments and state.
fn row_context(
    flags: Flags,
    args: &Rc<RefCell<CallArgs>>,
    state: &Rc<RefCell<u32>>,
    invalidations: &Rc<RefCell<u32>>,
) -> ToolRenderContext<u32, CallArgs> {
    let counter = Rc::clone(invalidations);
    ToolRenderContext {
        args: Rc::clone(args),
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
    }
}

#[test]
fn tool_definition_retains_render_context_identity_and_flags() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let tool = define_tool(render_definition(&observed));
    let theme = Theme::new(
        Vec::new(),
        Vec::new(),
        ColorMode::Truecolor,
        ThemeOptions::default(),
    )
    .unwrap();
    let empty = AgentToolResult {
        content: Vec::new(),
        details: None,
        terminate: None,
    };
    let invalidations = Rc::new(RefCell::new(0));
    for flipped in 0..6 {
        let flags: Flags = std::array::from_fn(|index| index == flipped);
        let args = Rc::new(RefCell::new(CallArgs {
            path: "before".into(),
        }));
        let state = Rc::new(RefCell::new(0));
        let mut context = row_context(flags, &args, &state, &invalidations);
        let row: ComponentHandle =
            (tool.render_call.as_ref().unwrap())(Rc::clone(&args), &theme, &context);
        context.last_component = Some(Rc::clone(&row));
        let options = ToolRenderResultOptions {
            expanded: !flags[3],
            is_partial: !flags[2],
        };
        let again = (tool.render_result.as_ref().unwrap())(&empty, options, &theme, &context);
        assert!(Rc::ptr_eq(&row, &again));
        assert_eq!(args.borrow().path, "after");
        assert_eq!(*state.borrow(), 2);
        let last = observed
            .borrow()
            .last()
            .map(|seen| (seen.flags, seen.options));
        assert_eq!(last, Some((flags, (!flags[3], !flags[2]))));
    }
    assert_eq!(*invalidations.borrow(), 6);
    assert!(observed.borrow().iter().all(|seen| seen.id == "stable"
        && seen.cwd == "/work"
        && seen.path == "after"
        && seen.calls == 2));
}
