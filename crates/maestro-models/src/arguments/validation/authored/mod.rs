//! Authored schema conversion applied before the unchanged plain pass.
//!
//! A schema node the author built carries its kind; a node present only as JSON does not.
//! Conversion visits the tagged nodes, so an untagged node keeps its supplied value and the
//! later plain pass observes it as ordinary JSON Schema.
mod schema_view;
mod types;
mod value;

use std::collections::HashMap;

use serde_json::Value;

use crate::SchemaKind;
use schema_view::{Converter, Node};

/// Convert one argument candidate through the kinds its schema author declared.
///
/// Only in-place effects are kept, as the reference keeps them: a replacement a converter
/// returns for the root is discarded, while container mutations reach the supplied value.
pub(super) fn convert(value: &mut Value, schema: &Value, kinds: &HashMap<String, SchemaKind>) {
    let mut converter = Converter::new(schema, kinds);
    let root = converter.root();
    let _replacement = value::convert::convert(&mut converter, root, value);
}

/// Whether the supplied node accepts the supplied value.
pub(super) fn accepts(node: &Node<'_>, value: &Value) -> bool {
    let schema = node.schema();
    super::check::check(&schema, value).is_ok_and(|errors| errors.is_empty())
}
