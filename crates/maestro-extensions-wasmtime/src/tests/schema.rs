//! Borrowed schema admission, origin and native transport controls.
use crate::{
    guest_calls::Callbacks, lifecycle::replies::Replies, local_resources::Pump,
    schema_registration::code, schema_wire::Shape,
};
use maestro_extensions::{EventBus, Local, SchemaNode};
use serde_json::json;
use std::rc::Rc;

/// The original callback family without background tail work.
fn events() -> EventBus<Local> {
    maestro_extensions::create_event_bus_for::<Local>(Rc::new(drop), Rc::new(|_| {}))
        .for_extension()
}

#[test]
fn schema_identity_cache_matches_actual_captures_and_retires_without_admission() {
    let (_owner, api) = super::api::api(maestro_extensions::create_extension_runtime::<Local>());
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    let (calls, _incoming) = tokio::sync::mpsc::unbounded_channel();
    let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
    let id = callbacks.allocate().unwrap().id;
    let handle = callbacks.retain(id, admission).unwrap();
    let mut schema = code(
        vec![(
            String::new(),
            Shape {
                base: false,
                refinements: 1,
            },
            handle.clone(),
        )],
        &registry,
        &api,
    );
    let node = schema.remove("").unwrap();
    assert_eq!(registry.schema_origin(&node).unwrap().id(), id);
    let (foreign, _) = Pump::new(crate::tests::support::widgets());
    assert!(foreign.registry().schema_origin(&node).is_none());
    let replaced = replacement_node();
    assert!(registry.schema_origin(&replaced).is_none());
    assert_eq!(
        registry.schema_origin(&node).unwrap().id(),
        id,
        "independently retained old code remains valid"
    );
    drop(node);
    assert!(
        registry
            .resources
            .borrow()
            .identities
            .values()
            .all(|identity| !identity.alive())
    );
    let replacement = code(
        vec![(
            String::new(),
            Shape {
                base: false,
                refinements: 1,
            },
            handle,
        )],
        &registry,
        &api,
    );
    assert_eq!(
        registry.resources.borrow().identities.len(),
        1,
        "next amortized prune removes retired identities"
    );
    drop(replacement);
}

#[test]
fn native_schema_json_transport_is_linear_for_selected_fields() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let (owner, runner, command) = growth_runner().await;
        let mut observed = Vec::new();
        for count in [16, 32, 64] {
            eprintln!("growth {count}: registration start");
            assert_eq!(
                command(
                    format!("schema growth:{count}"),
                    runner.create_command_context()
                )
                .await
                .unwrap_err()
                .to_string(),
                count.to_string()
            );
            eprintln!("growth {count}: registration done");
            let tool = owner.read().unwrap().tools["schema-growth"]
                .metadata()
                .definition;
            let call = maestro_models::ToolCall {
                id: "growth".into(),
                name: tool.name.clone(),
                arguments: json!({"marker":1,"large":"x".repeat(count*1024)}),
                thought_signature: None,
            };
            crate::schema_reads::BYTES.set((0, 0));
            eprintln!("growth {count}: validation start");
            let checked = maestro_models::validate_tool_arguments(&tool, &call).unwrap();
            assert_eq!(checked["touched"], true);
            assert_eq!(checked["large"], call.arguments["large"]);
            let bytes = crate::schema_reads::BYTES.get();
            assert_eq!(bytes, (count * 58, count * 12));
            observed.push(bytes);
        }
        println!("native selected JSON request/response bytes at 16/32/64: {observed:?}");
        assert_eq!(observed[2].0, 4 * observed[0].0);
    });
}

/// Load the real guest and retain its native registration owner for the byte control.
async fn growth_runner() -> (
    maestro_extensions::types::ExtensionHandle<Local>,
    maestro_extensions::ExtensionRunner<Local>,
    maestro_extensions::types::CommandHandler<Local>,
) {
    let native = maestro_extensions::create_extension_runtime::<Local>();
    let (owner, api) = super::api::api(native.clone());
    eprintln!("growth: factory start");
    crate::factory::artifact_factory(
        std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap(),
        crate::tests::support::widgets(),
        crate::Installation {
            version: "installed-component-host".into(),
            agent_dir: "/controlled-agent".into(),
        },
    )(api)
    .await
    .unwrap();
    eprintln!("growth: factory done");
    let runner = maestro_extensions::ExtensionRunner::<Local>::new(
        Rc::new(std::sync::RwLock::new(vec![owner.clone()])),
        native,
        super::session::runner_session("/schema").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;

    (owner, runner, command)
}

/// A newly supplied native callback must not resolve as an old guest capture.
fn replacement_node() -> SchemaNode {
    SchemaNode {
        base: None,
        refinements: vec![maestro_extensions::Refinement {
            check: maestro_extensions::SchemaCallback::new(|_| Ok(true)),
            error: maestro_extensions::SchemaCallback::new(|_| Ok(String::new())),
        }],
    }
}

#[test]
fn schema_check_during_a_routed_request_runs_through_that_route() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
            let registry = pump.registry();
            let (calls, incoming) = tokio::sync::mpsc::unbounded_channel();
            let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
            let id = callbacks.allocate().unwrap().id;
            let handle = callbacks.retain(id, admission.clone()).unwrap();
            drop(incoming);
            let checked = super::during_routed_request(
                &mut pump,
                move || {
                    let mut value = json!({"before": 7});
                    let accepted = handle.schema_call::<bool>(
                        &registry,
                        &events(),
                        crate::schema_wire::Operation::Check,
                        &mut value,
                    );
                    (accepted, value)
                },
                |call| {
                    let crate::guest_calls::Call::Schema(call) = call else {
                        panic!("missing nested schema check")
                    };
                    assert!(matches!(
                        call.operation,
                        crate::schema_wire::Operation::Check
                    ));
                    assert_eq!(
                        call.node
                            .read(|reply| crate::schema_reads::Request::Read(
                                "/before".into(),
                                reply
                            ))
                            .unwrap(),
                        Some("7".into())
                    );
                    call.node
                        .read(|reply| {
                            crate::schema_reads::Request::Set(
                                String::new(),
                                "seen".into(),
                                "true".into(),
                                reply,
                            )
                        })
                        .unwrap();
                    call.reply.send(Ok(Ok("true".into()))).unwrap();
                },
            )
            .await;
            assert!(checked.0.unwrap());
            assert_eq!(checked.1, json!({"before": 7, "seen": true}));
        });
}

#[test]
fn rejected_schema_admission_finishes_the_route_with_its_error() {
    assert_eq!(
        crate::import_route::Route::failed("schema description refused".into()).result(),
        Err("schema description refused".into())
    );
}

#[test]
fn routed_base_conversion_preserves_null_replacements_and_diagnostics() {
    use crate::schema_wire::{Code, Error};
    use maestro_extensions::{DiagnosticCode, DiagnosticErrorInfo};
    let diagnostic = DiagnosticErrorInfo {
        name: Some("SchemaFailure".into()),
        message: "authored failure".into(),
        stack: Some("authored stack".into()),
        code: Some(DiagnosticCode::Text("SCHEMA".into())),
    };
    let cases = [
        (Ok("null".to_owned()), Ok(None)),
        (Ok("[null]".to_owned()), Ok(Some(serde_json::Value::Null))),
        (
            Err(Error {
                name: diagnostic.name.clone(),
                message: diagnostic.message.clone(),
                stack: diagnostic.stack.clone(),
                code: Some(Code::Text("SCHEMA".into())),
            }),
            Err(diagnostic),
        ),
    ];
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            for (response, expected) in cases {
                let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
                let registry = pump.registry();
                let (calls, incoming) = tokio::sync::mpsc::unbounded_channel();
                let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
                let id = callbacks.allocate().unwrap().id;
                let handle = callbacks.retain(id, admission).unwrap();
                let (_owner, api) =
                    super::api::api(maestro_extensions::create_extension_runtime::<Local>());
                let mut schema = code(
                    vec![(
                        String::new(),
                        Shape {
                            base: true,
                            refinements: 0,
                        },
                        handle,
                    )],
                    &registry,
                    &api,
                );
                let base = schema.remove("").unwrap().base.unwrap();
                drop(incoming);
                let mut response = Some(response);
                let result = super::during_routed_request(
                    &mut pump,
                    move || base.convert.call(&mut json!({"original": true})),
                    move |call| answer_conversion(call, response.take().unwrap()),
                )
                .await;
                assert_eq!(result, expected);
            }
        });
}

/// Answers the native conversion on its actual nested route.
fn answer_conversion(call: super::NestedCall, response: Result<String, crate::schema_wire::Error>) {
    let crate::guest_calls::Call::Schema(call) = call else {
        panic!("missing nested conversion")
    };
    assert!(matches!(
        call.operation,
        crate::schema_wire::Operation::Convert
    ));
    call.reply.send(Ok(response)).unwrap();
}
