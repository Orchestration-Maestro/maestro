//! Differential schema keyword controls through the owning continuation.

use super::{frame::evaluate, operand::Operand};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

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
        let mut counts = HashMap::<String, usize>::new();
        let (working, errors) = evaluate(
            &case.schema,
            &[],
            Operand::Write(case.input),
            &mut |node, value| effect(node, value, &mut counts),
        )
        .unwrap();
        let working = working.value();
        assert_eq!(
            maestro_json::compact_json(working).unwrap(),
            maestro_json::compact_json(&case.working).unwrap(),
            "{} working operand",
            case.id,
        );
        assert_eq!(errors, case.errors, "{} corrective messages", case.id);
    }
}

/// Apply the recorded source callback below the checker's own phases.
fn effect(
    node: &Value,
    value: &mut Value,
    counts: &mut HashMap<String, usize>,
) -> Result<(), String> {
    let Some(effect) = node.get("x-effect") else {
        return Ok(());
    };
    let label = effect["label"].as_str().unwrap();
    let count = counts.entry(label.into()).or_default();
    *count += 1;
    if let Some(object) = value.as_object_mut() {
        for (key, value) in effect["put"].as_object().unwrap() {
            let value = if effect["toggle"] == true {
                serde_json::json!(*count)
            } else {
                value.clone()
            };
            object.insert(key.clone(), value);
        }
        for key in effect["remove"].as_array().unwrap() {
            object.shift_remove(key.as_str().unwrap());
        }
    }
    if effect["reject"] == true {
        Err(format!("reject {label}"))
    } else {
        Ok(())
    }
}
