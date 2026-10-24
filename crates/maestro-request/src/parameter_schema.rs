//! Parameter schemas that remember which nodes an author declared.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::ops::Deref;

/// Declared schema category of one authored node.
///
/// The variants name the schema kinds the authoring builders produce. A serialized schema
/// carries no kind information: it is ordinary JSON Schema, and ordinary JSON Schema is
/// validated without authored conversion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SchemaKind {
    /// Accepts every value.
    Any,
    /// Accepts no value.
    Never,
    /// Accepts every JSON value.
    Unknown,
    /// Text.
    String,
    /// A number, fractional included.
    Number,
    /// A number without a fractional part.
    Integer,
    /// A truth value.
    Boolean,
    /// The JSON null value.
    Null,
    /// A sequence of one declared item kind.
    Array,
    /// A map of declared named members.
    Object,
    /// A sequence whose positions have their own declared kinds.
    Tuple,
    /// Alternative members, selected in declaration order.
    Union,
    /// A map whose keys all use one declared kind.
    Record,
    /// A single constant value.
    Literal,
    /// The constants named by `enum`.
    Enum,
    /// The members of `allOf`, intersected.
    Intersect,
    /// A reference to a named definition.
    Ref,
    /// A definition that may refer to itself.
    Cyclic,
    /// Text constrained by a template pattern.
    TemplateLiteral,
}

/// An owned parameter schema with the kinds its author declared for individual nodes.
///
/// The record is the single owner of tool parameter schemas. It keeps the ordinary JSON
/// Schema value at the root, so providers and persistence observe exactly the supplied
/// schema, and keeps the authored kinds beside it, keyed by JSON pointer. A node without an
/// entry is plain: it stays whatever the supplied JSON says and is never converted as an
/// authored node. Equality compares the authored kinds as well as the supplied JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterSchema {
    /// Supplied JSON Schema value.
    schema: Value,
    /// Authored kinds by JSON pointer, where the empty pointer names the root.
    conversions: HashMap<String, SchemaKind>,
}

impl ParameterSchema {
    /// Retain a supplied schema and the kinds its author declared for its nodes.
    ///
    /// Each entry pairs a JSON pointer with that node's kind. The empty pointer names the
    /// supplied schema itself. `SchemaKind` and the pointer need not agree with the supplied
    /// JSON: the kind is what the caller built, and the JSON is what it serialized to.
    pub fn authored(
        schema: Value,
        conversions: impl IntoIterator<Item = (String, SchemaKind)>,
    ) -> Self {
        Self {
            schema,
            conversions: conversions.into_iter().collect(),
        }
    }

    /// Borrow the ordinary JSON Schema value, without authored provenance.
    #[must_use]
    pub fn as_json(&self) -> &Value {
        &self.schema
    }

    /// Consume the record into its ordinary JSON Schema value.
    ///
    /// The authored kinds belong to the node kinds the caller built, not to the schema text,
    /// so they end here. Rebuilding author intent needs the builder that declared it.
    #[must_use]
    pub fn into_json(self) -> Value {
        self.schema
    }

    /// Borrow the authored kinds by JSON pointer.
    ///
    /// Argument preparation and the internal registration transport read this. Provider
    /// payloads and persisted JSON never do: they carry the plain JSON Schema value.
    #[must_use]
    pub fn conversions(&self) -> &HashMap<String, SchemaKind> {
        &self.conversions
    }
}

impl From<Value> for ParameterSchema {
    fn from(schema: Value) -> Self {
        Self {
            schema,
            conversions: HashMap::new(),
        }
    }
}

impl Deref for ParameterSchema {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.schema
    }
}

impl Serialize for ParameterSchema {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.schema.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ParameterSchema {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Value::deserialize(deserializer).map(Self::from)
    }
}
