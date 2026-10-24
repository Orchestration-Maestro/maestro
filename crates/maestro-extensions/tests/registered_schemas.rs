//! Schema origin retained through native registration and wrapping.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::local_support as support;
use crate::tool_support;
use maestro_models::{ParameterSchema, SchemaKind};

/// A schema declaring one authored and one plain member.
fn mixed() -> ParameterSchema {
    ParameterSchema::authored(
        serde_json::json!({
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
fn registered_schema_origin_survives_native_retention() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("owner", bus.controller.for_extension());
    let mut wrapped_source = tool_support::definition("echo", "Echo");
    wrapped_source.definition.parameters = mixed();
    let wrapped = maestro_tools::local::wrap_tool_definition(wrapped_source, None);
    let mut definition = tool_support::definition("echo", "Echo");
    definition.definition.parameters = mixed();
    loaded.api.register_tool(definition).expect("registration");

    let retained = {
        let owner = loaded.owner.read().expect("owner");
        let retained = owner.tools["echo"]
            .definition
            .metadata()
            .definition
            .parameters;
        assert_eq!(
            retained.as_json()["properties"]["authored"]["type"],
            "string"
        );
        assert_eq!(retained.conversions().len(), 2, "both kinds retained");
        assert_eq!(
            retained.conversions().get("/properties/authored"),
            Some(&SchemaKind::String)
        );
        assert!(
            !retained.conversions().contains_key("/properties/plain"),
            "the untagged member stays plain"
        );
        retained
    };

    let declared = wrapped.definition.parameters;
    assert_eq!(declared.as_json(), retained.as_json(), "supplied schema");
    assert_eq!(declared.conversions(), retained.conversions(), "node kinds");
}
