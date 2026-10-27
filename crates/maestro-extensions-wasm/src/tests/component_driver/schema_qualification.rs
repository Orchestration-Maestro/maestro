//! Generated schema resources retain their methods, originating capability and lifetime.
use super::*;
use crate::bindings::host_side::maestro::extension::host::{SchemaOperation, SchemaShape};
use crate::tests::{block_on, host::Identity, schema_host::JsonNode};
use maestro_request::diagnostics::{DiagnosticCode, DiagnosticErrorInfo};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// Start one independently authored component.
fn start() -> ComponentDriver {
    let mut driver = ComponentDriver::new(crate::tests::path_of_component().unwrap());
    block_on(driver.start()).unwrap();
    driver
}
/// Register the source declaration, with a selected executable author program.
fn author(program: &str, failure: Option<DiagnosticErrorInfo>, phase: &str) -> ComponentDriver {
    let mut driver = start();
    let mut config = json!({"metadata":{"name":"original", "label":"Original", "description":"",
        "parameters":{"type":"object"}, "schemaKinds":{"":"Object"}},
        "schemaProgram":program, "schemaPhase":phase, "prepare":false, "result":{"content":[]}});
    if let Some(error) = failure {
        config["schemaFailure"] = json!(error);
    }
    register(&mut driver, &config);
    driver
}
/// Run a real author registration without replacing its wrappers.
fn register(driver: &mut ComponentDriver, config: &Value) {
    block_on(driver.run_plain_command("capture", &format!("tools {config}"))).unwrap();
}
/// Select each complete tool record for enumeration by the recipient.
fn listing(driver: &mut ComponentDriver, tools: Vec<Value>) {
    let tools: Vec<_> = tools.into_iter().map(|mut tool| {
        tool["sourceInfo"] = json!({"path":"/author", "source":"custom", "scope":"temporary", "origin":"top-level"});
        tool
    }).collect();
    driver.harness.as_mut().unwrap().observed().actions.value = json!(tools);
}
/// Enumerate only this call's answer, never an earlier success.
fn enumerate(driver: &mut ComponentDriver) -> Value {
    let before = driver.transcript().len();
    let request = json!({"method":"getAllTools", "input":{"code":true}});
    block_on(driver.run_plain_command("capture", &format!("action {request}"))).unwrap();
    let lines = driver.transcript();
    let answer = lines[before..]
        .iter()
        .find_map(|line| line.strip_prefix("entry action-answer "))
        .unwrap();
    serde_json::from_str(answer).unwrap()
}
/// Re-register a locally retained schema in its originating component.
fn relay(driver: &mut ComponentDriver) -> (u32, u32) {
    let original = schema_handle(driver, "original");
    let listed: Value = serde_json::from_str(driver.tools().last().unwrap()).unwrap();
    listing(driver, vec![listed]);
    register(
        driver,
        &json!({"metadata":{"name":"relayed", "label":"Relayed", "description":"", "parameters":{}},
        "relay":true, "prepare":false, "result":{"content":[]}}),
    );
    (original, schema_handle(driver, "relayed"))
}
/// Find one tool's resource, not the first tool in the inventory.
fn schema_handle(driver: &ComponentDriver, name: &str) -> u32 {
    driver
        .harness
        .as_ref()
        .unwrap()
        .store
        .data()
        .observed
        .schemas
        .iter()
        .find(|(tool, pointer, _)| tool == name && pointer.is_empty())
        .unwrap()
        .2
}
/// Read the layout from the originating handler, not from a reconstructed test declaration.
fn shape(driver: &mut ComponentDriver, handler: u32) -> SchemaShape {
    let harness = driver.harness.as_mut().unwrap();
    let exports = harness.exports.clone();
    let function = exports
        .func_describe_schema()
        .func()
        .typed::<(Resource<Identity>,), (
            Result<
                SchemaShape,
                crate::bindings::host_side::maestro::extension::host::DiagnosticErrorInfo,
            >,
        )>(&harness.store)
        .unwrap();
    block_on(harness.store.run_concurrent(async |accessor| {
        function.call_concurrent(accessor, (borrow(handler),)).await
    }))
    .unwrap()
    .unwrap()
    .0
    .unwrap()
}
/// Lend a real mutable native value, then restore it even when the guest or host fails.
fn callback(
    driver: &mut ComponentDriver,
    handler: u32,
    operation: SchemaOperation,
    value: &mut Value,
) -> Result<Value, DiagnosticErrorInfo> {
    let harness = driver.harness.as_mut().unwrap();
    let slot = Arc::new(Mutex::new(std::mem::take(value)));
    let node = harness
        .store
        .data_mut()
        .table
        .push(JsonNode(Arc::clone(&slot)))
        .unwrap();
    let emitter = harness
        .store
        .data_mut()
        .table
        .push(crate::tests::bus_host::Emitter { key: 37 })
        .unwrap();
    let exports = harness.exports.clone();
    let function = exports
        .func_invoke_schema()
        .func()
        .typed::<(
            Resource<Identity>,
            SchemaOperation,
            Resource<JsonNode>,
            Resource<crate::tests::bus_host::Emitter>,
        ), (
            Result<
                String,
                crate::bindings::host_side::maestro::extension::host::DiagnosticErrorInfo,
            >,
        )>(&harness.store)
        .unwrap();
    let result = block_on(harness.store.run_concurrent(async |accessor| {
        function
            .call_concurrent(
                accessor,
                (
                    borrow(handler),
                    operation,
                    Resource::new_borrow(node.rep()),
                    Resource::new_borrow(emitter.rep()),
                ),
            )
            .await
    }));
    *value = std::mem::take(&mut *slot.lock().unwrap());
    harness.store.data_mut().table.delete(node).unwrap();
    harness.store.data_mut().table.delete(emitter).unwrap();
    let (result,) = result
        .and_then(std::convert::identity)
        .map_err(|error| DiagnosticErrorInfo::error(format!("{error:#}")))?;
    let result = result.map_err(crate::schema_error::from_wire)?;
    crate::component_adapter::decode_json(&result).map_err(DiagnosticErrorInfo::error)
}
/// Release through the originating capability, independently of the active recipient.
fn release(driver: &mut ComponentDriver, handle: u32) {
    block_on(driver.repeat_release(handle)).unwrap();
}
/// Count destructor observations rather than inspecting reference counts.
fn released(driver: &ComponentDriver) -> usize {
    driver
        .transcript()
        .iter()
        .filter(|line| line.as_str() == "entry released \"schema\"")
        .count()
}

/// Base conversion, guard and diagnostic methods survive real retention and re-registration.
pub(crate) fn base_methods() {
    let mut driver = author("full", None, "");
    let (original, retained) = relay(&mut driver);
    release(&mut driver, original);
    assert!(shape(&mut driver, retained).base);
    let mut value = json!({});
    assert_eq!(
        callback(&mut driver, retained, SchemaOperation::Convert, &mut value).unwrap(),
        json!([{"replacement":true}])
    );
    assert_eq!(value, json!({"converted":true}));
    assert_eq!(
        callback(&mut driver, retained, SchemaOperation::Check, &mut value).unwrap(),
        json!(false)
    );
    assert_eq!(value, json!({"converted":true,"checked":true}));
    assert_eq!(
        callback(&mut driver, retained, SchemaOperation::Errors, &mut value).unwrap(),
        json!([{"keyword":"guard","message":"author detail"}])
    );
    assert_eq!(released(&driver), 0);
    release(&mut driver, retained);
    assert_eq!(released(&driver), 1);
}
/// Every refinement slot retains its own check and message in registration order.
pub(crate) fn stacked_refinements() {
    let mut driver = author("full", None, "");
    let (original, retained) = relay(&mut driver);
    assert_eq!(shape(&mut driver, retained).refinements, 2);
    let mut value = json!({});
    assert_eq!(
        callback(
            &mut driver,
            retained,
            SchemaOperation::Refine(0),
            &mut value
        )
        .unwrap(),
        json!(true)
    );
    assert_eq!(
        callback(
            &mut driver,
            retained,
            SchemaOperation::Refine(1),
            &mut value
        )
        .unwrap(),
        json!(false)
    );
    assert_eq!(
        value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["refine0", "refine1"]
    );
    for index in 0..2 {
        assert_eq!(
            callback(
                &mut driver,
                retained,
                SchemaOperation::Message(index),
                &mut value
            )
            .unwrap(),
            json!("same text")
        );
    }
    release(&mut driver, retained);
    assert_eq!(released(&driver), 0);
    assert_eq!(
        callback(
            &mut driver,
            original,
            SchemaOperation::Refine(1),
            &mut value
        )
        .unwrap(),
        json!(false)
    );
    release(&mut driver, original);
    assert_eq!(released(&driver), 1);
}
/// Full JSON access preserves kinds, order, edits and the attached child on an error path.
pub(crate) fn json_operands() {
    let mut driver = author("operand", None, "");
    let (original, retained) = relay(&mut driver);
    release(&mut driver, original);
    let mut value = json!({"null":null,"text":"text","number":3,"truth":true,"zero":-0.0,
        "object":{"10":10,"2":2,"02":20,"first":1,"second":2},"array":[1,2,3],"child":{}});
    assert_eq!(
        callback(
            &mut driver,
            retained,
            SchemaOperation::Refine(0),
            &mut value
        )
        .unwrap(),
        json!(true)
    );
    assert_eq!(
        value,
        json!({"null":null,"text":"text","number":3,"truth":true,"zero":-0.0,
        "object":{"10":10,"2":2,"02":20,"second":2,"third":3},"array":[false,"inserted",null,3],"child":{"kept":true}})
    );
    assert_eq!(
        maestro_json::canonical_entries(value["object"].as_object().unwrap())
            .into_iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        ["2", "10", "02", "second", "third"]
    );
    assert!(value["zero"].as_f64().unwrap().is_sign_negative());
    release(&mut driver, retained);
}

/// Every diagnostic field survives an owning guest's local schema alias.
#[test]
fn schema_callbacks_preserve_supplied_errors_on_owning_guest() {
    for code in [
        DiagnosticCode::Text("author-code".into()),
        DiagnosticCode::Number(7.0),
    ] {
        let phases = [
            ("convert", SchemaOperation::Convert),
            ("check", SchemaOperation::Check),
            ("errors", SchemaOperation::Errors),
            ("refine1", SchemaOperation::Refine(1)),
            ("message1", SchemaOperation::Message(1)),
        ];
        for (phase, operation) in phases {
            let expected = DiagnosticErrorInfo {
                name: Some("AuthorError".into()),
                message: "author failure".into(),
                stack: Some("author frame".into()),
                code: Some(code.clone()),
            };
            let mut driver = author("full", Some(expected.clone()), phase);
            let (original, retained) = relay(&mut driver);
            release(&mut driver, original);
            let mut value = json!({});
            assert_eq!(
                callback(&mut driver, retained, operation, &mut value),
                Err(expected)
            );
            release(&mut driver, retained);
            assert_eq!(released(&driver), 1);
        }
    }
}
/// Each tool gets only its own guest's callbacks; foreign records remain data-only.
#[test]
fn guest_schema_enumeration_keeps_foreign_entries_data_only() {
    let mut driver = author("full", None, "");
    let listed: Value = serde_json::from_str(driver.tools().last().unwrap()).unwrap();
    listing(
        &mut driver,
        vec![
            json!({"name":"plain","description":"Plain","parameters":{"type":"string"}}),
            listed,
        ],
    );
    assert_eq!(
        enumerate(&mut driver),
        json!({"ok":[
            {"name":"plain","parameters":{"type":"string"},"codeNodes":0},
            {"name":"original","parameters":{"type":"object"},"codeNodes":1},
        ]})
    );
    let path = crate::tests::path_of_component().unwrap();
    let host = driver.harness.as_mut().unwrap();
    host.exports = block_on(host.add_instance(&path)).unwrap();
    let result = enumerate(&mut driver);
    assert_eq!(
        result,
        json!({"ok":[
            {"name":"plain","parameters":{"type":"string"},"codeNodes":0},
            {"name":"original","parameters":{"type":"object"},"codeNodes":0},
        ]})
    );
}

/// The owning schema callback can emit to a listener synchronously before its later edit.
#[test]
fn owning_schema_callback_emits_to_local_listener() {
    let mut driver = author("emitting", None, "");
    let (original, retained) = relay(&mut driver);
    release(&mut driver, original);
    let mut value = json!({});
    assert_eq!(
        callback(
            &mut driver,
            retained,
            SchemaOperation::Refine(0),
            &mut value
        )
        .unwrap(),
        json!(true)
    );
    assert_eq!(value, json!({"before":true,"after":true}));
    assert!(
        driver
            .transcript()
            .iter()
            .any(|line| line == "entry schema-listener {\"before\":true}")
    );
    release(&mut driver, retained);
}

/// A rejected metadata record cannot keep an independently retained schema identity alive.
#[test]
fn local_schema_enumeration_releases_aliases_when_metadata_is_rejected() {
    let mut driver = author("full", None, "");
    let original = schema_handle(&driver, "original");
    let mut listed: Value = serde_json::from_str(driver.tools().last().unwrap()).unwrap();
    listed["description"] = json!(false);
    listing(&mut driver, vec![listed]);
    let result = enumerate(&mut driver);
    assert!(result.get("error").is_some());
    assert!(result.get("ok").is_none());
    release(&mut driver, original);
    assert_eq!(
        released(&driver),
        1,
        "rejected metadata releases its schema alias"
    );
}

/// Default Base methods admit every JSON kind without replacement, edits or error records.
#[test]
fn guest_base_defaults_keep_every_json_kind() {
    let mut driver = author("defaults", None, "");
    let (original, retained) = relay(&mut driver);
    release(&mut driver, original);
    for input in [
        json!(null),
        json!(true),
        json!(false),
        json!(0),
        json!(1),
        json!(-2),
        json!(1.5),
        json!(""),
        json!("4"),
        json!("bad"),
        json!([]),
        json!({}),
    ] {
        let mut value = input.clone();
        assert_eq!(
            callback(&mut driver, retained, SchemaOperation::Convert, &mut value).unwrap(),
            json!(null)
        );
        assert_eq!(
            callback(&mut driver, retained, SchemaOperation::Check, &mut value).unwrap(),
            json!(true)
        );
        assert_eq!(
            callback(&mut driver, retained, SchemaOperation::Errors, &mut value).unwrap(),
            json!([])
        );
        assert_eq!(value, input);
    }
    release(&mut driver, retained);
}

/// An invocation owns the driver only while calling the actual generated resource.
type SchemaDriver = std::rc::Rc<std::cell::RefCell<Option<ComponentDriver>>>;
/// Return a driver before caller code can reenter the native checker.
fn with_driver<T>(owner: &SchemaDriver, run: impl FnOnce(&mut ComponentDriver) -> T) -> T {
    let mut driver = owner.borrow_mut().take().unwrap();
    let result = run(&mut driver);
    *owner.borrow_mut() = Some(driver);
    result
}
/// Recorded setup and invocation controls, with no ignored fixture fields.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckerCases {
    /// Nested resource growth observations.
    growth: Vec<GrowthCase>,
    /// A later branch reads the earlier edit.
    mutation: CheckerCase,
    /// A collection rule resumes after an edit.
    #[serde(rename = "phaseMutation")]
    phase_mutation: CheckerCase,
    /// First, later and minimum-zero contains branches.
    contains: Vec<CheckerCase>,
}
/// One setup size and its complete ordered output.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GrowthCase {
    /// Number of nested schema nodes.
    n: usize,
    /// Authored schema protocol text.
    schema: String,
    /// Original invocation protocol text.
    input: String,
    /// Complete ordered result protocol text.
    result: String,
}
/// One complete callback result recorded from the source.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckerCase {
    /// Authored schema protocol text.
    schema: String,
    /// Original invocation protocol text.
    input: String,
    /// Complete returned value.
    result: Value,
}
/// Decode the shared recorder without accepting unused fixture fields.
fn checker_cases() -> CheckerCases {
    serde_json::from_str(include_str!("schema_checker_cases.json")).unwrap()
}
/// Invoke one generated schema method and decode its actual result.
fn native_callback<R: serde::de::DeserializeOwned + 'static>(
    owner: &SchemaDriver,
    handler: u32,
    operation: SchemaOperation,
) -> maestro_models::SchemaCallback<R> {
    let owner = std::rc::Rc::clone(owner);
    maestro_models::SchemaCallback::new(move |value| {
        let answer = with_driver(&owner, |driver| {
            driver
                .harness
                .as_mut()
                .unwrap()
                .observed()
                .schema_request_bytes += 13;
            let answer = callback(driver, handler, operation, value);
            if let Ok(value) = &answer {
                driver
                    .harness
                    .as_mut()
                    .unwrap()
                    .observed()
                    .schema_response_bytes += 9 + value.to_string().len();
            }
            answer
        })?;
        serde_json::from_value(answer)
            .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))
    })
}
/// Project the retained resource's complete layout, never a replacement checker.
fn native_node(owner: &SchemaDriver, handler: u32) -> maestro_models::SchemaNode {
    let shape = with_driver(owner, |driver| shape(driver, handler));
    maestro_models::SchemaNode {
        base: shape.base.then(|| maestro_models::Base {
            convert: {
                let convert =
                    native_callback::<Option<(Value,)>>(owner, handler, SchemaOperation::Convert);
                maestro_models::SchemaCallback::new(move |value| {
                    Ok(convert.call(value)?.map(|(value,)| value))
                })
            },
            check: native_callback(owner, handler, SchemaOperation::Check),
            errors: native_callback(owner, handler, SchemaOperation::Errors),
        }),
        refinements: (0..shape.refinements)
            .map(|index| maestro_models::Refinement {
                check: native_callback(owner, handler, SchemaOperation::Refine(index)),
                error: native_callback(owner, handler, SchemaOperation::Message(index)),
            })
            .collect(),
    }
}
/// Register, enumerate and re-register a real guest declaration, then release the originals.
fn checker_driver(schema: &Value, kinds: &Value, program: Option<&str>) -> SchemaDriver {
    let mut driver = start();
    register(
        &mut driver,
        &json!({"metadata":{"name":"original","label":"Original","description":"","parameters":schema,"schemaKinds":kinds},"schemaCode":true,"schemaProgram":program,"schemaPointer":"/properties/n","prepare":false,"result":{"content":[]}}),
    );
    let handles = driver
        .harness
        .as_ref()
        .unwrap()
        .store
        .data()
        .observed
        .schemas
        .clone();
    let listed =
        crate::tests::author::nested_schema_fixture::value(driver.tools().last().unwrap()).unwrap();
    listing(&mut driver, vec![listed]);
    register(
        &mut driver,
        &json!({"metadata":{"name":"relayed","label":"Relayed","description":"","parameters":{}},"relay":true,"prepare":false,"result":{"content":[]}}),
    );
    for (_, _, handle) in handles {
        release(&mut driver, handle);
    }
    assert!(block_on(driver.release("tool original")));
    std::rc::Rc::new(std::cell::RefCell::new(Some(driver)))
}
/// Consume only the enumerated and re-registered declaration in the one native validator.
fn checker_validate(owner: &SchemaDriver, args: &Value) -> Value {
    let (metadata, handles) = with_driver(owner, |driver| {
        (
            crate::tests::author::nested_schema_fixture::value(driver.tools().last().unwrap())
                .unwrap(),
            driver
                .harness
                .as_ref()
                .unwrap()
                .store
                .data()
                .observed
                .schemas
                .clone(),
        )
    });
    let code: maestro_models::SchemaCode = handles
        .into_iter()
        .filter(|(tool, _, _)| tool == "relayed")
        .map(|(_, pointer, handler)| (pointer, native_node(owner, handler)))
        .collect();
    let kinds: std::collections::HashMap<String, maestro_models::SchemaKind> =
        serde_json::from_value(metadata["schemaKinds"].clone()).unwrap();
    let tool = maestro_models::Tool {
        name: "relayed".into(),
        description: String::new(),
        parameters: maestro_models::ParameterSchema::<()>::authored(
            metadata["parameters"].clone(),
            kinds,
        )
        .with_code(code),
    };
    maestro_models::validate_tool_arguments(
        &tool,
        &maestro_models::ToolCall {
            id: "qualified".into(),
            name: "relayed".into(),
            arguments: args.clone(),
            thought_signature: None,
        },
    )
    .unwrap()
}
/// Execution must consume exactly the native checker's value, not the original arguments.
fn checker_execute(owner: &SchemaDriver, value: &Value) {
    with_driver(owner, |driver| {
        let before = driver.transcript().len();
        block_on(driver.tool(crate::tests::scenario::ToolRun {
            name: "relayed",
            id: "qualified",
            params: &value.to_string(),
            signal: None,
            update: false,
            held: false,
        }))
        .unwrap();
        let lines = driver.transcript();
        let execution = lines[before..]
            .iter()
            .find_map(|line| line.strip_prefix("entry execution "))
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(execution).unwrap()["args"],
            *value
        );
    });
}
/// Read source-recorded protocol values with the same admitted nesting as the author.
fn recorded_value(value: &str) -> Value {
    crate::tests::author::nested_schema_fixture::value(value).unwrap()
}
#[test]
fn schema_actual_route_accounts_for_setup_payload_and_invocations() {
    let cases = checker_cases();
    let mut costs = Vec::new();
    for row in &cases.growth {
        let n = row.n;
        let schema = recorded_value(&row.schema);
        let args = recorded_value(&row.input);
        let kinds: Value = (0..n)
            .map(|index| ("/properties/next".repeat(index), json!("Object")))
            .collect::<serde_json::Map<_, _>>()
            .into();
        let driver = checker_driver(&schema, &kinds, None);
        let first = checker_validate(&driver, &args);
        assert_eq!(first.to_string(), row.result);
        let second = checker_validate(&driver, &args);
        assert_eq!(second.to_string(), row.result);
        assert_eq!(args, recorded_value(&row.input));
        checker_execute(&driver, &second);
        with_driver(&driver, |driver| {
            let registrations = driver.tools();
            let observed = &driver.harness.as_ref().unwrap().store.data().observed;
            assert_eq!(registrations.len(), 2);
            assert_eq!(observed.schema_checks, 2 * n);
            println!(
                "SCHEMA-COST nodes={n} registration={} reregistration={} kinds={} requests={} responses={}",
                registrations[0].len(),
                registrations[1].len(),
                kinds.to_string().len(),
                observed.schema_request_bytes,
                observed.schema_response_bytes
            );
            costs.push((
                observed.schema_request_bytes,
                observed.schema_response_bytes,
            ));
        });
    }
    for pair in costs.windows(2) {
        assert!(pair[1].0 <= 2 * pair[0].0);
        assert!(pair[1].1 <= 2 * pair[0].1);
    }
}
/// Compare one complete live-edit result through validation and execution.
fn checker_case(row: &CheckerCase, kinds: &Value) {
    let schema = recorded_value(&row.schema);
    let args = recorded_value(&row.input);
    let driver = checker_driver(&schema, kinds, None);
    let result = checker_validate(&driver, &args);
    assert_eq!(result, row.result);
    assert_eq!(args, recorded_value(&row.input));
    checker_execute(&driver, &result);
}
#[test]
fn schema_actual_route_reads_edits_before_a_later_branch() {
    checker_case(&checker_cases().mutation, &json!({"/allOf/0":"Object"}));
}
#[test]
fn schema_actual_route_resumes_collection_rules_after_a_guest_edit() {
    checker_case(
        &checker_cases().phase_mutation,
        &json!({"/dependentSchemas/trigger":"Object"}),
    );
}
#[test]
fn schema_actual_route_stops_contains_after_the_first_accepted_member() {
    checker_case(
        &checker_cases().contains[0],
        &json!({"":"Array","/contains":"Object"}),
    );
}
#[test]
fn schema_actual_route_contains_keeps_later_match_and_disabled_controls() {
    for row in &checker_cases().contains[1..] {
        checker_case(row, &json!({"":"Array","/contains":"Object"}));
    }
}

/// The generated route can read, write and return values beyond default decoder depth.
fn deep_operand(program: &str) {
    let mut driver = author(program, None, "");
    let handler = schema_handle(&driver, "original");
    let mut nested = json!("leaf");
    for _ in 0..160 {
        nested = json!([nested]);
    }
    let mut value = json!({"nested":nested,"array":[nested]});
    assert_eq!(
        callback(&mut driver, handler, SchemaOperation::Refine(0), &mut value).unwrap(),
        json!(true)
    );
    assert_eq!(value["written"], nested);
    assert_eq!(value["array"], json!([nested]));
    release(&mut driver, handler);
}

#[test]
fn guest_json_read_keeps_admitted_nesting() {
    deep_operand("deep-read");
}
#[test]
fn guest_json_splice_keeps_admitted_nesting() {
    deep_operand("deep-splice");
}

/// Generated conversion results retain presence until the native member assignment.
#[test]
fn guest_conversion_distinguishes_null_replacement_from_no_replacement() {
    let schema = json!({"type":"object","properties":{"n":{}},"required":["n"]});
    let driver = checker_driver(
        &schema,
        &json!({"":"Object","/properties/n":"Base"}),
        Some("null-conversion"),
    );
    for (input, expected) in [
        (json!({"n":7}), json!({"n":7})),
        (json!({"n":""}), json!({"n":null})),
    ] {
        let result = checker_validate(&driver, &input);
        assert_eq!(result, expected);
        checker_execute(&driver, &result);
    }
}
