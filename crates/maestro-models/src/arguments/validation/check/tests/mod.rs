//! Differential schema keyword controls through the owning continuation.

use super::{frame::evaluate, operand::Operand};
use serde::Deserialize;
use serde_json::Value;
/// Shared author callback construction for unit and public application observations.
mod schema_effects {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/schema_effects.rs"
    ));
}

/// One recorded checker invocation and its independently observed result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Stable label for a source input.
    id: String,
    /// Schema including the controlled refinement declarations.
    schema: Value,
    /// Initial argument value.
    input: Value,
    /// Complete value after checking and diagnostics.
    working: Value,
    /// Corrective messages in source order.
    errors: Vec<String>,
}

#[test]
fn schema_rule_phases_keep_live_operands() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("schema_rule_phases.json")).unwrap();
    assert_eq!(cases.len(), 122);
    for case in cases {
        let callbacks = schema_effects::code(&case.schema);
        let code = super::Code::new(&case.schema, Some(&callbacks));
        let (working, errors) =
            evaluate(&case.schema, &[], Operand::Write(case.input), &code).unwrap();
        let working = working.value().unwrap();
        assert_eq!(
            maestro_json::compact_json(working).unwrap(),
            maestro_json::compact_json(&case.working).unwrap(),
            "{} working operand",
            case.id,
        );
        assert_eq!(errors, case.errors, "{} corrective messages", case.id);
    }
}

#[test]
fn compiled_required_members_are_checked_after_dependency_deletion() {
    use serde_json::json;
    for (member, accepted) in [
        (json!({"type":"string"}), false),
        (json!({"type":"null"}), false),
        (json!(true), true),
        (json!({}), true),
        (json!({"not":{"type":"null"}}), true),
        (json!({"const":null}), false),
        (json!({"enum":[null]}), false),
        (json!({"type":"undefined"}), true),
    ] {
        let schema = json!({"type":"object", "properties":{"n":member}, "required":["n"],
            "dependentSchemas":{"go":{"x-effect":{"label":"delete","put":{},"remove":["n"]}}}});
        let callbacks = schema_effects::code(&schema);
        let code = super::Code::new(&schema, Some(&callbacks));
        let mut value = json!({"go":true,"n":"x"});
        assert_eq!(
            super::frame::candidate(&schema, &mut value, &code).unwrap(),
            accepted,
            "{member}"
        );
        assert_eq!(value, json!({"go":true}));
        let mut interpreted = json!({"go":true,"n":"x"});
        assert!(super::frame::authored_candidate(&schema, &[], &mut interpreted, &code).unwrap());
        let mut optional = schema.clone();
        optional.as_object_mut().unwrap().shift_remove("required");
        let code = super::Code::new(&optional, Some(&callbacks));
        assert!(
            super::frame::candidate(&optional, &mut json!({"go":true,"n":"x"}), &code).unwrap()
        );
    }
}
