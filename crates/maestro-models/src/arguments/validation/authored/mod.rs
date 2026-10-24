//! Authored schema conversion applied before the unchanged plain pass.
//!
//! A schema node the author built carries its kind; a node present only as JSON does not.
//! Conversion visits the tagged nodes, so an untagged node keeps its supplied value and the
//! later plain pass observes it as ordinary JSON Schema.
mod collections;
mod composites;
mod evaluate;
mod scalars;
mod templates;

use crate::SchemaKind;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;

/// The keys a schema declares in the named map, as one indexed lookup set.
fn keys(schema: &Value, member: &str) -> HashSet<String> {
    schema
        .get(member)
        .and_then(Value::as_object)
        .map(|declared| declared.keys().cloned().collect())
        .unwrap_or_default()
}

/// Whether a value validates against the supplied schema node.
pub(super) fn validates(schema: &Value, value: &Value) -> bool {
    super::check::check(schema, value).is_ok_and(|errors| errors.is_empty())
}

/// Convert one argument candidate through the kinds its schema author declared.
pub(super) fn convert(value: &mut Value, schema: &Value, kinds: &HashMap<String, SchemaKind>) {
    Context {
        schema,
        kinds,
        bindings: Vec::new(),
    }
    .convert_at(value, "");
}

/// One conversion walk: the supplied schema, its authored kinds and the visible definitions.
pub(super) struct Context<'a> {
    /// Supplied schema value.
    schema: &'a Value,
    /// Authored kinds by JSON pointer.
    kinds: &'a HashMap<String, SchemaKind>,
    /// Definition identifiers visible to the node being converted, innermost last.
    bindings: Vec<(String, String)>,
}

impl Context<'_> {
    /// Convert a value against the node at the supplied pointer.
    fn convert_at(&mut self, value: &mut Value, pointer: &str) {
        let Some(kind) = self.kinds.get(pointer).cloned() else {
            return;
        };
        let Some(schema) = self.schema.pointer(pointer).cloned() else {
            return;
        };
        match kind {
            SchemaKind::String => scalars::to_string(value),
            SchemaKind::Number => scalars::to_number(value, false),
            SchemaKind::Integer => scalars::to_number(value, true),
            SchemaKind::Boolean => scalars::to_boolean(value),
            SchemaKind::Null => scalars::to_null(value),
            SchemaKind::Array => self.convert_array(value, pointer),
            SchemaKind::Tuple => self.convert_tuple(value, pointer, &schema),
            SchemaKind::Object => self.convert_object(value, pointer, &schema),
            SchemaKind::Record => self.convert_record(value, pointer, &schema),
            SchemaKind::Literal => composites::convert_literal(value, &schema),
            SchemaKind::Enum => composites::convert_enum(value, &schema),
            SchemaKind::Union => composites::convert_union(self, value, pointer),
            SchemaKind::Intersect => self.convert_intersect(value, pointer, &schema),
            SchemaKind::Ref | SchemaKind::Cyclic => self.convert_reference(value, pointer, &schema),
            SchemaKind::TemplateLiteral => templates::convert(value, &schema),
            _ => {}
        }
    }

    /// Convert a value against a child node of the current pointer.
    pub(super) fn convert_child(&mut self, value: &mut Value, pointer: &str, suffix: &str) {
        let child = collections::child_pointer(pointer, suffix);
        self.convert_at(value, &child);
    }

    /// Wrap a supplied value into a sequence and convert its items.
    fn convert_array(&mut self, value: &mut Value, pointer: &str) {
        let mut candidate = scalars::to_array(value);
        if let Some(values) = candidate.as_array_mut() {
            for member in values {
                self.convert_child(member, pointer, "items");
            }
        }
        *value = candidate;
    }

    /// Convert the leading positions of a sequence through their positional nodes.
    fn convert_tuple(&mut self, value: &mut Value, pointer: &str, schema: &Value) {
        let Some(items) = schema.get("items").and_then(Value::as_array) else {
            return;
        };
        let Some(values) = value.as_array_mut() else {
            return;
        };
        for (index, member) in values.iter_mut().enumerate().take(items.len()) {
            self.convert_child(member, pointer, &format!("items/{index}"));
        }
    }

    /// Convert present declared members, then declared additional members.
    fn convert_object(&mut self, value: &mut Value, pointer: &str, schema: &Value) {
        let Some(object) = value.as_object_mut() else {
            return;
        };
        let declared = keys(schema, "properties");
        let present: Vec<String> = object.keys().cloned().collect();
        for key in &declared {
            if let Some(member) = object.get_mut(key) {
                let suffix = format!("properties/{}", collections::escape(key));
                self.convert_child(member, pointer, &suffix);
            }
        }
        let uncovered: Vec<String> = present
            .into_iter()
            .filter(|name| !declared.contains(name))
            .collect();
        self.convert_additional(object, pointer, schema, uncovered);
    }

    /// Convert every member that no declared pattern covers.
    fn convert_record(&mut self, value: &mut Value, pointer: &str, schema: &Value) {
        let Some(object) = value.as_object_mut() else {
            return;
        };
        let patterns = schema
            .get("patternProperties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let expressions: Vec<(String, regress::Regex)> = patterns
            .keys()
            .filter_map(|pattern| Some((pattern.clone(), collections::pattern(pattern)?)))
            .collect();
        let names: Vec<String> = object.keys().cloned().collect();
        for declared in &expressions {
            self.convert_pattern(object, pointer, declared, &names);
        }
        let uncovered: Vec<String> = names
            .into_iter()
            .filter(|name| {
                !expressions
                    .iter()
                    .any(|(_, expression)| expression.find(name).is_some())
            })
            .collect();
        self.convert_additional(object, pointer, schema, uncovered);
    }

    /// Convert the present members one declared pattern covers.
    fn convert_pattern(
        &mut self,
        object: &mut serde_json::Map<String, Value>,
        pointer: &str,
        declared: &(String, regress::Regex),
        names: &[String],
    ) {
        let (pattern, expression) = declared;
        let suffix = format!("patternProperties/{}", collections::escape(pattern));
        for name in names {
            if expression.find(name).is_some()
                && let Some(member) = object.get_mut(name)
            {
                self.convert_child(member, pointer, &suffix);
            }
        }
    }

    /// Convert schema-valued additional members that no declared entry covers.
    fn convert_additional(
        &mut self,
        object: &mut serde_json::Map<String, Value>,
        pointer: &str,
        schema: &Value,
        uncovered: Vec<String>,
    ) {
        if !schema
            .get("additionalProperties")
            .is_some_and(Value::is_object)
        {
            return;
        }
        for name in uncovered {
            if let Some(member) = object.get_mut(&name) {
                self.convert_child(member, pointer, "additionalProperties");
            }
        }
    }

    /// Convert through the evaluated intersection of the declared members.
    fn convert_intersect(&mut self, value: &mut Value, pointer: &str, schema: &Value) {
        let members = schema
            .get("allOf")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if let Some(node) = evaluate::evaluate(self, pointer, &members) {
            Context {
                schema: &node.schema,
                kinds: &node.kinds,
                bindings: Vec::new(),
            }
            .convert_at(value, "");
        }
    }

    /// Convert through the definition a reference names.
    fn convert_reference(&mut self, value: &mut Value, pointer: &str, schema: &Value) {
        let publish = self.kinds.get(pointer) == Some(&SchemaKind::Cyclic);
        let added = publish.then(|| {
            let definitions = schema.get("$defs").cloned().unwrap_or(Value::Null);
            self.bind(pointer, &definitions)
        });
        if let Some(reference) = schema.get("$ref").and_then(Value::as_str)
            && let Some(target) = self
                .bindings
                .iter()
                .rev()
                .find(|(identifier, _)| identifier == reference)
                .map(|(_, target)| target.clone())
        {
            self.convert_at(value, &target);
        }
        if added.flatten().is_some() {
            self.bindings
                .truncate(self.bindings.len().saturating_sub(1));
        }
    }

    /// Publish the identifiers a definition block declares.
    fn bind(&mut self, pointer: &str, definitions: &Value) -> Option<usize> {
        let definitions = definitions.as_object()?;
        let before = self.bindings.len();
        for (key, definition) in definitions {
            let identifier = definition
                .get("$id")
                .and_then(Value::as_str)
                .unwrap_or(key)
                .to_owned();
            let target = format!("{pointer}/$defs/{}", collections::escape(key));
            self.bindings.push((identifier, target));
        }
        Some(self.bindings.len() - before)
    }

    /// Borrow the authored kinds.
    pub(super) fn kinds(&self) -> &HashMap<String, SchemaKind> {
        self.kinds
    }

    /// Borrow the supplied schema.
    pub(super) fn schema(&self) -> &Value {
        self.schema
    }
}
