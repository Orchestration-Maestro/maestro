//! Parameter schemas that remember which nodes an author declared.
pub mod transport;
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
    /// A node supplying its own conversion and guard.
    Base,
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
    /// A map whose member values take the kind declared for each name pattern they match.
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
pub struct ParameterSchema<C = ()> {
    /// Executable code retained separately from ordinary schema data.
    code: C,
    /// Supplied JSON Schema value.
    schema: Value,
    /// Authored kinds by JSON pointer, where the empty pointer names the root.
    conversions: HashMap<String, SchemaKind>,
}

impl<C: Default> ParameterSchema<C> {
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
            code: C::default(),
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

impl<C: Default> From<Value> for ParameterSchema<C> {
    fn from(schema: Value) -> Self {
        Self {
            schema,
            code: C::default(),
            conversions: HashMap::new(),
        }
    }
}

impl<C> Deref for ParameterSchema<C> {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.schema
    }
}

impl<C> Serialize for ParameterSchema<C> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.schema.serialize(serializer)
    }
}

impl<'de, C: Default> Deserialize<'de> for ParameterSchema<C> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Value::deserialize(deserializer).map(Self::from)
    }
}

/// Executable schema callback records.
pub mod code;
pub use code::{Base, Refinement, SchemaCallback, SchemaCode, SchemaCodeSource, SchemaNode};
impl<C> ParameterSchema<C> {
    /// Borrow the code payload.
    pub fn code(&self) -> &C {
        &self.code
    }
    /// Replace the code payload without changing the supplied schema or declared kinds.
    pub fn with_code<D>(self, code: D) -> ParameterSchema<D> {
        ParameterSchema {
            schema: self.schema,
            conversions: self.conversions,
            code,
        }
    }
    /// Copy the data-only projection for a separately owned request.
    pub fn data(&self) -> ParameterSchema {
        ParameterSchema {
            schema: self.schema.clone(),
            conversions: self.conversions.clone(),
            code: (),
        }
    }
}
