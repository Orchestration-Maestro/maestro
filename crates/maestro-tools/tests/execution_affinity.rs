//! Caller-local tool execution through the public factory boundary.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#[cfg(unix)]
use crate::read_support::Fixture as _;
use futures_util::FutureExt;
use maestro_agent::{AgentToolResult, ToolExecutionMode};
use maestro_models::{Cancellation, Tool};
use maestro_tools::local::{ToolDefinition, define_tool, wrap_tool_definition};
use serde_json::{Value, json};
use std::sync::Arc;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// A local definition with no effects, suitable for assigning controlled callbacks.
fn custom(name: &str) -> ToolDefinition<Value> {
    ToolDefinition {
        definition: Tool {
            name: name.into(),
            description: "D".into(),
            parameters: json!({"type":"object"}).into(),
        },
        label: "L".into(),
        prompt_snippet: None,
        prompt_guidelines: None,
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: Rc::new(|_, _, _, _, _| {
            Box::pin(std::future::ready(Ok(AgentToolResult {
                content: vec![],
                details: None,
                terminate: None,
                usage: None,
            })))
        }),
        render_call: None,
        render_result: None,
    }
}
/// A model-facing text block.
fn text(value: &str) -> maestro_models::UserBlock {
    maestro_models::UserBlock::Text(maestro_models::TextContent::new(value.into()))
}
/// Serialize the complete result using its existing optional-field contract.
fn result_value<D: serde::Serialize>(result: AgentToolResult<D>) -> Value {
    serde_json::to_value(result).unwrap()
}
#[test]
fn local_definition_wrapper_keeps_context_and_synchronous_updates() {
    for case in cases("local_definition_wrapper_keeps_context_and_synchronous_updates") {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let signal = Cancellation::new();
        let definition = echo_definition(seen.clone(), signal.clone());
        let callback = definition.execute.clone();
        let definition = define_tool(definition);
        let identity = Rc::ptr_eq(&callback, &definition.execute);
        let count = Rc::new(Cell::new(0));
        let next = count.clone();
        let factory = case.input["contextFactory"].as_bool().unwrap().then(|| {
            Rc::new(move || {
                next.set(next.get() + 1);
                Ok(json!({"n":next.get()}))
            }) as maestro_tools::local::ToolContextFactory<Value>
        });
        let tool = wrap_tool_definition(definition, factory);
        let updates = Rc::new(RefCell::new(Vec::new()));
        let mut results = Vec::new();
        for n in 0..2 {
            let observed = updates.clone();
            let future = (tool.execute)(
                format!("call-{n}"),
                json!({"n":n}),
                Some(signal.clone()),
                Some(Rc::new(move |r| {
                    observed.borrow_mut().push(result_value(r));
                })),
            );
            assert_eq!(seen.borrow().len(), usize::try_from(n + 1).unwrap());
            assert_eq!(updates.borrow().len(), usize::try_from(n + 1).unwrap());
            results.push(result_value(future.now_or_never().unwrap().unwrap()));
        }
        let preparation = tool.prepare_arguments.as_ref().unwrap()(&mut json!({"n":3})).unwrap();
        assert_eq!(
            json!({"seen":*seen.borrow(),"updates":*updates.borrow(),"results":results,"count":count.get(),"identity":identity,"preparation":preparation,"mode":tool.execution_mode.unwrap()}),
            case.expected
        );
    }
}

/// A recorded public query with its complete expected observation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    test: String,
    id: String,
    input: Value,
    expected: Value,
}
/// Read one fixture partition, rejecting duplicate complete queries.
fn cases(test: &str) -> Vec<Case> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Fixture {
        cases: Vec<Case>,
    }
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/execution-affinity.json")).unwrap();
    let mut queries = std::collections::HashSet::new();
    for case in &fixture.cases {
        assert!(queries.insert((case.test.clone(), case.input.to_string())));
        assert!(!case.id.is_empty());
    }
    fixture
        .cases
        .into_iter()
        .filter(|c| c.test == test)
        .collect()
}
/// Complete prompt and declaration projection, preserving optional member absence.
fn metadata<C, P, D, S, E: maestro_tools::ToolExecution>(
    tool: &maestro_tools::ToolDefinitionRecord<C, P, D, S, E>,
) -> Value {
    let mut value = json!({"name":tool.definition.name,"description":tool.definition.description,"parameters":tool.definition.parameters,"label":tool.label});
    if let Some(snippet) = &tool.prompt_snippet {
        value["promptSnippet"] = json!(snippet);
    }
    if let Some(guidelines) = &tool.prompt_guidelines {
        value["promptGuidelines"] = json!(guidelines);
    }
    assert_eq!(
        tool.render_shell,
        if tool.definition.name == "edit" {
            Some(maestro_tools::ToolRenderShell::SelfRendered)
        } else {
            None
        }
    );
    if tool.render_shell.is_some() {
        value["renderShell"] = json!("self");
    }
    value
}
/// Exercise the same public construction calls through each concrete namespace.
macro_rules! factory_matrix {
    ($api:path, $ptr:ident) => {{
        use $api as api;
        let cwd = std::env::current_dir().unwrap().to_str().unwrap().to_owned();
        let manager = Arc::new(maestro_tools::ToolsManager::new("/unused/search".into(), None));
        let individual = |name: &str| match name {
            "read" => metadata(&api::create_read_tool_definition::<()>(cwd.clone(), Default::default())),
            "bash" => metadata(&api::create_bash_tool_definition::<()>(cwd.clone(), String::new(), Default::default())),
            "edit" => metadata(&api::create_edit_tool_definition::<()>(cwd.clone(), Default::default())),
            "write" => metadata(&api::create_write_tool_definition::<()>(cwd.clone(), Default::default())),
            "grep" => metadata(&api::create_grep_tool_definition::<()>(cwd.clone(), Default::default(), manager.clone())),
            "find" => metadata(&api::create_find_tool_definition::<()>(cwd.clone(), Default::default(), manager.clone())),
            "ls" => metadata(&api::create_ls_tool_definition::<()>(cwd.clone(), Default::default())),
            _ => unreachable!(),
        };
        let selected = |definition: api::ToolDef| match definition {
            api::ToolDef::Read(t) => metadata(&t), api::ToolDef::Bash(t) => metadata(&t), api::ToolDef::Edit(t) => metadata(&t), api::ToolDef::Write(t) => metadata(&t), api::ToolDef::Grep(t) => metadata(&t), api::ToolDef::Find(t) => metadata(&t), api::ToolDef::Ls(t) => metadata(&t),
        };
        for case in cases("local_factories_keep_declarations_and_presets") {
            let observed = if case.id == "fresh-construction" {
                let names = case.input["names"].as_array().unwrap();
                json!(names.iter().map(|name| {
                    let name = name.as_str().unwrap();
                    let a=api::create_tool(name,cwd.clone(),Default::default(),String::new(),manager.clone()).unwrap();
                    let b=api::create_tool(name,cwd.clone(),Default::default(),String::new(),manager.clone()).unwrap();
                    json!({"name":name,"freshExecute":!$ptr::ptr_eq(&a.execute,&b.execute)})
                }).collect::<Vec<_>>())
            } else if let Some(factory) = case.input.get("factory") {
                let names: Vec<String> = match factory.as_str().unwrap() {
                    "createCodingToolDefinitions" => api::create_coding_tool_definitions::<()>(cwd.clone(),Default::default(),String::new(),manager.clone()).into_iter().map(|t|selected(t)["name"].as_str().unwrap().to_owned()).collect(),
                    "createReadOnlyToolDefinitions" => api::create_read_only_tool_definitions::<()>(cwd.clone(),Default::default(),String::new(),manager.clone()).into_iter().map(|t|selected(t)["name"].as_str().unwrap().to_owned()).collect(),
                    "createAllToolDefinitions" => api::create_all_tool_definitions::<()>(cwd.clone(),Default::default(),String::new(),manager.clone()).into_values().map(|t|selected(t)["name"].as_str().unwrap().to_owned()).collect(),
                    "createCodingTools" => api::create_coding_tools(cwd.clone(),Default::default(),String::new(),manager.clone()).into_iter().map(|t|t.definition.name).collect(),
                    "createReadOnlyTools" => api::create_read_only_tools(cwd.clone(),Default::default(),String::new(),manager.clone()).into_iter().map(|t|t.definition.name).collect(),
                    "createAllTools" => api::create_all_tools(cwd.clone(),Default::default(),String::new(),manager.clone()).into_values().map(|t|t.definition.name).collect(),
                    _ => unreachable!(),
                }; json!(names)
            } else {
                let name = case.input["name"].as_str().unwrap();
                match case.input.get("selector").and_then(Value::as_str) {
                    Some("createToolDefinition") => selected(api::create_tool_definition::<()>(name,cwd.clone(),Default::default(),String::new(),manager.clone()).unwrap()),
                    Some("createTool") => { let tool = api::create_tool(name,cwd.clone(),Default::default(),String::new(),manager.clone()).unwrap(); json!({"name":tool.definition.name,"description":tool.definition.description,"parameters":tool.definition.parameters,"label":tool.label}) },
                    None => individual(name), _ => unreachable!(),
                }
            }; assert_eq!(observed,case.expected,"{}",case.id);
        }
        assert_eq!(api::create_tool("unknown",cwd,Default::default(),String::new(),manager).err().unwrap().message,"Unknown tool name: unknown");
    }};
}
#[test]
fn local_factories_keep_declarations_and_presets() {
    factory_matrix!(maestro_tools, Arc);
    factory_matrix!(maestro_tools::local, Rc);
}

/// Required operand for the typed decoding boundary.
#[derive(serde::Deserialize)]
struct Args {
    _n: u32,
}
#[test]
fn local_context_failure_precedes_execution() {
    for case in cases("local_context_failure_precedes_execution") {
        assert_eq!(case.input, json!({}));
        let ran = Rc::new(Cell::new(false));
        let invoked = ran.clone();
        let mut definition = custom("failing");
        definition.execute = Rc::new(move |_, _, _, _, _| {
            invoked.set(true);
            Box::pin(async { unreachable!() })
        });
        let diagnostic = maestro_models::DiagnosticErrorInfo {
            message: "no context".into(),
            name: Some("original".into()),
            stack: Some("original stack".into()),
            code: Some(maestro_models::DiagnosticCode::Text("E_CONTEXT".into())),
        };
        let supplied = diagnostic.clone();
        let tool = wrap_tool_definition(definition, Some(Rc::new(move || Err(supplied.clone()))));
        let error = (tool.execute)("id".into(), json!({}), None, None)
            .now_or_never()
            .unwrap()
            .err()
            .unwrap();
        assert_eq!(
            json!({"error":error.message,"ran":ran.get(),"same":error==diagnostic}),
            case.expected
        );
    }

    let count = Rc::new(Cell::new(0));
    let invoked = count.clone();
    let source = custom("typed");
    let typed: ToolDefinition<(), Args> = ToolDefinition {
        definition: source.definition,
        label: source.label,
        prompt_snippet: None,
        prompt_guidelines: None,
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: Rc::new(|_, _, _, _, _| Box::pin(async { unreachable!() })),
        render_call: None,
        render_result: None,
    };
    let tool = wrap_tool_definition(
        typed,
        Some(Rc::new(move || {
            invoked.set(invoked.get() + 1);
            Ok(())
        })),
    );
    assert!(
        (tool.execute)("id".into(), json!({}), None, None)
            .now_or_never()
            .unwrap()
            .is_err()
    );
    assert_eq!(count.get(), 0);
}

#[test]
fn local_wrapper_keeps_duplicates_and_snapshot_execution() {
    use maestro_tools::local::{create_tool_definition_from_agent_tool, wrap_tool_definitions};
    for case in cases("local_wrapper_keeps_duplicates_and_snapshot_execution") {
        match case.input["operation"].as_str().unwrap() {
            "wrapToolDefinitions" => {
                let tools = wrap_tool_definitions(
                    case.input["names"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|n| custom(n.as_str().unwrap()))
                        .collect(),
                    None,
                );
                assert_eq!(
                    json!(
                        tools
                            .into_iter()
                            .map(|t| t.definition.name)
                            .collect::<Vec<_>>()
                    ),
                    case.expected
                );
            }
            "createToolDefinitionFromAgentTool" => {
                assert_eq!(snapshot_observation(), case.expected);
            }
            _ => unreachable!(),
        }
    }
    let mut tool = wrap_tool_definition(custom("failure"), None);
    tool.description = Some(Rc::new(|| {
        Err(maestro_models::DiagnosticErrorInfo::error(
            "description failed",
        ))
    }));
    assert_eq!(
        create_tool_definition_from_agent_tool::<()>(tool)
            .err()
            .unwrap()
            .message,
        "description failed"
    );
}

/// Image detection that suspends until the caller releases it.
struct ImageOps {
    /// Ordered operations including the retained context's model lookup.
    seen: Arc<std::sync::Mutex<Vec<&'static str>>>,
    /// One detection admission, consumed by the first call.
    gate: std::sync::Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}
impl maestro_tools::ReadOperations for ImageOps {
    fn access(
        &self,
        _: String,
    ) -> maestro_models::BoxFuture<Result<(), maestro_models::DiagnosticErrorInfo>> {
        self.seen.lock().unwrap().push("access");
        Box::pin(async { Ok(()) })
    }
    fn detect_image_mime_type(
        &self,
        _: String,
    ) -> maestro_models::BoxFuture<Result<Option<String>, maestro_models::DiagnosticErrorInfo>>
    {
        self.seen.lock().unwrap().push("mime");
        let gate = self.gate.lock().unwrap().take().unwrap();
        Box::pin(async move {
            gate.await.unwrap();
            Ok(Some("image/png".into()))
        })
    }
    fn read_file(
        &self,
        _: String,
    ) -> maestro_models::BoxFuture<Result<Vec<u8>, maestro_models::DiagnosticErrorInfo>> {
        self.seen.lock().unwrap().push("read");
        Box::pin(async { Ok(vec![1, 2, 3]) })
    }
}
/// A caller-local model slot read after the detection future completes.
struct LocalContext {
    /// Caller-owned live model.
    model: Rc<RefCell<Option<Arc<maestro_models::Model>>>>,
    /// Shared operation sequence.
    seen: Arc<std::sync::Mutex<Vec<&'static str>>>,
}
impl maestro_tools::ReadContext for LocalContext {
    fn model(&self) -> Option<Arc<maestro_models::Model>> {
        self.seen.lock().unwrap().push("model");
        self.model.borrow().clone()
    }
}
#[test]
fn local_read_context_is_retained_through_detection() {
    for (input, expected) in cases("local_read_context_is_retained_through_detection")
        .into_iter()
        .map(|c| (c.input, c.expected))
        .chain(std::iter::once((
            json!({"model":"cancel"}),
            json!({"error":"Operation aborted"}),
        )))
    {
        check_read_context(&input, &expected);
    }
}

/// Controlled raw shell execution with synchronous descriptor observation.
struct ShellOps {
    /// Selected settlement.
    outcome: String,
    /// Complete entry and callback transcript.
    calls: Arc<std::sync::Mutex<Vec<Value>>>,
    /// Observer publications, read back before the raw callback returns.
    updates: Arc<std::sync::Mutex<Vec<Value>>>,
}
impl maestro_tools::BashOperations<maestro_models::Local> for ShellOps {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        cwd: &'a str,
        options: maestro_tools::BashExecOptions<
            'a,
            <maestro_models::Local as maestro_tools::BashExecution>::Data<'a>,
        >,
    ) -> <maestro_models::Local as maestro_models::Execution>::Future<
        'a,
        Result<maestro_tools::BashExecResult, maestro_tools::BashError>,
    > {
        assert_eq!(*self.updates.lock().unwrap(), [json!({"content":[]})]);
        assert_eq!(command, "selected");
        assert_eq!(cwd, "/selected");
        assert_eq!(options.timeout, Some(2.0));
        if self.outcome != "empty" {
            let env = options
                .env
                .unwrap()
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_str().unwrap().to_owned(),
                        json!(v.as_ref().unwrap().to_str().unwrap()),
                    )
                })
                .collect::<serde_json::Map<_, _>>();
            self.calls.lock().unwrap().push(json!([
                "operation",
                command,
                cwd,
                options.timeout,
                env
            ]));
            (options.on_data)(b"hello");
            let content = self
                .updates
                .lock()
                .unwrap()
                .iter()
                .map(|u| u["content"].clone())
                .collect::<Vec<_>>();
            self.calls
                .lock()
                .unwrap()
                .push(json!(["after-data", content]));
        }
        let result = match self.outcome.as_str() {
            "success" | "empty" => Ok(maestro_tools::BashExecResult {
                exit_code: Some(0.0),
            }),
            "exit" => Ok(maestro_tools::BashExecResult {
                exit_code: Some(3.0),
            }),
            "aborted" => Err(maestro_tools::BashError::Aborted),
            "timeout" => Err(maestro_tools::BashError::Timeout(2.0)),
            "other" => Err(std::io::Error::other("operation failed").into()),
            _ => unreachable!(),
        };
        Box::pin(std::future::ready(result))
    }
}
#[test]
fn local_bash_progress_keeps_prefix_and_settlement() {
    for case in cases("local_bash_progress_keeps_prefix_and_settlement") {
        let outcome = case.input["outcome"].as_str().unwrap();
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let hook_calls = calls.clone();
        let updates = Arc::new(std::sync::Mutex::new(Vec::new()));
        let values = updates.clone();
        let hook_error = outcome == "hook-error";
        let definition = maestro_tools::local::create_bash_tool_definition::<()>(
            "/original".into(),
            String::new(),
            maestro_tools::local::BashToolOptions {
                operations: Some(Rc::new(ShellOps {
                    outcome: outcome.into(),
                    calls: calls.clone(),
                    updates: updates.clone(),
                })),
                command_prefix: Some("prefix".into()),
                shell_path: None,
                spawn_hook: Some(selected_spawn_hook(hook_calls, hook_error)),
            },
        );
        let local_probe = Rc::new(Cell::new(0));
        let observed = local_probe.clone();
        let future = (definition.execute)(
            "id".into(),
            maestro_tools::BashToolInput {
                command: "command".into(),
                timeout: Some(2.0),
            },
            None,
            Some(Rc::new(move |value| {
                observed.set(observed.get() + 1);
                values.lock().unwrap().push(result_value(value));
            })),
            None,
        );
        assert_eq!(updates.lock().unwrap().len(), usize::from(!hook_error));
        let result = crate::runtime::run(future);
        let mut observed = json!({"calls":*calls.lock().unwrap(),"beforeAwait":updates.lock().unwrap().iter().map(|u|u["content"].clone()).collect::<Vec<_>>(),"updates":*updates.lock().unwrap()});
        match result {
            Ok(result) => observed["result"] = result_value(result),
            Err(error) => observed["error"] = json!(error.message),
        }
        let observed: Value =
            serde_json::from_str(&maestro_json::compact_json(&observed).unwrap()).unwrap();
        assert_eq!(observed, case.expected, "{}", case.id);
        assert_eq!(local_probe.get(), updates.lock().unwrap().len());
    }
}

/// A raw producer whose channel lifetime determines settlement.
struct ChannelOps {
    /// Single execution's controlled raw chunks.
    chunks: std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>>>,
}
impl maestro_tools::BashOperations<maestro_models::Local> for ChannelOps {
    fn exec<'a>(
        &'a self,
        _: &'a str,
        _: &'a str,
        options: maestro_tools::BashExecOptions<
            'a,
            <maestro_models::Local as maestro_tools::BashExecution>::Data<'a>,
        >,
    ) -> <maestro_models::Local as maestro_models::Execution>::Future<
        'a,
        Result<maestro_tools::BashExecResult, maestro_tools::BashError>,
    > {
        let mut chunks = self.chunks.lock().unwrap().take().unwrap();
        Box::pin(async move {
            while let Some(chunk) = chunks.recv().await {
                (options.on_data)(&chunk);
            }
            Ok(maestro_tools::BashExecResult {
                exit_code: Some(0.0),
            })
        })
    }
}
#[test]
fn local_bash_progress_uses_existing_throttle_and_decoder() {
    for case in cases("local_bash_progress_uses_existing_throttle_and_decoder") {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            times: Vec<u64>,
            chunks: Vec<Vec<u8>>,
        }
        let input: Input = serde_json::from_value(case.input).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            tokio::time::pause();
            tokio::time::advance(std::time::Duration::from_millis(input.times[0])).await;
            let (send,chunks)=tokio::sync::mpsc::unbounded_channel();
            let definition=maestro_tools::local::create_bash_tool_definition::<()>("/work".into(),String::new(),maestro_tools::local::BashToolOptions{operations:Some(Rc::new(ChannelOps{chunks:std::sync::Mutex::new(Some(chunks))})),..Default::default()});
            let updates=Rc::new(RefCell::new(Vec::new()));let values=updates.clone();
            let mut future=(definition.execute)("id".into(),maestro_tools::BashToolInput{command:"literal".into(),timeout:None},None,Some(Rc::new(move |r|values.borrow_mut().push(result_value(r)))),None);
            send.send(input.chunks[0].clone()).unwrap();send.send(input.chunks[1].clone()).unwrap();
            assert!(future.as_mut().now_or_never().is_none());
            let contents=|| updates.borrow().iter().map(|u|u["content"].clone()).collect::<Vec<_>>();
            let at_start=contents();
            tokio::time::advance(std::time::Duration::from_millis(input.times[1]-input.times[0])).await;
            assert!(future.as_mut().now_or_never().is_none());
            let before=contents();
            tokio::time::advance(std::time::Duration::from_millis(input.times[2]-input.times[1])).await;
            assert!(future.as_mut().now_or_never().is_none());
            // Deliver the paused clock's due timer wheel before observing the pending operation.
            tokio::time::sleep(std::time::Duration::ZERO).await;
            assert!(future.as_mut().now_or_never().is_none());
            let at_boundary=contents();
            send.send(input.chunks[2].clone()).unwrap();assert!(future.as_mut().now_or_never().is_none());
            send.send(input.chunks[3].clone()).unwrap();drop(send);
            let result=future.await.unwrap();
            assert_eq!(json!({"atStart":at_start,"before":before,"atBoundary":at_boundary,"updates":*updates.borrow(),"result":result_value(result)}),case.expected);
            let weak=Rc::downgrade(&updates);drop(updates);assert!(weak.upgrade().is_none());
        });
    }
}

#[test]
#[cfg(unix)]
fn local_bash_sink_uses_native_child_waiter() {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let root = crate::read_support::Scratch::new().unwrap();
    crate::runtime::run(async {
        let tool = maestro_tools::local::create_bash_tool_definition::<()>(
            root.text(),
            String::new(),
            maestro_tools::local::BashToolOptions::default(),
        );
        let socket = Rc::new(RefCell::new(None::<std::net::TcpStream>));
        let connection = socket.clone();
        let updates = Rc::new(RefCell::new(Vec::new()));
        let values = updates.clone();
        let weak = Rc::downgrade(&values);
        let command = format!(
            "python3 -c \"import socket,os; s=socket.create_connection(('127.0.0.1',{port})); assert s.recv(1)==b'x'; os.write(1,b'one'); assert s.recv(1)==b'x'; os.write(2,b'two')\""
        );
        let mut future = (tool.execute)(
            "id".into(),
            maestro_tools::BashToolInput {
                command,
                timeout: None,
            },
            None,
            Some(Rc::new(move |value| {
                let value = result_value(value);
                if value.pointer("/content/0/text").and_then(Value::as_str) == Some("one") {
                    connection
                        .borrow_mut()
                        .as_mut()
                        .unwrap()
                        .write_all(b"x")
                        .unwrap();
                }
                values.borrow_mut().push(value);
            })),
            None,
        );
        assert_eq!(*updates.borrow(), [json!({"content":[]})]);
        assert!(future.as_mut().now_or_never().is_none());
        let (mut stream, _) = listener.accept().unwrap();
        stream.write_all(b"x").unwrap();
        *socket.borrow_mut() = Some(stream);
        assert_eq!(
            result_value(future.await.unwrap()),
            json!({"content":[{"type":"text","text":"onetwo"}]})
        );
        assert_eq!(
            updates.borrow()[1],
            json!({"content":[{"type":"text","text":"one"}],"details":{}})
        );
        drop(tool);
        drop(updates);
        assert!(weak.upgrade().is_none());
    });
}

#[test]
fn local_renderer_binding_drops_executable_owners() {
    use maestro_tools::ToolRendererState;
    let env = crate::display_support::Env::with(
        crate::display_support::shipped_theme("dark", None),
        Rc::new(crate::display_support::NoLanguages),
    );
    let recorded = json!({"readme":"/opt/maestro/README.md","bindings":[]});
    let setup = crate::display_support::Setup::from(&recorded);
    let _bindings = crate::display_support::bind(&crate::display_support::expand_keys(&recorded));
    let args = json!({});
    let row = crate::display_support::context::<ToolRendererState>(&args, &setup, &env, None);
    check_custom_projection(&env.live, &row);
    check_builtin_projection(&env.live, &row);
}

#[test]
fn threaded_factories_keep_sdk_call_syntax_and_send_futures() {
    fn send_sync<T: Send + Sync>() {}
    fn send(_: &impl Send) {}
    send_sync::<maestro_tools::ExecuteToolDefinition<(), Value, Value>>();
    send_sync::<maestro_tools::ToolContextFactory<()>>();
    send_sync::<maestro_agent::AgentToolUpdateCallback>();
    let _: maestro_tools::ToolsOptions = maestro_tools::ToolsOptions {
        ..Default::default()
    };
    let _: maestro_tools::BashToolOptions = maestro_tools::BashToolOptions {
        ..Default::default()
    };
    let manager = Arc::new(maestro_tools::ToolsManager::new(
        "/unused/search".into(),
        None,
    ));
    let tools = maestro_tools::create_all_tools(
        "/work".into(),
        maestro_tools::ToolsOptions::default(),
        String::new(),
        manager.clone(),
    );
    assert_eq!(tools.len(), 7);
    for tool in tools.into_values() {
        let future = (tool.execute)("id".into(), json!({}), None, None);
        send(&future);
    }
    let definitions = maestro_tools::create_all_tool_definitions::<()>(
        "/work".into(),
        maestro_tools::ToolsOptions::default(),
        String::new(),
        manager,
    );
    for definition in definitions.into_values() {
        macro_rules! future {
            ($tool:ident,$args:expr) => {
                send(&($tool.execute)(
                    "id".into(),
                    serde_json::from_value($args).unwrap(),
                    None,
                    None,
                    None,
                ))
            };
        }
        match definition {
            maestro_tools::ToolDef::Read(t) => future!(t, json!({"path":"f"})),
            maestro_tools::ToolDef::Bash(t) => future!(t, json!({"command":"true"})),
            maestro_tools::ToolDef::Edit(t) => future!(t, json!({"path":"f","edits":[]})),
            maestro_tools::ToolDef::Write(t) => future!(t, json!({"path":"f","content":""})),
            maestro_tools::ToolDef::Grep(t) => future!(t, json!({"pattern":"x"})),
            maestro_tools::ToolDef::Find(t) => future!(t, json!({"pattern":"*"})),
            maestro_tools::ToolDef::Ls(t) => future!(t, json!({})),
        }
    }
}

/// Preserve selected typed arguments while crossing the existing local wrapper.
#[cfg(unix)]
fn wrap_selected<C: 'static>(
    definition: maestro_tools::local::ToolDef<C>,
    context: Option<maestro_tools::local::ToolContextFactory<C>>,
) -> maestro_tools::local::Tool {
    match definition {
        maestro_tools::local::ToolDef::Read(t) => wrap_tool_definition(t, context),
        maestro_tools::local::ToolDef::Bash(t) => wrap_tool_definition(t, context),
        maestro_tools::local::ToolDef::Edit(t) => wrap_tool_definition(t, context),
        maestro_tools::local::ToolDef::Write(t) => wrap_tool_definition(t, context),
        maestro_tools::local::ToolDef::Grep(t) => wrap_tool_definition(t, context),
        maestro_tools::local::ToolDef::Find(t) => wrap_tool_definition(t, context),
        maestro_tools::local::ToolDef::Ls(t) => wrap_tool_definition(t, context),
    }
}
/// Tool arguments that require legacy edit preparation before schema validation.
fn builtin_arguments(name: &str) -> Value {
    match name {
        "bash" => json!({"command":"printf bash-operations"}),
        "edit" => json!({"path":"f","oldText":"a","newText":"b"}),
        _ => crate::factory_support::execution::arguments(name),
    }
}
#[test]
#[cfg(unix)]
fn local_builtin_execution_reuses_existing_operations() {
    use crate::factory_support::execution::Probes;
    use maestro_agent::{AgentMessage, PromptInput};
    use maestro_models::{Execution, Local};
    let root = crate::read_support::Scratch::new().unwrap();
    crate::grep_support::producer(&root, &crate::grep_support::Input::default());
    let probes = Probes::new(false);
    let context_calls = Rc::new(Cell::new(0));
    let count = context_calls.clone();
    let factory: maestro_tools::local::ToolContextFactory<LocalContext> = Rc::new(move || {
        count.set(count.get() + 1);
        Ok(LocalContext {
            model: Rc::new(RefCell::new(None)),
            seen: Arc::default(),
        })
    });
    let tools = builtin_tools(&root, &probes, &factory);
    let names = ["read", "bash", "edit", "write", "grep", "find", "ls"];
    let calls = names
        .iter()
        .map(|name| (*name, *name, builtin_arguments(name)))
        .collect::<Vec<_>>();
    let tasks = Rc::new(RefCell::new(Vec::new()));
    let agent = controlled_local_agent(tools, &calls, &tasks);
    let progress = Rc::new(RefCell::new(Vec::new()));
    let observed = progress.clone();
    let subscription = agent.subscribe(Rc::new(move |event, _| {
        if let maestro_agent::AgentEvent::ToolExecutionUpdate {
            tool_call,
            partial_result,
            ..
        } = &*event.read().unwrap()
        {
            observed
                .borrow_mut()
                .push((tool_call.name.clone(), result_value(partial_result.clone())));
        }
        Box::pin(async { Ok(()) })
    }));
    let result = agent.prompt(PromptInput::Text {
        text: "run controlled tools".into(),
        images: vec![],
    });
    let work: <Local as Execution>::Future<'static, ()> = tasks.borrow_mut().pop().unwrap();
    crate::runtime::run(work);
    crate::runtime::run(result).unwrap();
    subscription.unsubscribe();
    let messages = agent.state().read().unwrap().messages().clone();
    let results = messages
        .read()
        .unwrap()
        .iter()
        .filter_map(|m| match m {
            AgentMessage::ToolResult(r) => Some(r.read().unwrap().clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(context_calls.get(), 7);
    check_builtin_results(&results, &probes, &root, &progress.borrow());
    assert!(tasks.borrow().is_empty());
}

/// Check routed file effects and the local agent's published output.
#[cfg(unix)]
fn check_builtin_results(
    results: &[maestro_models::ToolResultMessage],
    probes: &crate::factory_support::execution::Probes,
    root: &crate::read_support::Scratch,
    progress: &[(String, Value)],
) {
    assert_eq!(
        results
            .iter()
            .map(|r| r.tool_name.as_str())
            .collect::<Vec<_>>(),
        ["read", "bash", "edit", "write", "grep", "find", "ls"]
    );
    for result in results {
        assert!(!result.is_error, "{}", result.tool_name);
        assert!(!result.content.is_empty());
    }
    assert_eq!(results[0].content, vec![text("read-operations")]);
    assert_eq!(results[1].content, vec![text("bash-operations")]);
    assert_eq!(
        json!(probes.edit.script.calls().last().unwrap()),
        json!(["write", format!("{}/f", root.text()), "b c"])
    );
    assert!(!probes.read.log().is_empty());
    assert!(!probes.find.log().is_empty());
    assert!(!probes.ls.log().is_empty());
    assert!(!probes.write.calls().is_empty());
    assert!(!probes.grep.calls.lock().unwrap().is_empty());
    assert_eq!(
        progress
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["bash", "bash"]
    );
    assert_eq!(
        progress[1].1["content"],
        json!([{"type":"text","text":"bash-operations"}])
    );
}

/// Build the recorded callback with retained context and a synchronous update.
fn echo_definition(seen: Rc<RefCell<Vec<Value>>>, signal: Cancellation) -> ToolDefinition<Value> {
    let captured = seen;
    let original_signal = signal;
    let mut definition = custom("local");
    definition.prepare_arguments = Some(Rc::new(|args| {
        Ok(json!({"n":args["n"].as_i64().unwrap()+1}))
    }));
    definition.execution_mode = Some(ToolExecutionMode::Sequential);
    definition.execute = Rc::new(move |id, args, signal, update, context| {
        captured.borrow_mut().push(json!({"id":id,"args":args,"sameSignal":signal.as_ref().is_some_and(|s| { original_signal.abort(); s.is_aborted() }),"context":context}));
        if let Some(update) = update {
            update(AgentToolResult {
                content: vec![text("prefix")],
                details: Some(
                    json!({"n":context.as_ref().map_or(-1, |c| c["n"].as_i64().unwrap())}),
                ),
                terminate: None,
                usage: None,
            });
        }
        let n = args["n"].as_i64().unwrap();
        Box::pin(std::future::ready(Ok(AgentToolResult {
            content: vec![text("done")],
            details: (n == 0).then_some(Value::Null),
            terminate: (n == 0).then_some(false),
            usage: None,
        })))
    });
    definition
}

/// Snapshot a live local declaration once, retaining preparation and execution.
fn snapshot_observation() -> Value {
    let seen = Rc::new(RefCell::new(Value::Null));
    let captured = seen.clone();
    let mut definition = custom("custom");
    definition.execution_mode = Some(ToolExecutionMode::Sequential);
    definition.prepare_arguments = Some(Rc::new(|v| {
        v["prepared"] = json!(true);
        Ok(v.clone())
    }));
    definition.execute = Rc::new(move |id, args, _, _, _| {
        *captured.borrow_mut() = json!([id, args]);
        Box::pin(std::future::ready(Ok(AgentToolResult {
            content: vec![],
            details: Some(Value::Null),
            terminate: None,
            usage: None,
        })))
    });
    let mut tool = wrap_tool_definition(definition, None);
    let descriptions = Rc::new(Cell::new(0));
    let calls = descriptions.clone();
    tool.description = Some(Rc::new(move || {
        calls.set(calls.get() + 1);
        Ok("D".into())
    }));
    let definition =
        maestro_tools::local::create_tool_definition_from_agent_tool::<Value>(tool).unwrap();
    assert_eq!(descriptions.get(), 1);
    let preparation = definition.prepare_arguments.as_ref().unwrap()(&mut json!({"x":1})).unwrap();
    let result = (definition.execute)("id".into(), json!({"x":1}), None, None, None)
        .now_or_never()
        .unwrap()
        .unwrap();
    let mut metadata = metadata(&definition);
    metadata["executionMode"] = json!(definition.execution_mode.unwrap());
    let observed = json!({"metadata":metadata,"seen":*seen.borrow(),"result":result_value(result),"preparation":preparation,"mode":definition.execution_mode.unwrap(),"hasRender":definition.render_call.is_some()||definition.render_result.is_some(),"hasSnippet":definition.prompt_snippet.is_some()});
    assert_eq!(descriptions.get(), 1);
    observed
}

/// Observe one live model policy while detection is pending.
fn check_read_context(input: &Value, expected: &Value) {
    use maestro_models::ModelInput;
    let state = input["model"].as_str().unwrap();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let (release, gate) = tokio::sync::oneshot::channel();
    let slot = Rc::new(RefCell::new(Some(crate::read_support::model(
        if state == "vision" {
            vec![ModelInput::Text, ModelInput::Image]
        } else {
            vec![ModelInput::Text]
        },
    ))));
    let tool = maestro_tools::local::create_read_tool_definition::<LocalContext>(
        "/work".into(),
        maestro_tools::ReadToolOptions {
            auto_resize_images: Some(false),
            operations: Some(Arc::new(ImageOps {
                seen: seen.clone(),
                gate: std::sync::Mutex::new(Some(gate)),
            })),
        },
    );
    let signal = Cancellation::new();
    let context = (state != "none").then(|| LocalContext {
        model: slot.clone(),
        seen: seen.clone(),
    });
    let mut future = (tool.execute)(
        "id".into(),
        maestro_tools::ReadToolInput {
            path: "image".into(),
            offset: None,
            limit: None,
        },
        Some(signal.clone()),
        None,
        context,
    );
    assert!(future.as_mut().now_or_never().is_none());
    assert_eq!(*seen.lock().unwrap(), ["access", "mime"]);
    if state == "changed-after-mime" {
        *slot.borrow_mut() = Some(crate::read_support::model(vec![
            ModelInput::Text,
            ModelInput::Image,
        ]));
    }
    if state == "cancel" {
        signal.abort();
    }
    release.send(()).unwrap();
    let result = future.now_or_never().unwrap();
    if state == "cancel" {
        assert_eq!(json!({"error":result.err().unwrap().message}), *expected);
        assert_eq!(*seen.lock().unwrap(), ["access", "mime"]);
    } else {
        assert_eq!(
            json!({"seen":*seen.lock().unwrap(),"result":result_value(result.unwrap())}),
            *expected
        );
    }
}

/// Replace the shell operands after recording the literal command prefix.
fn selected_spawn_hook(
    hook_calls: Arc<std::sync::Mutex<Vec<Value>>>,
    hook_error: bool,
) -> maestro_tools::local::BashSpawnHook {
    Rc::new(move |mut context| {
        hook_calls
            .lock()
            .unwrap()
            .push(json!(["hook", context.command]));
        if hook_error {
            return Err(maestro_models::DiagnosticErrorInfo::error("hook failed"));
        }
        context.command = "selected".into();
        context.cwd = "/selected".into();
        context.env.clear();
        context.env.insert("SELECTED".into(), Some("yes".into()));
        Ok(context)
    })
}

/// Retain custom renderers without retaining executable captures.
fn check_custom_projection(
    theme: &maestro_theme::LiveTheme,
    row: &maestro_tools::ToolRenderContext<'_, maestro_tools::ToolRendererState>,
) {
    let mut definition = custom("rendered");
    let owner = Rc::new(());
    let weak = Rc::downgrade(&owner);
    definition.execute = Rc::new(move |_, _, _, _, _| {
        assert_eq!(Rc::strong_count(&owner), 1);
        Box::pin(async { unreachable!() })
    });
    definition.render_call = Some(Rc::new(|_, _, row| {
        let calls = row
            .state
            .borrow()
            .get("calls")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            + 1;
        row.state.borrow_mut()["calls"] = json!(calls);
        Ok(Rc::new(maestro_tui::components::Text::new(
            "rendered".into(),
            0,
            0,
            None,
        )))
    }));
    let binding = maestro_tools::ToolRendererBinding::from_definition(&definition);
    drop(definition);
    assert!(weak.upgrade().is_none());
    let call = binding.render_call.unwrap();
    assert_eq!(call(None, theme, row).unwrap().render(8), ["rendered"]);
    assert_eq!(row.state.borrow().state::<Value>().borrow()["calls"], 1);
}

/// Retain builtin renderers without retaining executable captures.
fn check_builtin_projection(
    theme: &maestro_theme::LiveTheme,
    row: &maestro_tools::ToolRenderContext<'_, maestro_tools::ToolRendererState>,
) {
    let manager = Arc::new(maestro_tools::ToolsManager::new(
        "/unused/search".into(),
        None,
    ));
    for name in maestro_tools::ALL_TOOL_NAMES {
        let mut selected = maestro_tools::local::create_tool_definition::<()>(
            &name.to_string(),
            "/work".into(),
            maestro_tools::local::ToolsOptions::default(),
            String::new(),
            manager.clone(),
        )
        .unwrap();
        macro_rules! captured {
            ($tool:ident) => {{
                let owner = Rc::new(());
                let weak = Rc::downgrade(&owner);
                let execute = $tool.execute.clone();
                $tool.execute = Rc::new(move |id, args, signal, update, context| {
                    assert_eq!(Rc::strong_count(&owner), 1);
                    execute(id, args, signal, update, context)
                });
                weak
            }};
        }
        let weak = match &mut selected {
            maestro_tools::local::ToolDef::Read(t) => captured!(t),
            maestro_tools::local::ToolDef::Bash(t) => captured!(t),
            maestro_tools::local::ToolDef::Edit(t) => captured!(t),
            maestro_tools::local::ToolDef::Write(t) => captured!(t),
            maestro_tools::local::ToolDef::Find(t) => captured!(t),
            maestro_tools::local::ToolDef::Grep(t) => captured!(t),
            maestro_tools::local::ToolDef::Ls(t) => captured!(t),
        };
        let binding = selected.renderer_binding().unwrap();
        drop(selected);
        assert!(weak.upgrade().is_none());
        let args = builtin_arguments(&name.to_string());
        let component = binding.render_call.as_ref().unwrap()(Some(&args), theme, row).unwrap();
        assert!(!component.render(80).is_empty());
        assert!(binding.render_result.is_some());
    }
}

/// Bind the existing file probes to local definitions before erasure.
#[cfg(unix)]
fn builtin_tools(
    root: &crate::read_support::Scratch,
    probes: &crate::factory_support::execution::Probes,
    factory: &maestro_tools::local::ToolContextFactory<LocalContext>,
) -> Vec<maestro_agent::SharedAgentTool<maestro_models::Local>> {
    use crate::factory_support::execution::{all_options, manager};
    use std::sync::RwLock;
    let base = probes.options(&all_options());
    let options = maestro_tools::local::ToolsOptions {
        read: base.read,
        edit: base.edit,
        write: base.write,
        grep: base.grep,
        find: base.find,
        ls: base.ls,
        bash: maestro_tools::local::BashToolOptions::default(),
    };
    let definitions = maestro_tools::local::create_all_tool_definitions::<LocalContext>(
        root.text(),
        options,
        String::new(),
        manager(root),
    );
    definitions
        .into_values()
        .map(|definition| {
            Rc::new(RwLock::new(wrap_selected(
                definition,
                Some(factory.clone()),
            )))
        })
        .collect()
}

/// Caller-local submitted work driven explicitly by the test.
#[cfg(unix)]
type LocalTasks =
    Rc<RefCell<Vec<<maestro_models::Local as maestro_models::Execution>::Future<'static, ()>>>>;
/// The existing local agent representation, with no custom message kinds.
#[cfg(unix)]
type LocalAgent = maestro_agent::Agent<maestro_agent::NoCustomMessages, maestro_models::Local>;
/// Retain responses and use the caller's execution facility without a provider.
#[cfg(unix)]
fn controlled_local_agent(
    tools: Vec<maestro_agent::SharedAgentTool<maestro_models::Local>>,
    calls: &[(&str, &str, Value)],
    tasks: &LocalTasks,
) -> Rc<LocalAgent> {
    use maestro_agent::{Agent, AgentInitialState, AgentOptions};
    use std::collections::VecDeque;
    let first = crate::loop_support::assistant("", calls);
    first.write().unwrap().stop_reason = maestro_models::StopReason::ToolUse;
    let responses = Rc::new(RefCell::new(VecDeque::from([
        first,
        crate::loop_support::assistant("done", &[]),
    ])));
    let submitted = tasks.clone();

    Rc::new(Agent::new_local(AgentOptions {
        initial_state: AgentInitialState {
            tools,
            ..Default::default()
        },
        spawn: Some(Rc::new(move |future| submitted.borrow_mut().push(future))),
        stream_fn: Some(Rc::new(move |_, _, _, _| {
            let response = responses.borrow_mut().pop_front().unwrap();
            Box::pin(async {
                let stream = maestro_models::AssistantMessageEventStream::new();
                stream.end(Some(response));
                Ok(stream)
            })
        })),
        ..Default::default()
    }))
}
