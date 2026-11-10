//! Executable node lookup and source-ordered guard and refinement phases.

use std::collections::HashMap;

use crate::{DiagnosticErrorInfo, SchemaCode, SchemaNode};
use serde_json::Value;

use super::super::{authored, scalars};
use super::{Evaluation, Failure, operand::Operand};

/// Callback annotations resolved once against the original schema document.
#[derive(Default)]
pub(in super::super) struct Code<'a> {
    /// Original schema identity to its retained author callbacks.
    nodes: HashMap<*const Value, &'a SchemaNode>,
}

impl<'a> Code<'a> {
    /// Resolve authored pointers before checking any nested operand.
    pub(in super::super) fn new(schema: &'a Value, code: Option<&'a SchemaCode>) -> Self {
        Self {
            nodes: code.map_or_else(HashMap::new, |code| authored::index(schema, code)),
        }
    }

    /// Read callbacks for this original schema, including resolved reference targets.
    pub(in super::super) fn node(&self, schema: &Value) -> Option<&'a SchemaNode> {
        if schema.is_object() {
            self.nodes.get(&std::ptr::from_ref(schema)).copied()
        } else {
            None
        }
    }
}

/// Apply the guard independently of conversion-kind tagging.
pub(super) fn guard(
    node: Option<&SchemaNode>,
    operand: &mut Operand<'_>,
    path: &str,
    exhaustive: bool,
    result: &mut Evaluation,
) -> Result<(), DiagnosticErrorInfo> {
    if let Some(base) = node.and_then(|node| node.base.as_ref())
        && let Operand::Write(value) = operand
        && !base.check.call(value)?
    {
        if exhaustive {
            // Guard details are author effects even though the localized text is fixed.
            base.errors.call(value)?;
        }
        result.add_errors([scalars::render(path, "must match check function")]);
    }
    Ok(())
}

/// Apply refinements only after the containing schema's ordinary assertions succeeded.
pub(super) fn refine(
    node: Option<&SchemaNode>,
    operand: &mut Operand<'_>,
    path: &str,
    exhaustive: bool,
    result: &mut Evaluation,
) -> Result<(), DiagnosticErrorInfo> {
    let (Some(node), Operand::Write(value)) = (node, operand) else {
        return Ok(());
    };
    for refinement in &node.refinements {
        if !refinement.check.call(value)? {
            let message = if exhaustive {
                refinement.error.call(value)?
            } else {
                String::new()
            };
            result.add_errors([Failure::Refinement(scalars::render(path, &message))]);
            if !exhaustive {
                break;
            }
        }
    }
    Ok(())
}
