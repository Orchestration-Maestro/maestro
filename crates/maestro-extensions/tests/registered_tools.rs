//! Retained registration through real facades and runner consumers.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::local_support as support;
use maestro_extensions::{ExtensionExecution, types::*};
use maestro_models::{Local, Tool};
use maestro_tools::ToolDefinitionRecord;
use serde_json::{Value, json};
use std::{rc::Rc, sync::RwLock};

use crate::tool_support;
use tool_support::definition;

#[test]
fn tool_replacement_refreshes_after_the_map_update() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("owner", bus.controller.for_extension());
    loaded.api.register_tool(definition("A", "first")).unwrap();
    loaded.api.register_tool(definition("B", "second")).unwrap();
    let weak = Rc::downgrade(&loaded.owner);
    let mut actions = Local::unbound_actions();
    actions.refresh_tools = Rc::new(move || {
        let owner = weak.upgrade().unwrap();
        let owner = owner.read().unwrap();
        assert_eq!(
            owner.tools.keys().map(String::as_str).collect::<Vec<_>>(),
            ["A", "B"]
        );
        assert_eq!(owner.tools["A"].definition.metadata().label, "replacement");
        assert!(std::sync::Arc::ptr_eq(
            &owner.tools["A"].source_info,
            &owner.source_info
        ));
        Err("refresh failure".into())
    });
    *loaded.runtime.actions.write().unwrap() = actions;
    assert_eq!(
        loaded
            .api
            .register_tool(definition("A", "replacement"))
            .unwrap_err()
            .to_string(),
        "refresh failure"
    );
    assert_eq!(
        loaded.owner.read().unwrap().tools["A"]
            .definition
            .metadata()
            .label,
        "replacement"
    );
    loaded.runtime.invalidate(Some("expired".into()));
    assert_eq!(
        loaded
            .api
            .register_tool(definition("C", "ignored"))
            .unwrap_err()
            .to_string(),
        "expired"
    );
    assert_eq!(loaded.owner.read().unwrap().tools.len(), 2);
}

#[test]
fn tool_enumeration_and_lookup_preserve_their_distinct_winner_rules() {
    check_raw_names_and_plural_order();
    let bus = support::LocalBus::new();
    let first = support::loaded("first", bus.controller.for_extension());
    let second = support::loaded("second", bus.controller.for_extension());
    let runner = support::runner(&first.owners);
    assert!(runner.get_all_registered_tools().is_empty());
    first.api.register_tool(definition("key", "first")).unwrap();
    first.api.register_tool(definition("B", "B")).unwrap();
    register_raw_named(&second.api, "key", "second");
    second
        .api
        .register_tool(definition("new", "third"))
        .unwrap();
    first.owners.write().unwrap().push(second.owner);
    let retained = runner
        .get_tool_definition::<Value, Value, ()>("key")
        .unwrap();
    assert_eq!(retained.read().unwrap().label, "first");
    retained.write().unwrap().definition.name = "new".into();
    let tools = runner.get_all_registered_tools();
    assert_eq!(
        tools
            .iter()
            .map(|t| t.definition.name())
            .collect::<Vec<_>>(),
        ["new", "B", "key"]
    );
    assert_eq!(tools[0].definition.metadata().label, "first");
    assert!(
        runner
            .get_tool_definition::<Value, SerializeOnly, RawState>("key")
            .is_none()
    );
    assert!(
        runner
            .get_tool_definition::<Value, Value, ()>("missing")
            .is_none()
    );
    assert!(Rc::ptr_eq(
        &retained,
        &runner
            .get_tool_definition::<Value, Value, ()>("key")
            .unwrap()
    ));
}

#[test]
fn registered_tools_expose_metadata_and_renderers_to_generic_callers() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("owner", bus.controller.for_extension());
    let mut framed = definition("framed", "Framed");
    framed.prompt_snippet = Some("Frame text".into());
    framed.render_shell = Some(maestro_tools::ToolRenderShell::SelfRendered);
    loaded.api.register_tool(framed).unwrap();
    let tools = support::runner(&loaded.owners).get_all_registered_tools();
    let metadata = metadata_of(&tools[0]);
    assert_eq!(
        (metadata.label.as_str(), metadata.prompt_snippet.as_deref()),
        ("Framed", Some("Frame text"))
    );
    let binding = tools[0].renderer_binding().unwrap();
    assert_eq!(
        binding.render_shell,
        Some(maestro_tools::ToolRenderShell::SelfRendered)
    );
    assert!(binding.render_call.is_none() && binding.render_result.is_none());
}

/// Reads metadata where only the execution bound is known.
fn metadata_of<E: ExtensionExecution>(
    tool: &RegisteredTool<E>,
) -> maestro_extensions::ToolDefinitionMetadata {
    tool.metadata()
}

#[test]
fn registered_tools_forward_preparation_context_progress_and_errors() {
    support::local(async {
        check_suspended_context_rebinding().await;
        let bus = support::LocalBus::new();
        let loaded = support::loaded("owner", bus.controller.for_extension());
        let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
        let tool = forwarding_definition(&seen);
        loaded.api.register_tool(tool).unwrap();
        let runner = Rc::new(support::runner(&loaded.owners));
        let registered = runner.get_all_registered_tools();
        let wrapped = maestro_extensions::wrap_registered_tool(&registered[0], runner.clone());
        assert_eq!(
            wrapped.execution_mode,
            Some(maestro_agent::ToolExecutionMode::Sequential)
        );
        let signal = maestro_models::Cancellation::new();
        signal.abort();
        let partials = Rc::new(std::cell::RefCell::new(Vec::new()));
        for updates in [false, true] {
            let mut args = json!({});
            let prepared = wrapped.prepare_arguments.as_ref().unwrap()(&mut args).unwrap();
            let update: Option<maestro_agent::AgentToolUpdateCallback<Value, Local>> = if updates {
                Some(progress_recorder(&partials))
            } else {
                None
            };
            let result = (wrapped.execute)("call".into(), prepared, Some(signal.clone()), update)
                .await
                .unwrap();
            assert_eq!(result.details, Some(json!({"answer":3})));
            assert_eq!(result.terminate, Some(true));
        }
        assert_eq!(*partials.borrow(), [Some(json!({"partial":2}))]);
        assert_eq!(seen.borrow().len(), 2);
        let retained = runner
            .get_tool_definition::<Value, Value, ()>("execute")
            .unwrap();
        let error = maestro_models::DiagnosticErrorInfo {
            message: "body failure".into(),
            name: Some("Body".into()),
            stack: Some("trace".into()),
            code: None,
        };
        let expected = error.clone();
        retained.write().unwrap().execute = Rc::new(move |_, _, _, _, _| {
            let error = error.clone();
            Box::pin(async move { Err(error) })
        });
        assert_eq!(
            (wrapped.execute)("later".into(), json!({}), None, None)
                .await
                .unwrap_err(),
            expected
        );
        bus.settle().await;
    });
}

use crate::display_context;
use maestro_tools::{
    ToolRenderResult, ToolRenderResultOptions, ToolRendererState,
    output::render_utils::ToolRenderResultLike,
};

#[test]
fn render_callbacks_keep_result_first_arguments_and_shared_state() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("render", bus.controller.for_extension());
    let mut tool = definition("render", "render");
    tool.render_call = Some(Rc::new(|args, _, row| {
        assert_eq!(args, Some(&json!({"arg":4})));
        assert_eq!(row.tool_call_id, "call");
        (row.invalidate)();
        Ok(Rc::new(maestro_tui::Text::new("call".into(), 0, 0, None)))
    }));
    tool.render_result = Some(ToolRenderResult::<Value, (), Local>::Typed(Rc::new(
        |result, options, _, row| {
            assert_eq!(result.details, Some(&json!({"result":5})));
            assert_eq!(result.terminate, Some(true));
            assert!(options.expanded);
            assert!(!options.is_partial);
            assert_eq!(row.cwd, "/work");
            assert!(row.last_component.is_some());
            Ok(row.last_component.clone().unwrap())
        },
    )));
    loaded.api.register_tool(tool).unwrap();
    let runner = support::runner(&loaded.owners);
    let tool = runner.get_all_registered_tools().remove(0);
    let binding = tool.definition.renderer_binding().unwrap();
    let args = json!({"arg":4});
    let setup = display_context::Setup::from(
        &json!({"readme":"/opt/maestro/README.md","bindings":[],"showImages":false}),
    );
    let env = display_context::Env::from_setup(&setup);
    let mut row = display_context::context::<ToolRendererState>(&args, &setup, &env, None);
    row.tool_call_id = "call".into();
    let call = binding.render_call.as_ref().unwrap()(Some(&args), &env.live, &row).unwrap();
    assert_eq!(call.render(30), [format!("{:<30}", "call")]);
    row.last_component = Some(call.clone());
    let details = json!({"result":5});
    let prepared = binding
        .render_result
        .unwrap()
        .prepare(ToolRenderResultLike {
            content: &[],
            details: Some(&details),
            terminate: Some(true),
            is_error: Some(false),
        })
        .unwrap();
    let returned = prepared
        .render(
            ToolRenderResultOptions {
                expanded: true,
                is_partial: false,
            },
            &env.live,
            &row,
        )
        .unwrap();
    assert!(Rc::ptr_eq(&returned, &call));
}

#[derive(serde::Serialize)]
struct SerializeOnly {
    value: u32,
}
#[derive(Default)]
struct RawState(u32);

#[test]
fn raw_registration_keeps_serialize_only_details() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("raw", bus.controller.for_extension());
        let definition = raw_definition();
        loaded
            .api
            .register_tool(RegisteredToolInput::from_raw(definition))
            .unwrap();
        let runner = Rc::new(support::runner(&loaded.owners));
        let registered = runner.get_all_registered_tools().remove(0);
        let wrapped = maestro_extensions::wrap_registered_tool(&registered, runner.clone());
        let handle = runner
            .get_tool_definition::<Value, SerializeOnly, RawState>("raw")
            .unwrap();
        let binding = registered.definition.renderer_binding().unwrap();
        let args = json!({});
        let setup =
            display_context::Setup::from(&json!({"readme":"/opt/maestro/README.md","bindings":[]}));
        let env = display_context::Env::from_setup(&setup);
        let row = display_context::context::<ToolRendererState>(&args, &setup, &env, None);
        for (value, count) in [(7, 1), (9, 2)] {
            let result = (wrapped.execute)("raw".into(), json!({}), None, None)
                .await
                .unwrap();
            assert_eq!(result.details, Some(json!({"value":value})));
            let rendered = render_raw_result(&binding, &result, &env, &row);
            assert_eq!(
                rendered.render(30),
                [format!("{:<30}", format!("{value}:{count}"))]
            );
            handle.write().unwrap().execute =
                Rc::new(|_, _, _, _, _| ready_details(Some(SerializeOnly { value: 9 })));
        }
        handle.write().unwrap().definition.name = "no-renderer".into();
        handle.write().unwrap().render_result = None;
        loaded
            .api
            .register_tool(RegisteredToolInput::from_raw(
                maestro_tools::tool_definition_wrapper::ToolDefinitionInput::Retained(
                    handle.clone(),
                ),
            ))
            .unwrap();
        assert!(
            runner
                .get_tool_definition::<Value, SerializeOnly, RawState>("no-renderer")
                .is_some()
        );
        check_deferred_raw_binding(&loaded, &runner, handle).await;
    });
}

#[test]
fn registered_factories_await_initialization_and_preserve_provenance() {
    support::local(async {
        let bus = support::LocalBus::new();
        check_factory_paths(&bus).await;
        for suspended in [false, true] {
            check_factory_initialization(&bus, suspended).await;
        }
        bus.settle().await;
    });
}

#[test]
fn retained_wrappers_observe_execute_mutation_at_the_correct_phase() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("factory-reentry", bus.controller.for_extension());
        let api = loaded.api.clone();
        let owner = Rc::downgrade(&loaded.owner);
        let selected = Rc::new(std::cell::RefCell::new(Vec::new()));
        let raw = mutable_lower_definition();
        let weak_raw = Rc::downgrade(&raw);
        let trace = selected.clone();
        let first = selected.clone();
        raw.write().unwrap().execute = Rc::new(move |_, _, _, _, _| {
            first.borrow_mut().push("A");
            ready_details(None)
        });
        let wrapped = maestro_tools::tool_definition_wrapper::local::wrap_tool_definition(
            maestro_tools::tool_definition_wrapper::ToolDefinitionInput::<
                (),
                Value,
                Value,
                (),
                Local,
                Local,
            >::Retained(raw.clone()),
            Some(Rc::new(move || {
                assert!(owner.upgrade().unwrap().try_write().is_ok());
                api.register_tool(definition("factory-entry", "factory-entry"))
                    .unwrap();
                let trace = trace.clone();
                weak_raw.upgrade().unwrap().write().unwrap().execute =
                    Rc::new(move |_, _, _, _, _| {
                        trace.borrow_mut().push("B");
                        ready_details(None)
                    });
                Ok(())
            })),
        );
        for _ in 0..2 {
            (wrapped.execute)("id".into(), json!({}), None, None)
                .await
                .unwrap();
        }
        assert_eq!(*selected.borrow(), ["A", "B"]);
        assert!(
            loaded
                .owner
                .read()
                .unwrap()
                .tools
                .contains_key("factory-entry")
        );
    });
}

#[test]
fn registered_tool_aliases_release_after_last_consumer() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("aliases", bus.controller.for_extension());
        let released = Rc::new(std::cell::Cell::new(false));
        let capture = support::Released(released.clone());
        let mut tool = definition("old", "old");
        tool.execute = Rc::new(move |_, _, _, _, _| {
            let _ = &capture;
            Box::pin(async {
                Ok(maestro_agent::AgentToolResult {
                    content: vec![],
                    details: Some(json!("old")),
                    terminate: None,
                })
            })
        });
        loaded.api.register_tool(tool).unwrap();
        let runner = Rc::new(support::runner(&loaded.owners));
        let registered = runner.get_all_registered_tools().remove(0);
        let typed = runner
            .get_tool_definition::<Value, Value, ()>("old")
            .unwrap();
        let wrapped = maestro_extensions::wrap_registered_tool(&registered, runner.clone());
        loaded.api.register_tool(definition("old", "new")).unwrap();
        loaded.owners.write().unwrap().clear();
        drop(loaded);
        drop(runner);
        drop(registered);
        drop(typed);
        assert!(!released.get());
        assert_eq!(
            (wrapped.execute)("id".into(), json!({}), None, None)
                .await
                .unwrap()
                .details,
            Some(json!("old"))
        );
        drop(wrapped);
        assert!(released.get());
    });
}

#[derive(serde::Deserialize)]
struct OtherParameters {
    count: u32,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct OtherDetails {
    answer: String,
}

#[test]
fn heterogeneous_definitions_keep_typed_details_and_renderer_state() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("heterogeneous", bus.controller.for_extension());
        register_raw_named(&loaded.api, "json", "JSON");
        let typed = heterogeneous_definition();
        loaded.api.register_tool(typed).unwrap();
        let runner = Rc::new(support::runner(&loaded.owners));
        let registrations = runner.get_all_registered_tools();
        assert_eq!(
            registrations
                .iter()
                .map(|r| r.definition.name())
                .collect::<Vec<_>>(),
            ["json", "typed"]
        );
        let wrappers = maestro_extensions::wrap_registered_tools(&registrations, &runner);
        let first_result = check_raw_execute_replacement(&runner, &wrappers[0]).await;
        assert_eq!(
            (wrappers[1].execute)("t".into(), json!({"count":6}), None, None)
                .await
                .unwrap()
                .details,
            Some(json!({"answer":"6"}))
        );
        let handle = runner
            .get_tool_definition::<OtherParameters, OtherDetails, u32>("typed")
            .unwrap();
        handle.write().unwrap().execute = Rc::new(|_, _, _, _, _| {
            ready_details(Some(OtherDetails {
                answer: "replacement".into(),
            }))
        });
        assert_eq!(
            (wrappers[1].execute)("t".into(), json!({"count":7}), None, None)
                .await
                .unwrap()
                .details,
            Some(json!({"answer":"replacement"}))
        );
        assert!(
            runner
                .get_tool_definition::<Value, Value, ()>("typed")
                .is_none()
        );
        check_heterogeneous_rendering(&registrations, &first_result);
    });
}

/// Records the exact progress details from the erased public operation.
fn progress_recorder(
    partials: &Rc<std::cell::RefCell<Vec<Option<Value>>>>,
) -> maestro_agent::AgentToolUpdateCallback<Value, Local> {
    let partials = partials.clone();
    Rc::new(move |result| {
        partials.borrow_mut().push(result.details);
    })
}

/// Raw rendering keeps Serialize-only details and a retained typed row counter.
fn raw_definition() -> LocalDefinition<Value, SerializeOnly, RawState> {
    let base = definition("raw", "raw");
    ToolDefinitionRecord::<ExtensionContext, Value, SerializeOnly, RawState, Local, Local> {
        definition: base.definition,
        label: base.label,
        prompt_snippet: None,
        prompt_guidelines: None,
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: Rc::new(|_, _, _, _, _| ready_details(Some(SerializeOnly { value: 7 }))),
        render_call: None,
        render_result: Some(ToolRenderResult::<SerializeOnly, RawState, Local>::Raw(
            Rc::new(|result, _, _, row| {
                row.state.borrow_mut().0 += 1;
                let value = result.details.unwrap()["value"].as_u64().unwrap();
                Ok(Rc::new(maestro_tui::Text::new(
                    format!("{value}:{}", row.state.borrow().0),
                    0,
                    0,
                    None,
                )))
            }),
        )),
    }
}

/// Render one execution result through the retained lower raw binding.
fn render_raw_result(
    binding: &maestro_tools::ToolRendererBinding,
    result: &maestro_agent::AgentToolResult,
    env: &display_context::Env,
    row: &maestro_tools::ToolRenderContext<'_, ToolRendererState>,
) -> maestro_tui::tui::ComponentHandle {
    binding
        .render_result
        .as_ref()
        .unwrap()
        .prepare(ToolRenderResultLike {
            content: &result.content,
            details: result.details.as_ref(),
            terminate: None,
            is_error: None,
        })
        .unwrap()
        .render(
            ToolRenderResultOptions {
                expanded: false,
                is_partial: false,
            },
            &env.live,
            row,
        )
        .unwrap()
}

/// Every inline and authored path retains its original provenance spelling.
async fn check_factory_paths(bus: &support::LocalBus) {
    for (path, source, base) in [
        ("", "local", Some(".")),
        ("<inline>", "inline", None),
        ("<temp:one>", "temp", None),
        ("<>", "temporary", None),
        ("<:one>", "temporary", None),
        ("<temporary:>", "temporary", None),
        ("relative/tool.js", "local", Some("relative")),
        ("/abs/tool.js", "local", Some("/abs")),
        ("<prefix", "local", None),
    ] {
        let owner = maestro_extensions::load_extension_from_factory::<Local>(
            Rc::new(|_| Box::pin(async { Ok(()) })),
            "/factory".into(),
            bus.controller.for_extension(),
            maestro_extensions::create_extension_runtime::<Local>(),
            Some(path.into()),
        )
        .await
        .unwrap();
        let owner = owner.read().unwrap();
        assert_eq!(owner.path, path);
        assert_eq!(owner.resolved_path, path);
        let provenance = serde_json::to_value(&*owner.source_info.read().unwrap()).unwrap();
        assert_eq!(provenance["path"], path);
        assert_eq!(provenance["source"], source);
        assert_eq!(provenance["scope"], "temporary");
        assert_eq!(provenance["origin"], "top-level");
        assert_eq!(provenance.get("baseDir").and_then(Value::as_str), base);
        assert_eq!(
            [
                owner.handlers.len(),
                owner.tools.len(),
                owner.commands.len(),
                owner.flags.len(),
                owner.shortcuts.len(),
                owner.message_renderers.len()
            ],
            [0; 6]
        );
    }
}

/// Controlled ready and suspended factory success/error paths.
async fn check_factory_initialization(bus: &support::LocalBus, suspended: bool) {
    let trace = support::Trace::default();
    let received = Rc::new(std::cell::RefCell::new(Vec::new()));
    let output = received.clone();
    let subscription = bus.controller.on(
        "factory-ready",
        Rc::new(move |data, _| {
            output.borrow_mut().push((*data).clone());
            Ok(None)
        }),
    );
    let factory = initializing_factory(&trace, suspended);
    let owner = maestro_extensions::load_extension_from_factory::<Local>(
        factory,
        "/factory".into(),
        bus.controller.for_extension(),
        maestro_extensions::create_extension_runtime::<Local>(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(owner.read().unwrap().path, "<inline>");
    assert_eq!(
        owner
            .read()
            .unwrap()
            .tools
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        ["factory"]
    );
    assert_eq!(
        *trace.borrow(),
        if suspended {
            vec!["registered", "awaited"]
        } else {
            vec!["registered"]
        }
    );
    assert_eq!(*received.borrow(), [json!({"suspended":suspended})]);
    subscription.unsubscribe();
    check_factory_failure(bus, suspended).await;
}

/// A second concrete parameter/details/state family.
fn heterogeneous_definition() -> LocalDefinition<OtherParameters, OtherDetails, u32> {
    let original = definition("typed", "typed");
    ToolDefinitionRecord::<ExtensionContext, OtherParameters, OtherDetails, u32, Local, Local> {
        definition: original.definition,
        label: original.label,
        prompt_snippet: Some(String::new()),
        prompt_guidelines: Some(vec!["guide".into()]),
        render_shell: Some(maestro_tools::ToolRenderShell::SelfRendered),
        prepare_arguments: None,
        execution_mode: Some(maestro_agent::ToolExecutionMode::Parallel),
        execute: Rc::new(|_, params, _, _, context| {
            assert!(context.is_some());
            Box::pin(async move {
                Ok(maestro_agent::AgentToolResult {
                    content: vec![],
                    details: Some(OtherDetails {
                        answer: params.count.to_string(),
                    }),
                    terminate: None,
                })
            })
        }),
        render_call: Some(Rc::new(|_, _, row| {
            *row.state.borrow_mut() += 1;
            Ok(Rc::new(maestro_tui::Text::new(
                format!("typed:{}", row.state.borrow()),
                0,
                0,
                None,
            )))
        })),
        render_result: None,
    }
}

/// A supplied initialization failure keeps its concrete error type.
async fn check_factory_failure(bus: &support::LocalBus, suspended: bool) {
    let error = maestro_extensions::load_extension_from_factory::<Local>(
        Rc::new(move |_| {
            Box::pin(async move {
                if suspended {
                    tokio::task::yield_now().await;
                }
                Err(std::io::Error::other("factory original").into())
            })
        }),
        "/factory".into(),
        bus.controller.for_extension(),
        maestro_extensions::create_extension_runtime::<Local>(),
        None,
    )
    .await
    .err()
    .unwrap();
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().to_string(),
        "factory original"
    );
}

/// Typed row state remains separate from JSON registration state.
fn check_heterogeneous_rendering(
    registrations: &[RegisteredTool<Local>],
    first_result: &maestro_agent::AgentToolResult,
) {
    let args = json!({});
    let setup =
        display_context::Setup::from(&json!({"readme":"/opt/maestro/README.md","bindings":[]}));
    let env = display_context::Env::from_setup(&setup);
    let row = display_context::context::<ToolRendererState>(&args, &setup, &env, None);
    let raw = registrations[0].definition.renderer_binding().unwrap();
    for count in 1..=2 {
        assert_eq!(
            render_raw_result(&raw, first_result, &env, &row).render(3),
            [format!("7:{count}")]
        );
    }
    let binding = registrations[1].definition.renderer_binding().unwrap();
    for count in 1..=2 {
        assert_eq!(
            binding.render_call.as_ref().unwrap()(Some(&args), &env.live, &row)
                .unwrap()
                .render(7),
            [format!("typed:{count}")]
        );
    }
    assert_eq!(row.state.borrow().state::<RawState>().borrow().0, 2);
    assert_eq!(*row.state.borrow().state::<u32>().borrow(), 2);
}

/// A ready local result with no content or termination request.
fn ready_details<D: 'static>(
    details: Option<D>,
) -> <Local as maestro_models::Execution>::Future<
    'static,
    Result<maestro_agent::AgentToolResult<D>, maestro_models::DiagnosticErrorInfo>,
> {
    Box::pin(std::future::ready(Ok(maestro_agent::AgentToolResult {
        content: vec![],
        details,
        terminate: None,
    })))
}
/// Local registered definition with explicit argument, details and state families.
type LocalDefinition<P = Value, D = Value, S = ()> =
    ToolDefinitionRecord<ExtensionContext, P, D, S, Local, Local>;
/// Execution fixture asserts prepared operands and retains real contexts across calls.
fn forwarding_definition(seen: &Rc<std::cell::RefCell<Vec<ExtensionContext>>>) -> LocalDefinition {
    let captured = seen.clone();
    let mut tool = definition("execute", "typed");
    tool.prepare_arguments = Some(Rc::new(|args| {
        args["prepared"] = json!(true);
        Ok(args.clone())
    }));
    tool.execution_mode = Some(maestro_agent::ToolExecutionMode::Sequential);
    tool.execute = Rc::new(move |id, args, signal, update, context| {
        assert_eq!(id, "call");
        assert_eq!(args, json!({"prepared":true}));
        assert!(signal.unwrap().is_aborted());
        let context = context.unwrap();
        assert_eq!(context.cwd().unwrap(), "/work");
        captured.borrow_mut().push(context);
        if let Some(update) = update {
            update(maestro_agent::AgentToolResult {
                content: vec![],
                details: Some(json!({"partial":2})),
                terminate: None,
            });
        }
        Box::pin(async {
            tokio::task::yield_now().await;
            Ok(maestro_agent::AgentToolResult {
                content: vec![],
                details: Some(json!({"answer":3})),
                terminate: Some(true),
            })
        })
    });
    tool
}

/// Raw names remain keys and plural wrapping preserves caller order and duplicates.
fn check_raw_names_and_plural_order() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("names", bus.controller.for_extension());
    let names = [
        "",
        " spaced ",
        "Ω",
        "\u{feff}named",
        "named\u{85}",
        "__proto__",
        "constructor",
    ];
    for name in names {
        loaded.api.register_tool(definition(name, name)).unwrap();
    }
    let runner = Rc::new(support::runner(&loaded.owners));
    let tools = runner.get_all_registered_tools();
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.definition.name())
            .collect::<Vec<_>>(),
        names
    );
    let selected = [tools[1].clone(), tools[0].clone(), tools[1].clone()];
    assert_eq!(
        maestro_extensions::wrap_registered_tools(&selected, &runner)
            .iter()
            .map(|tool| tool.definition.name.as_str())
            .collect::<Vec<_>>(),
        [" spaced ", "", " spaced "]
    );
    assert!(maestro_extensions::wrap_registered_tools(&[], &runner).is_empty());
    for size in [16, 32] {
        let loaded = support::loaded("linear", bus.controller.for_extension());
        for n in 0..size {
            loaded
                .api
                .register_tool(definition(&format!("tool-{n}"), "linear"))
                .unwrap();
        }
        assert_eq!(
            support::runner(&loaded.owners)
                .get_all_registered_tools()
                .len(),
            size
        );
    }
}

use crate::model_support;

/// A pending invocation retains the old model reader, while the next gets the rebinding.
async fn check_suspended_context_rebinding() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("context", bus.controller.for_extension());
    let runner = Rc::new(support::runner(&loaded.owners));
    let first = model_support::model("first");
    let second = model_support::model("second");
    bind_model_reader(&runner, &first);
    let (send, receive) = tokio::sync::oneshot::channel::<()>();
    let pending = Rc::new(std::cell::RefCell::new(Some(receive)));
    let contexts = Rc::new(std::cell::RefCell::new(Vec::new()));
    let captured = contexts.clone();
    let mut tool = definition("context", "context");
    tool.execute = Rc::new(move |_, _, _, _, context| {
        let context = context.unwrap();
        captured.borrow_mut().push(context.clone());
        let pending = pending.borrow_mut().take();
        Box::pin(async move {
            if let Some(pending) = pending {
                pending.await.unwrap();
            }
            let model = context.model().unwrap().unwrap();
            let id = json!(model.read().unwrap().id);
            ready_details(Some(id)).await
        })
    });
    loaded.api.register_tool(tool).unwrap();
    let wrapped = maestro_extensions::wrap_registered_tool(
        &runner.get_all_registered_tools()[0],
        runner.clone(),
    );
    let mut first_call = (wrapped.execute)("first".into(), json!({}), None, None);
    assert!(
        std::future::Future::poll(
            first_call.as_mut(),
            &mut std::task::Context::from_waker(std::task::Waker::noop())
        )
        .is_pending()
    );
    bind_model_reader(&runner, &second);
    assert_eq!(
        (wrapped.execute)("second".into(), json!({}), None, None)
            .await
            .unwrap()
            .details,
        Some(json!("second"))
    );
    assert!(std::sync::Arc::ptr_eq(
        &contexts.borrow()[0].model().unwrap().unwrap(),
        &first
    ));
    assert!(std::sync::Arc::ptr_eq(
        &contexts.borrow()[1].model().unwrap().unwrap(),
        &second
    ));
    send.send(()).unwrap();
    assert_eq!(first_call.await.unwrap().details, Some(json!("first")));
}

/// Bind one distinguishable native model reader without installing application policy.
fn bind_model_reader(
    runner: &maestro_extensions::ExtensionRunner<Local>,
    model: &std::sync::Arc<RwLock<maestro_models::Model>>,
) {
    let model = model.clone();
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_model: Rc::new(move || Ok(Some(model.clone()))),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
}

/// Named raw registrations reuse the existing Serialize-only fixture family.
fn register_raw_named(api: &maestro_extensions::ExtensionAPI<Local>, name: &str, label: &str) {
    let mut tool = raw_definition();
    tool.definition.name = name.into();
    tool.label = label.into();
    api.register_tool(RegisteredToolInput::from_raw(tool))
        .unwrap();
}

/// The first heterogeneous family also observes retained execute replacement.
async fn check_raw_execute_replacement(
    runner: &Rc<maestro_extensions::ExtensionRunner<Local>>,
    wrapper: &maestro_agent::AgentToolRecord<Local>,
) -> maestro_agent::AgentToolResult {
    let first = (wrapper.execute)("j".into(), json!({"json":5}), None, None)
        .await
        .unwrap();
    assert_eq!(first.details, Some(json!({"value":7})));
    let retained = runner
        .get_tool_definition::<Value, SerializeOnly, RawState>("json")
        .unwrap();
    retained.write().unwrap().execute =
        Rc::new(|_, _, _, _, _| ready_details(Some(SerializeOnly { value: 11 })));
    assert_eq!(
        (wrapper.execute)("j2".into(), json!({}), None, None)
            .await
            .unwrap()
            .details,
        Some(json!({"value":11}))
    );
    first
}

/// Callback retirement must leave the registration owner available for re-entry.
struct RetireRegistration {
    /// Weak registration owner whose guard must be released before retirement.
    owner: std::rc::Weak<RwLock<maestro_extensions::Extension<Local>>>,
    /// Weak facade used for a real registration from the destructor.
    api: maestro_extensions::ExtensionAPI<Local>,
    /// Observable destruction completion.
    retired: Rc<std::cell::Cell<bool>>,
}
impl Drop for RetireRegistration {
    fn drop(&mut self) {
        assert!(self.owner.upgrade().unwrap().try_write().is_ok());
        self.api
            .register_tool(definition("retired", "retired"))
            .unwrap();
        self.retired.set(true);
    }
}

#[test]
fn registered_callbacks_reenter_and_retire_without_registry_guards() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("reentry", bus.controller.for_extension());
        let retired = Rc::new(std::cell::Cell::new(false));
        let tool = reentrant_definition(&loaded, &retired);
        loaded
            .api
            .register_tool(
                maestro_tools::tool_definition_wrapper::ToolDefinitionInput::Retained(tool),
            )
            .unwrap();
        let runner = Rc::new(support::runner(&loaded.owners));
        let registered = runner.get_all_registered_tools().remove(0);
        let typed = runner
            .get_tool_definition::<Value, Value, ()>("old")
            .unwrap();
        let binding = registered.definition.renderer_binding().unwrap();
        let wrapped = maestro_extensions::wrap_registered_tool(&registered, runner.clone());
        loaded.api.register_tool(definition("old", "new")).unwrap();
        assert!(!retired.get());
        assert_eq!(
            (wrapped.execute)("id".into(), json!({}), None, None)
                .await
                .unwrap()
                .details,
            Some(json!("old"))
        );
        let args = json!({});
        let setup =
            display_context::Setup::from(&json!({"readme":"/opt/maestro/README.md","bindings":[]}));
        let env = display_context::Env::from_setup(&setup);
        let row = display_context::context::<ToolRendererState>(&args, &setup, &env, None);
        assert_eq!(
            binding.render_call.as_ref().unwrap()(Some(&args), &env.live, &row)
                .unwrap()
                .render(12),
            ["old renderer"]
        );
        for key in ["execute-entry", "render-entry"] {
            assert!(
                loaded.owner.read().unwrap().tools.contains_key(key),
                "{key}"
            );
        }
        assert!(typed.try_write().is_ok());
        drop((registered, typed, wrapped));
        assert!(!retired.get());
        drop(binding);
        assert!(retired.get());
        assert!(loaded.owner.read().unwrap().tools.contains_key("retired"));
        check_unretained_retirement(&loaded, &retired);
    });
}

/// Registration and execution succeed before raw binding rejects a typed slot.
async fn check_deferred_raw_binding(
    loaded: &support::Loaded,
    runner: &Rc<maestro_extensions::ExtensionRunner<Local>>,
    handle: Rc<RwLock<LocalDefinition<Value, SerializeOnly, RawState>>>,
) {
    handle.write().unwrap().definition.name = "typed-on-raw".into();
    handle.write().unwrap().render_result =
        Some(ToolRenderResult::<SerializeOnly, RawState, Local>::Typed(
            Rc::new(|_, _, _, _| panic!("a raw route must not invoke the typed callback")),
        ));
    loaded
        .api
        .register_tool(RegisteredToolInput::from_raw(
            maestro_tools::tool_definition_wrapper::ToolDefinitionInput::Retained(handle),
        ))
        .unwrap();
    let typed_on_raw = runner
        .get_all_registered_tools()
        .into_iter()
        .find(|tool| tool.definition.name() == "typed-on-raw")
        .unwrap();
    let executable = maestro_extensions::wrap_registered_tool(&typed_on_raw, runner.clone());
    assert_eq!(
        (executable.execute)("typed-on-raw".into(), json!({}), None, None)
            .await
            .unwrap()
            .details,
        Some(json!({"value":9}))
    );
    assert_eq!(
        typed_on_raw
            .definition
            .renderer_binding()
            .err()
            .unwrap()
            .message,
        "Typed result callback requires ToolRendererBinding::from_definition"
    );
}

/// Standalone local definition used to distinguish execute-selection phases.
fn mutable_lower_definition()
-> Rc<RwLock<maestro_tools::types::local::ToolDefinition<(), Value, Value, ()>>> {
    Rc::new(RwLock::new(maestro_tools::types::local::ToolDefinition::<
        (),
        Value,
        Value,
        (),
    > {
        definition: Tool {
            name: "lower".into(),
            description: String::new(),
            parameters: json!({}).into(),
        },
        label: "lower".into(),
        prompt_snippet: None,
        prompt_guidelines: None,
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: Rc::new(|_, _, _, _, _| ready_details(None)),
        render_call: None,
        render_result: None,
    }))
}

/// Ready or suspended initialization emits through the facade's supplied bus.
fn initializing_factory(
    trace: &support::Trace,
    suspended: bool,
) -> maestro_extensions::ExtensionFactory<Local> {
    let observed = trace.clone();
    Rc::new(move |api: maestro_extensions::ExtensionAPI<Local>| {
        api.register_tool(definition("factory", "factory")).unwrap();
        support::mark(&observed, "registered");
        let trace = observed.clone();
        Box::pin(async move {
            if suspended {
                tokio::task::yield_now().await;
                support::mark(&trace, "awaited");
            }
            api.events
                .emit("factory-ready", json!({"suspended":suspended}))
                .await;
            Ok(())
        })
            as <Local as maestro_models::Execution>::Future<
                'static,
                Result<(), maestro_extensions::EventError>,
            >
    })
}

/// Execute and render callbacks enter the live registry without retained guards.
fn reentrant_definition(
    loaded: &support::Loaded,
    retired: &Rc<std::cell::Cell<bool>>,
) -> Rc<RwLock<LocalDefinition>> {
    let capture = Rc::new(RetireRegistration {
        owner: Rc::downgrade(&loaded.owner),
        api: loaded.api.clone(),
        retired: retired.clone(),
    });
    let tool = Rc::new(RwLock::new(definition("old", "old")));
    let weak_definition = Rc::downgrade(&tool);
    let (api, owner, held) = (
        loaded.api.clone(),
        Rc::downgrade(&loaded.owner),
        capture.clone(),
    );
    tool.write().unwrap().execute = Rc::new(move |_, _, _, _, _| {
        let _ = &held;
        assert!(weak_definition.upgrade().unwrap().try_write().is_ok());
        assert!(owner.upgrade().unwrap().try_write().is_ok());
        api.register_tool(definition("execute-entry", "execute-entry"))
            .unwrap();
        ready_details(Some(json!("old")))
    });
    let (api, owner) = (loaded.api.clone(), Rc::downgrade(&loaded.owner));
    let weak_definition = Rc::downgrade(&tool);
    tool.write().unwrap().render_call = Some(Rc::new(move |_, _, _| {
        let _ = &capture;
        assert!(weak_definition.upgrade().unwrap().try_write().is_ok());
        assert!(owner.upgrade().unwrap().try_write().is_ok());
        api.register_tool(definition("render-entry", "render-entry"))
            .unwrap();
        Ok(Rc::new(maestro_tui::Text::new(
            "old renderer".into(),
            0,
            0,
            None,
        )))
    }));
    tool
}

/// Replacing the sole registration releases its callback outside the map guard.
fn check_unretained_retirement(loaded: &support::Loaded, retired: &Rc<std::cell::Cell<bool>>) {
    retired.set(false);
    let cleanup = RetireRegistration {
        owner: Rc::downgrade(&loaded.owner),
        api: loaded.api.clone(),
        retired: retired.clone(),
    };
    let mut unretained = definition("unretained", "unretained");
    unretained.execute = Rc::new(move |_, _, _, _, _| {
        let _ = &cleanup;
        ready_details(None)
    });
    loaded.api.register_tool(unretained).unwrap();
    assert!(!retired.get());
    loaded
        .api
        .register_tool(definition("unretained", "replacement"))
        .unwrap();
    assert!(retired.get());
}
