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
