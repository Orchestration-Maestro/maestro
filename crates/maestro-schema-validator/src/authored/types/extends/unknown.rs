//! Extends check for a left node that accepts every JSON value.

use crate::authored::schema_view::Node;
use maestro_request::parameter_schema::SchemaKind;

/// A node accepting every value extends only a node that also accepts every value.
pub(crate) fn extends_unknown(right: &Node<'_>) -> bool {
    matches!(right.kind(), Some(SchemaKind::Any | SchemaKind::Unknown))
}
