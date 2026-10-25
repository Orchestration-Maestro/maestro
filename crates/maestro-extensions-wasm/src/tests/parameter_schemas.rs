//! Guest registration and enumeration keep the schema origin the author declared.
use crate::tests::scenario::Driver;
use maestro_extensions_wasm::{ParameterSchema, SchemaKind, ToolInfo};
use serde_json::{Value, json};
use std::collections::HashMap;

/// The mixed schema the guest supplies: an authored member beside a plain one.
fn mixed_schema() -> Value {
    json!({"type":"object","properties":{
        "authored":{"type":"string"},
        "plain":{"type":"string"},
    }})
}

/// The kinds the author declared for that schema.
fn mixed_kinds() -> HashMap<String, SchemaKind> {
    HashMap::from([
        (String::new(), SchemaKind::Object),
        ("/properties/authored".to_owned(), SchemaKind::String),
    ])
}

/// Host configuration registering that schema through the authored entry point.
fn configuration() -> Value {
    json!({"metadata":{
        "name":"echo",
        "label":"Echo",
        "description":"Mixed schema",
        "parameters":mixed_schema(),
        "schemaKinds":mixed_kinds(),
    }})
}

/// One enumerated tool record as the host hands it over.
fn enumerated() -> Value {
    json!([{
        "name": "listed",
        "description": "Listed tool",
        "parameters": mixed_schema(),
        "sourceInfo": {"path":"/x","source":"custom","scope":"temporary","origin":"top-level"},
        "schemaKinds": mixed_kinds(),
    }])
}

/// Run one authored action and return its answer.
async fn ask(driver: &mut impl Driver, method: &str, input: Value) -> Result<Value, String> {
    let before = driver.transcript().len();
    driver
        .run_plain_command(
            "capture",
            &format!("action {}", json!({"method":method,"input":input})),
        )
        .await?;
    let lines = driver.transcript();
    let answer = lines[before..]
        .iter()
        .find_map(|line| line.strip_prefix("entry action-answer "))
        .ok_or("no action answer")?;
    serde_json::from_str(answer).map_err(|error| error.to_string())
}

/// The guest exports the request schema owner rather than a schema record of its own.
#[test]
fn guest_exports_the_request_schema_owner() {
    let authored = ParameterSchema::authored(mixed_schema(), mixed_kinds());
    let shared: &maestro_request::parameter_schema::ParameterSchema = &authored;
    assert_eq!(shared.as_json(), &mixed_schema());
    let listed: ToolInfo = serde_json::from_value(json!({
        "name": "listed",
        "description": "Listed tool",
        "parameters": mixed_schema(),
        "sourceInfo": {"path":"/x","source":"custom","scope":"temporary","origin":"top-level"},
    }))
    .expect("the guest tool record decodes");
    assert!(
        listed.parameters.conversions().is_empty(),
        "ordinary JSON carries no declared kinds"
    );
}

/// Registration hands the host the supplied schema with the kinds declared beside it.
async fn registration(driver: &mut impl Driver) -> Result<(), String> {
    let config = configuration();
    driver
        .run_plain_command("capture", &format!("tools {config}"))
        .await?;
    let recorded: Value =
        serde_json::from_str(driver.tools().last().unwrap()).map_err(|e| e.to_string())?;
    assert_eq!(
        recorded["parameters"],
        mixed_schema(),
        "the host observes the supplied schema, not a text encoding of it"
    );
    assert_eq!(
        recorded["schemaKinds"],
        json!(mixed_kinds()),
        "the internal transport carries the declared kinds"
    );
    let _ = driver.release("tool echo").await;
    let _ = driver.release("prepare echo").await;
    Ok(())
}

/// Enumeration restores the kinds the transport carried.
async fn enumeration(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command(
            "capture",
            &format!(
                "action {}",
                json!({"method":"control","input":{"value":enumerated()}})
            ),
        )
        .await?;
    let listed = ask(driver, "getAllTools", json!({"kinds":true})).await?;
    assert_eq!(
        listed["ok"][0],
        json!({"parameters":mixed_schema(),"kinds":mixed_kinds()}),
        "enumeration keeps both the supplied schema and its declared kinds"
    );
    let plain = ask(driver, "getAllTools", json!({})).await?;
    assert_eq!(
        plain["ok"][0]["parameters"],
        mixed_schema(),
        "ordinary serialization writes the supplied schema alone"
    );
    Ok(())
}

/// Registration and enumeration hand the schema across the boundary with its kinds.
async fn registration_and_enumeration(driver: &mut impl Driver) -> Result<(), String> {
    registration(driver).await?;
    enumeration(driver).await
}

#[test]
fn guest_registration_and_enumeration_keep_schema_origin() -> Result<(), String> {
    on_both_adapters!(registration_and_enumeration)
}

/// One recorded invocation paired with a declaration of the same name.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NestedInvocation {
    /// Declaration identity.
    id: String,
    /// Original invocation arguments.
    arguments_json: String,
    /// Prepared value, also returned as opaque tool details by the route witness.
    expected_json: String,
}

/// Read the matching invocation rows, rejecting missing, extra or reordered cases.
fn nested_invocations() -> Vec<NestedInvocation> {
    let declarations = super::author::nested_schema_fixture::cases().unwrap();
    let invocations: Vec<NestedInvocation> =
        serde_json::from_str(include_str!("nested_schema_invocations.json")).unwrap();
    for (declaration, invocation) in declarations.iter().zip(&invocations) {
        assert_eq!(declaration.id, invocation.id);
    }
    assert_eq!(declarations.len(), invocations.len());
    invocations
}

/// Register and relay one complete declaration through the actual author facade.
async fn relay_nested(driver: &mut impl Driver, case: &NestedInvocation) -> Result<(), String> {
    use super::author::nested_schema_fixture::{parameters, value};
    let details = &case.expected_json;
    let mut config = json!({"metadata":{
        "name":"nested", "label":"Nested", "description":"", "parameters":{}
    }, "prepare":false,"result":{"content":[],"details":details},"corpusCase":case.id});
    driver
        .run_plain_command("capture", &format!("tools {config}"))
        .await?;
    let first = driver.tools().last().unwrap().clone();
    let parsed = value(&first)?;
    let expected = parameters(&case.id)?;
    assert_eq!(&parsed["parameters"], expected.as_json());
    assert_eq!(parsed["schemaKinds"], json!(expected.conversions()));
    let mut listed = parsed.clone();
    listed["sourceInfo"] =
        json!({"path":"/author","source":"custom","scope":"temporary","origin":"top-level"});
    let listing = json!([listed]).to_string();
    let request = json!({"method":"control","input":{"raw":listing}});
    driver
        .run_plain_command("capture", &format!("action {request}"))
        .await?;
    config.as_object_mut().unwrap().shift_remove("corpusCase");
    config["metadata"]["name"] = json!("relayed");
    config["relay"] = json!(true);
    driver
        .run_plain_command("capture", &format!("tools {config}"))
        .await?;
    let second = driver.tools().last().unwrap().clone();
    let relayed = value(&second)?;
    assert_eq!(relayed["parameters"], parsed["parameters"]);
    assert_eq!(relayed["schemaKinds"], parsed["schemaKinds"]);
    let result = invoke_nested(driver, case).await?;
    println!(
        "ACCOUNTING {}",
        json!({"id":case.id,
        "wrapper": first.len() - parsed["parameters"].to_string().len() - parsed["schemaKinds"].to_string().len(),
        "schema":parsed["parameters"].to_string().len(), "kinds":parsed["schemaKinds"].to_string().len(),
        "registration":first.len(), "enumeration":listing.len(), "reregistration":second.len(),
        "invocation_input":case.arguments_json.len(), "invocation_output":result.len()})
    );
    assert!(driver.release("tool nested").await);
    assert!(driver.release("tool relayed").await);
    Ok(())
}

/// Execute the re-registered callback and observe only this invocation's transcript.
async fn invoke_nested(
    driver: &mut impl Driver,
    case: &NestedInvocation,
) -> Result<String, String> {
    use super::author::nested_schema_fixture::value;
    let before = driver.transcript().len();
    let result = driver
        .tool(super::scenario::ToolRun {
            name: "relayed",
            id: "nested-call",
            params: &case.arguments_json,
            signal: None,
            update: false,
            held: false,
        })
        .await?;
    assert_eq!(value(&result)?["details"], json!(case.expected_json));
    let transcript = driver.transcript();
    let execution = transcript[before..]
        .iter()
        .find_map(|line| line.strip_prefix("entry execution "))
        .unwrap();
    assert_eq!(value(execution)?["args"], value(&case.arguments_json)?);
    Ok(result)
}

/// Both adapters retain every complete declaration and release each callback owner.
async fn nested_relay(driver: &mut impl Driver) -> Result<(), String> {
    for case in nested_invocations() {
        relay_nested(driver, &case).await?;
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}

/// Nested declarations survive registration, enumeration, reuse and execution on both routes.
#[test]
fn nested_declarations_register_and_relay_on_both_adapters() -> Result<(), String> {
    on_both_adapters!(nested_relay)
}

/// Every Base method survives the generated retention and enumeration route.
#[test]
fn guest_base_methods_keep_results_edits_and_errors() {
    super::component_driver::schema_qualification::base_methods();
}

/// Stacked checks and their independent message factories retain every slot.
#[test]
fn guest_stacked_refinements_keep_each_check_and_error() {
    super::component_driver::schema_qualification::stacked_refinements();
}

/// Borrowed JSON operands preserve all value kinds and cleanup on callback failures.
#[test]
fn guest_json_operands_cover_values_members_and_failure_cleanup() {
    super::component_driver::schema_qualification::json_operands();
}
