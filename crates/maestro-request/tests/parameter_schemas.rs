//! Shared parameter schemas and the ordinary JSON they carry.

use maestro_request::parameter_schema::{ParameterSchema, SchemaKind};
use maestro_request::tool_info::ToolInfo;
use maestro_request::types::{Context, Tool};
use serde_json::json;

/// A schema declaring a string member and a plain sibling member.
fn mixed() -> (serde_json::Value, [(String, SchemaKind); 2]) {
    (
        json!({
            "type": "object",
            "required": ["authored", "plain"],
            "properties": {
                "authored": {"type": "string"},
                "plain": {"type": "string"},
            },
        }),
        [
            (String::new(), SchemaKind::Object),
            ("/properties/authored".to_owned(), SchemaKind::String),
        ],
    )
}

#[test]
fn shared_tool_records_keep_one_schema_owner() {
    let (schema, kinds) = mixed();
    let tool = Tool {
        name: "echo".into(),
        description: "Echo".into(),
        parameters: ParameterSchema::authored(schema.clone(), kinds.clone()),
    };
    let context = Context {
        system_prompt: None,
        messages: Vec::new(),
        tools: Some(vec![tool]),
    };
    let info: ToolInfo = ToolInfo {
        name: "echo".into(),
        description: "Echo".into(),
        parameters: ParameterSchema::authored(schema, kinds),
        source_info: maestro_request::source_info::SourceInfo {
            path: "tools/echo.ts".into(),
            source: "supplied".into(),
            scope: maestro_request::source_info::SourceScope::User,
            origin: maestro_request::source_info::SourceOrigin::TopLevel,
            base_dir: None,
        },
    };
    let declared = context.tools.as_ref().and_then(|tools| tools.first());
    assert!(declared.is_some_and(|declared| declared.parameters.conversions().len() == 2));
    assert_eq!(
        info.parameters.as_json(),
        &declared.unwrap().parameters.as_json().clone()
    );
}

#[test]
fn mixed_schema_metadata_survives_owned_clones() {
    let (schema, kinds) = mixed();
    let authored: ParameterSchema = ParameterSchema::authored(schema.clone(), kinds.clone());
    assert_eq!(authored.as_json(), &schema);
    assert_eq!(authored.conversions().len(), kinds.len());
    assert_eq!(
        authored.conversions().get("/properties/authored"),
        Some(&SchemaKind::String)
    );
    assert!(
        !authored.conversions().contains_key("/properties/plain"),
        "an untagged member stays plain"
    );
    let clone = authored.clone();
    assert_eq!(clone.as_json(), authored.as_json());
    assert_eq!(clone.conversions(), authored.conversions());
    assert_eq!(
        authored.as_json(),
        &*authored,
        "deref observes the supplied JSON"
    );
}

#[test]
fn schema_serialization_is_plain_json() {
    let (schema, kinds) = mixed();
    let authored: ParameterSchema = ParameterSchema::authored(schema.clone(), kinds);
    let encoded = serde_json::to_value(&authored).expect("schema serialization");
    assert_eq!(
        encoded, schema,
        "serialization writes the supplied schema only"
    );

    let restored: ParameterSchema =
        serde_json::from_value(encoded).expect("schema deserialization");
    assert_eq!(restored.as_json(), &schema);
    assert!(
        restored.conversions().is_empty(),
        "reparsing the JSON loses the authored kinds"
    );

    let plain: ParameterSchema = ParameterSchema::from(schema.clone());
    assert_eq!(plain.conversions().len(), 0);
    assert_eq!(plain.into_json(), schema);
}

use maestro_request::diagnostics::{DiagnosticCode, DiagnosticErrorInfo};
use maestro_request::parameter_schema::{Base, SchemaCallback, SchemaCode, SchemaNode};
use serde_json::Value;
use std::{cell::Cell, rc::Rc};

#[test]
fn code_annotations_stay_out_of_serialized_parameters() {
    let calls = Rc::new(Cell::new(0));
    let capture = Rc::clone(&calls);
    let code: SchemaCode = std::collections::HashMap::from([(
        String::new(),
        SchemaNode {
            base: Some(Base {
                convert: SchemaCallback::new(move |value| {
                    capture.set(capture.get() + 1);
                    *value = json!({"edited": true});
                    Ok(Some(json!(42)))
                }),
                ..Base::default()
            }),
            refinements: Vec::new(),
        },
    )]);
    let schema = json!({"type": "object"});
    let parameters = ParameterSchema::<()>::from(schema.clone()).with_code(code);
    let tool = Tool {
        name: "local".into(),
        description: String::new(),
        parameters,
    };
    let encoded = serde_json::to_value(&tool).unwrap();
    assert_eq!(encoded["parameters"], schema);
    assert_eq!(calls.get(), 0);
    let restored: Tool<SchemaCode> = serde_json::from_value(encoded).unwrap();
    assert!(restored.parameters.code().is_empty());
    let data = tool.parameters.data();
    assert_eq!(data.as_json(), &schema);
    assert_eq!(data.conversions(), tool.parameters.conversions());
    let retained = tool.clone();
    drop(tool);
    let base = retained.parameters.code()[""].base.as_ref().unwrap();
    let mut value = json!({});
    assert_eq!(base.convert.call(&mut value).unwrap(), Some(json!(42)));
    assert_eq!(value, json!({"edited": true}));
    assert_eq!(calls.get(), 1);
}

#[test]
fn schema_callbacks_preserve_supplied_errors() {
    let error = DiagnosticErrorInfo {
        name: Some("AuthorError".into()),
        message: "conversion failed".into(),
        stack: Some("author frame".into()),
        code: Some(DiagnosticCode::Number(7.0)),
    };
    let original = error.clone();
    let callback = SchemaCallback::<bool>::new(move |_| Err(error.clone()));
    assert_eq!(callback.call(&mut Value::Null), Err(original));
}
