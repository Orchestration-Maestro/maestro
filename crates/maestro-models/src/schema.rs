//! Identity-bearing schemas with offline checker compilation.
use crate::ThrownValue;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// Shared mutable schema identity with hidden per-node conversion metadata.
/// Clones share the successful compiled checker; replacement does not invalidate
/// it. Serialization emits only JSON and reparsing creates a plain new identity.
#[derive(Clone)]
pub struct TSchema {
    state: Arc<Mutex<State>>,
}
struct State {
    json: Arc<Value>,
    kinds: BTreeMap<Vec<String>, String>,
    legacy: bool,
    checker: Option<Arc<Checker>>,
}
impl TSchema {
    /// Own JSON, hidden kinds keyed by schema paths, and legacy symbol presence.
    pub fn new(json: Value, kinds: BTreeMap<Vec<String>, String>, has_typebox_kind: bool) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                json: Arc::new(json),
                kinds,
                legacy: has_typebox_kind,
                checker: None,
            })),
        }
    }
    /// Return an owned JSON snapshot without runtime metadata.
    pub fn json(&self) -> Value {
        let json = self
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .json
            .clone();
        crate::scalar::clone_json(&json)
    }
    /// Replace JSON and metadata while retaining identity and cached compilation.
    /// No caller code runs while the state lock is held.
    pub fn replace(
        &self,
        json: Value,
        kinds: BTreeMap<Vec<String>, String>,
        has_typebox_kind: bool,
    ) {
        let old = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            (
                std::mem::replace(&mut state.json, Arc::new(json)),
                std::mem::replace(&mut state.kinds, kinds),
                std::mem::replace(&mut state.legacy, has_typebox_kind),
            )
        };
        drop(old);
    }
    pub(crate) fn snapshot(&self) -> (Value, BTreeMap<Vec<String>, String>, bool) {
        let (json, kinds, legacy) = {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            (state.json.clone(), state.kinds.clone(), state.legacy)
        };
        (crate::scalar::clone_json(&json), kinds, legacy)
    }
    pub(crate) fn checker(&self) -> Result<Arc<Checker>, ThrownValue> {
        if let Some(cached) = self
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .checker
            .clone()
        {
            return Ok(cached);
        }
        let json = self.json();
        let checker = Arc::new(build(&json)?);
        if !json.is_object() && !json.is_array() {
            return Err(ThrownValue::Error(Box::new(crate::Error {
                name: "TypeError".into(),
                message: "Invalid value used as weak map key".into(),
                stack: None,
                code: None,
            })));
        }
        let result = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state
                .checker
                .get_or_insert_with(|| Arc::clone(&checker))
                .clone()
        };
        Ok(result)
    }
}
impl From<Value> for TSchema {
    fn from(json: Value) -> Self {
        Self::new(json, BTreeMap::new(), false)
    }
}
impl PartialEq for TSchema {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }
}
impl std::fmt::Debug for TSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TSchema").finish_non_exhaustive()
    }
}
impl serde::Serialize for TSchema {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let json = self
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .json
            .clone();
        json.serialize(serde_stacker::Serializer::new(serializer))
    }
}
impl<'de> serde::Deserialize<'de> for TSchema {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Value::deserialize(serde_stacker::Deserializer::new(deserializer)).map(Self::from)
    }
}

// Maximum measured compiler/check frame cost across native debug and release.
const STACK_BYTES_PER_LEVEL: usize = 6341;
pub(crate) struct Checker {
    validator: jsonschema::Validator,
    stack_size: usize,
}
impl Checker {
    pub(crate) fn is_valid(&self, value: &Value) -> Result<bool, ThrownValue> {
        foreign_stack(self.stack_size, || self.validator.is_valid(value))
    }
}
fn foreign_stack<T>(bytes: usize, f: impl FnOnce() -> T) -> Result<T, ThrownValue> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| stacker::grow(bytes, f))).map_err(
        |panic| {
            let message = panic
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| panic.downcast_ref::<&str>().copied())
                .unwrap_or("");
            if message.contains("allocate stack")
                || message.starts_with("mprotect/mmap failed:")
                || message.starts_with("unable to allocate fiber:")
                || message == "unreasonably large stack requested"
            {
                crate::records::diagnostics::error(
                    "Unable to allocate stack for schema checking".into(),
                )
            } else {
                std::panic::resume_unwind(panic)
            }
        },
    )
}
fn nesting_depth(schema: &Value) -> usize {
    let mut pending = vec![(schema, 1)];
    let mut deepest = 1;
    while let Some((value, depth)) = pending.pop() {
        deepest = deepest.max(depth);
        match value {
            Value::Array(values) => pending.extend(values.iter().map(|v| (v, depth + 1))),
            Value::Object(values) => pending.extend(values.values().map(|v| (v, depth + 1))),
            _ => {}
        }
    }
    deepest
}
pub(crate) fn build(schema: &Value) -> Result<Checker, ThrownValue> {
    let normalized = normalize(schema);
    let defaults = serde_stacker::Deserializer::new(());
    let stack_size = nesting_depth(&normalized)
        .checked_mul(STACK_BYTES_PER_LEVEL)
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(|| {
            crate::records::diagnostics::error(
                "Unable to allocate stack for schema checking".into(),
            )
        })?
        .max(defaults.stack_size);
    let result = foreign_stack(stack_size, || {
        jsonschema::options()
            .offline()
            .with_draft(jsonschema::Draft::Draft202012)
            .should_validate_formats(true)
            .build(&normalized)
    })?;
    crate::scalar::drop_json(normalized);
    result
        .map(|validator| Checker {
            validator,
            stack_size,
        })
        .map_err(|e| crate::records::diagnostics::error(e.to_string()))
}

pub(crate) fn keys(object: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut indices: Vec<_> = object
        .keys()
        .filter_map(|key| {
            let n = key.parse::<u32>().ok()?;
            (n != u32::MAX && n.to_string() == *key).then_some((n, key.clone()))
        })
        .collect();
    indices.sort_by_key(|(n, _)| *n);
    let mut result: Vec<_> = indices.into_iter().map(|(_, key)| key).collect();
    result.extend(
        object
            .keys()
            .filter(|key| !result.contains(key))
            .cloned()
            .collect::<Vec<_>>(),
    );
    result
}

pub(crate) fn errors(schema: &Value, value: &Value) -> Result<Vec<String>, ThrownValue> {
    let checker = build(schema)?;
    let mut diagnostics = Vec::new();
    foreign_stack(checker.stack_size, || {
        for e in checker.validator.iter_errors(value) {
            collect_error(schema, value, &e, &mut diagnostics)?;
        }
        Ok::<_, ThrownValue>(())
    })??;
    diagnostics.sort_by_key(|d| rank(schema, &d.order_path));
    Ok(diagnostics
        .into_iter()
        .take(8)
        .map(|d| {
            format!(
                "  - {}: {}",
                if d.path.is_empty() { "root" } else { &d.path },
                d.message
            )
        })
        .collect())
}
fn collect_error(
    schema: &Value,
    value: &Value,
    e: &jsonschema::ValidationError<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), ThrownValue> {
    use jsonschema::error::ValidationErrorKind as K;
    let raw_schema_path = e.schema_path().to_string();
    let mapped_schema_path = original_path(schema, &raw_schema_path);
    let mut order_path = original_path(schema, &e.evaluation_path().to_string());
    if let Some(parent) = mapped_schema_path
        .strip_suffix("/dependentRequired")
        .or_else(|| mapped_schema_path.strip_suffix("/dependencies"))
    {
        let instance_path = e.instance_path().to_string();
        let node = schema.pointer(parent).unwrap_or(schema);
        let object = value.pointer(&instance_path).unwrap_or(value);
        for keyword in ["dependencies", "dependentRequired"] {
            if let Some(entries) = node[keyword].as_object() {
                for key in keys(entries) {
                    let Some(required) = entries[&key].as_array() else {
                        continue;
                    };
                    if object.get(&key).is_none() {
                        continue;
                    }
                    let missing = required
                        .iter()
                        .filter_map(Value::as_str)
                        .filter(|name| object.get(*name).is_none())
                        .count();
                    let schema_path = format!(
                        "{parent}/{keyword}/{}",
                        key.replace('~', "~0").replace('/', "~1")
                    );
                    if diagnostics
                        .iter()
                        .any(|d| d.schema_path == schema_path && d.instance_path == instance_path)
                    {
                        continue;
                    }
                    let count = if keyword == "dependencies" {
                        usize::from(missing > 0)
                    } else {
                        missing
                    };
                    for _ in 0..count {
                        diagnostics.push(Diagnostic {
                            keyword: keyword.into(),
                            schema_path: schema_path.clone(),
                            order_path: schema_path.clone(),
                            instance_path: instance_path.clone(),
                            path: display_path(&instance_path),
                            message: format!(
                                "must have properties {} when property {key} is present",
                                required
                                    .iter()
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        });
                    }
                }
            }
        }
        return Ok(());
    }
    let mut keyword = e.kind().keyword().to_string();
    let mut schema_path = original_path(schema, &e.schema_path().to_string());
    let mut instance_path = e.instance_path().to_string();
    if schema_path.ends_with("/additionalItems")
        && matches!(e.kind(), K::FalseSchema)
        && diagnostics.iter().any(|d| {
            d.schema_path == schema_path
                && d.instance_path.rsplit_once('/').map(|(parent, _)| parent)
                    == instance_path.rsplit_once('/').map(|(parent, _)| parent)
        })
    {
        return Ok(());
    }
    let base = schema_path
        .rsplit_once('/')
        .map_or("", |(base, _)| base)
        .to_string();
    let mut node = schema.pointer(&base).unwrap_or(schema);
    let mut message_override = None;
    for part in [
        "additionalProperties",
        "propertyNames",
        "then",
        "else",
        "dependentRequired",
    ] {
        let needle = format!("/{part}");
        if let Some(index) = schema_path.find(&needle) {
            let parent = schema_path[..index].to_string();
            let parent_node = schema.pointer(&parent).unwrap_or(schema);
            if part == "additionalProperties" || part == "propertyNames" {
                keyword = part.into();
                schema_path = format!("{parent}/{part}");
                if part == "additionalProperties" {
                    instance_path = instance_path.rsplit_once('/').map_or("", |(p, _)| p).into();
                }
                node = parent_node;
            } else if part == "then" {
                keyword = "if".into();
                schema_path = format!("{parent}/if");
                order_path = schema_path.clone();
                node = parent_node;
                message_override = Some("must match \"then\" schema".to_string());
            } else if part == "else" {
                let parent_instance = instance_path.clone();
                let mut copied_path = schema_path.clone();
                copied_path.push_str("/~if");
                diagnostics.push(Diagnostic {
                    keyword: "if".into(),
                    order_path: copied_path.clone(),
                    schema_path: copied_path,
                    instance_path: parent_instance.clone(),
                    path: display_path(&parent_instance),
                    message: "must match \"else\" schema".into(),
                });
            } else if part == "dependentRequired" {
                let key = schema_path[index + needle.len()..]
                    .trim_start_matches('/')
                    .split('/')
                    .next()
                    .unwrap_or("")
                    .replace("~1", "/")
                    .replace("~0", "~");
                let dependency_key = if parent_node["dependentRequired"].get(&key).is_some() {
                    "dependentRequired"
                } else {
                    "dependencies"
                };
                let list = &parent_node[dependency_key][&key];
                keyword = dependency_key.into();
                schema_path = format!(
                    "{parent}/{dependency_key}/{}",
                    key.replace('~', "~0").replace('/', "~1")
                );
                node = parent_node;
                message_override = Some(format!(
                    "must have properties {} when property {key} is present",
                    list.as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            break;
        }
    }
    if matches!(
        keyword.as_str(),
        "required" | "additionalProperties" | "propertyNames" | "dependencies" | "if"
    ) && diagnostics.iter().any(|d| {
        d.keyword == keyword && d.schema_path == schema_path && d.instance_path == instance_path
    }) {
        return Ok(());
    }
    if let K::AnyOf { context } | K::OneOfNotValid { context } = e.kind() {
        for branch in context {
            for error in branch {
                collect_error(schema, value, error, diagnostics)?;
            }
        }
    }
    let mut path = display_path(&instance_path);
    let token = |key: &str| scalar_token(&node[key]);
    let message = if let Some(message) = message_override {
        message
    } else {
        match keyword.as_str() {
            "falseSchema" => "schema is false".into(),
            "additionalProperties" => "must not have additional properties".into(),
            "anyOf" => "must match a schema in anyOf".into(),
            "oneOf" => "must match exactly one schema in oneOf".into(),
            "const" => "must be equal to constant".into(),
            "enum" => "must be equal to one of the allowed values".into(),
            "contains" => "must contain at least 1 valid item".into(),
            "not" => "must not be valid".into(),
            "uniqueItems" => "must not have duplicate items".into(),
            "unevaluatedItems" => "must not have unevaluated items".into(),
            "unevaluatedProperties" => "must not have unevaluated properties".into(),
            "minimum" => format!("must be >= {}", token("minimum")),
            "maximum" => format!("must be <= {}", token("maximum")),
            "exclusiveMinimum" => format!("must be > {}", token("exclusiveMinimum")),
            "exclusiveMaximum" => format!("must be < {}", token("exclusiveMaximum")),
            "multipleOf" => format!("must be multiple of {}", token("multipleOf")),
            "maxItems" | "maxLength" | "maxProperties" => format!(
                "must not have more than {} {}",
                token(&keyword),
                if keyword == "maxItems" {
                    "items"
                } else if keyword == "maxLength" {
                    "characters"
                } else {
                    "properties"
                }
            ),
            "minItems" | "minLength" | "minProperties" => format!(
                "must not have fewer than {} {}",
                token(&keyword),
                if keyword == "minItems" {
                    "items"
                } else if keyword == "minLength" {
                    "characters"
                } else {
                    "properties"
                }
            ),
            "format" | "pattern" => format!(
                "must match {keyword} \"{}\"",
                node[&keyword].as_str().unwrap_or("")
            ),
            "required" => {
                let object = value.pointer(&instance_path).unwrap_or(value);
                let missing: Vec<_> = node["required"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|key| object.get(*key).is_none())
                    .collect();
                if let Some(first) = missing.first().filter(|key| !key.is_empty()) {
                    path = if path.is_empty() {
                        (*first).into()
                    } else {
                        format!("{path}.{first}")
                    };
                }
                format!("must have required properties {}", missing.join(", "))
            }
            "propertyNames" => {
                let object = value.pointer(&instance_path).unwrap_or(value);
                let checker = build(&node["propertyNames"])?;
                let invalid = object
                    .as_object()
                    .map(keys)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|k| !checker.is_valid(&Value::String(k.clone())).unwrap_or(false))
                    .collect::<Vec<_>>();
                format!("property names {} are invalid", invalid.join(", "))
            }
            "type" => {
                let kind = &node["type"];
                if let Some(kind) = kind.as_str() {
                    format!("must be {kind}")
                } else {
                    format!(
                        "must be either {}",
                        kind.as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(" or ")
                    )
                }
            }
            keyword => {
                return Err(crate::records::diagnostics::error(format!(
                    "Unmapped checker keyword {keyword}"
                )));
            }
        }
    };
    if keyword == "anyOf" || keyword == "oneOf" {
        schema_path.push_str("/~union");
        order_path.push_str("/~union");
    }
    diagnostics.push(Diagnostic {
        keyword,
        schema_path,
        order_path,
        instance_path,
        path,
        message,
    });
    Ok(())
}
fn scalar_token(value: &Value) -> String {
    if let Some(n) = value.as_f64() {
        ryu_js::Buffer::new().format(n).into()
    } else {
        value.to_string()
    }
}
fn original_path(schema: &Value, path: &str) -> String {
    let mut node = schema;
    let mut result = String::new();
    let parts: Vec<_> = path.trim_start_matches('/').split('/').collect();
    let mut at = 0;
    while at < parts.len() {
        let key = parts[at].replace("~1", "/").replace("~0", "~");
        if key == "allOf"
            && let Some(index) = parts.get(at + 1).and_then(|s| s.parse::<usize>().ok())
        {
            let existing = node["allOf"].as_array().map_or(0, Vec::len);
            if index >= existing
                && let Some((source_key, compiled_key, _)) =
                    extra_constraints(node).get(index - existing)
                && parts.get(at + 2) == Some(compiled_key)
            {
                result.push('/');
                result.push_str(source_key);
                node = &node[*source_key];
                at += 3;
                continue;
            }
        }
        let original = if key == "prefixItems" && node["items"].is_array() {
            "items"
        } else if key == "items" && node["items"].is_array() {
            "additionalItems"
        } else if (key == "dependentSchemas" || key == "dependentRequired")
            && node.get(&key).is_none()
            && node.get("dependencies").is_some()
        {
            "dependencies"
        } else {
            &key
        };
        result.push('/');
        result.push_str(&original.replace('~', "~0").replace('/', "~1"));
        node = node.get(original).unwrap_or(&Value::Null);
        at += 1;
    }
    result
}

struct Diagnostic {
    keyword: String,
    schema_path: String,
    order_path: String,
    instance_path: String,
    path: String,
    message: String,
}
fn display_path(path: &str) -> String {
    path.strip_prefix('/')
        .unwrap_or(path)
        .replace("~1", "/")
        .replace("~0", "~")
        .replace('/', ".")
}
const KEYWORDS: &[&str] = &[
    "type",
    "required",
    "additionalProperties",
    "dependencies",
    "dependentRequired",
    "dependentSchemas",
    "patternProperties",
    "properties",
    "propertyNames",
    "minProperties",
    "maxProperties",
    "additionalItems",
    "contains",
    "items",
    "maxContains",
    "maxItems",
    "minContains",
    "minItems",
    "prefixItems",
    "uniqueItems",
    "maxLength",
    "minLength",
    "format",
    "pattern",
    "exclusiveMaximum",
    "exclusiveMinimum",
    "maximum",
    "minimum",
    "multipleOf",
    "$ref",
    "$recursiveRef",
    "$dynamicRef",
    "~guard",
    "const",
    "enum",
    "if",
    "not",
    "allOf",
    "anyOf",
    "oneOf",
    "unevaluatedItems",
    "unevaluatedProperties",
    "~refine",
];
fn rank(schema: &Value, path: &str) -> Vec<usize> {
    let mut node = schema;
    let mut ranks = Vec::new();
    for encoded in path.trim_start_matches('/').split('/') {
        let key = encoded.replace("~1", "/").replace("~0", "~");
        let index = if key.starts_with("~union") || key.starts_with("~if") {
            usize::MAX
        } else if let Some(array) = node.as_array() {
            key.parse::<usize>().unwrap_or(array.len())
        } else if let Some(object) = node.as_object() {
            if KEYWORDS.contains(&key.as_str()) {
                KEYWORDS.iter().position(|k| *k == key).unwrap()
            } else {
                keys(object)
                    .iter()
                    .position(|k| *k == key)
                    .unwrap_or(object.len())
            }
        } else {
            0
        };
        ranks.push(index);
        node = node.get(&key).unwrap_or(&Value::Null);
    }
    ranks
}

fn normalize(schema: &Value) -> Value {
    normalize_at(schema, schema)
}
fn normalize_at(schema: &Value, root: &Value) -> Value {
    crate::scalar::grow(|| {
        let Some(source) = schema.as_object() else {
            return if schema.is_boolean() {
                crate::scalar::clone_json(schema)
            } else {
                serde_json::json!({})
            };
        };
        let Value::Object(mut object) = crate::scalar::clone_json(schema) else {
            unreachable!()
        };
        object.shift_remove("$schema");
        if !source.contains_key("$ref")
            && let Some(reference) = object.shift_remove("$recursiveRef")
        {
            object.insert("$ref".into(), reference);
        } else {
            object.shift_remove("$recursiveRef");
        }
        if let Some(reference) = source.get("$ref").and_then(Value::as_str) {
            let resolved = reference == "#"
                || reference
                    .strip_prefix('#')
                    .is_some_and(|path| root.pointer(path).is_some());
            if !resolved {
                return Value::Bool(false);
            }
        }
        if let Some(kind) = object.get("type") {
            let known = |s: &str| {
                [
                    "object", "array", "boolean", "integer", "number", "null", "string",
                ]
                .contains(&s)
            };
            let usable = match kind {
                Value::String(s) => known(s),
                Value::Array(ts) => {
                    !ts.is_empty() && ts.iter().all(|t| t.as_str().is_some_and(known))
                }
                _ => false,
            };
            if !usable {
                object.shift_remove("type");
            }
        }
        for key in [
            "minimum",
            "maximum",
            "exclusiveMinimum",
            "exclusiveMaximum",
            "multipleOf",
        ] {
            if object.get(key).is_some_and(|v| !v.is_number()) {
                object.shift_remove(key);
            }
        }
        if object
            .get("required")
            .is_some_and(|v| !v.as_array().is_some_and(|a| a.iter().all(Value::is_string)))
        {
            object.shift_remove("required");
        }
        for key in [
            "properties",
            "patternProperties",
            "$defs",
            "definitions",
            "dependentSchemas",
        ] {
            if let Some(map) = object.get_mut(key).and_then(Value::as_object_mut) {
                for v in map.values_mut() {
                    *v = normalize_at(v, root);
                }
            }
        }
        for key in ["allOf", "anyOf", "oneOf", "prefixItems"] {
            if let Some(array) = object.get_mut(key).and_then(Value::as_array_mut) {
                for v in array {
                    *v = normalize_at(v, root);
                }
            }
        }
        for key in [
            "additionalProperties",
            "items",
            "additionalItems",
            "contains",
            "propertyNames",
            "if",
            "then",
            "else",
            "not",
            "unevaluatedProperties",
            "unevaluatedItems",
        ] {
            if let Some(v) = object.get_mut(key) {
                if key == "items"
                    && let Some(array) = v.as_array_mut()
                {
                    for v in array {
                        *v = normalize_at(v, root);
                    }
                } else {
                    *v = normalize_at(v, root);
                }
            }
        }
        if object.get("items").is_some_and(Value::is_array) {
            let tuple = object.shift_remove("items").unwrap();
            object.insert("prefixItems".into(), tuple);
            if let Some(extra) = object.shift_remove("additionalItems") {
                object.insert("items".into(), extra);
            }
        } else {
            object.shift_remove("additionalItems");
        }
        if let Some(dependencies) = object
            .shift_remove("dependencies")
            .and_then(|v| v.as_object().cloned())
        {
            for (key, nested) in dependencies {
                let keyword = if nested.is_array() {
                    "dependentRequired"
                } else {
                    "dependentSchemas"
                };
                let entry = object
                    .entry(keyword)
                    .or_insert_with(|| serde_json::json!({}));
                if let Some(map) = entry.as_object_mut()
                    && !source.contains_key(keyword)
                {
                    map.insert(
                        key,
                        if nested.is_array() {
                            nested
                        } else {
                            normalize_at(&nested, root)
                        },
                    );
                }
            }
        }
        let extra = extra_constraints(schema);
        if !extra.is_empty() {
            let branches = object
                .entry("allOf")
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Some(branches) = branches.as_array_mut() {
                for (_source_key, compiled_key, nested) in extra {
                    let nested = match compiled_key {
                        "prefixItems" => Value::Array(
                            nested
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|v| normalize_at(v, root))
                                .collect(),
                        ),
                        "dependentSchemas" => Value::Object(
                            nested
                                .as_object()
                                .unwrap()
                                .iter()
                                .map(|(key, v)| (key.clone(), normalize_at(v, root)))
                                .collect(),
                        ),
                        _ => nested,
                    };
                    branches.push(serde_json::json!({compiled_key:nested}));
                }
            }
        }
        Value::Object(object)
    })
}

fn extra_constraints(schema: &Value) -> Vec<(&str, &str, Value)> {
    let mut extra = Vec::new();
    if schema["items"].is_array() && schema["prefixItems"].is_array() {
        extra.push(("prefixItems", "prefixItems", schema["prefixItems"].clone()));
    }
    if let Some(entries) = schema["dependencies"].as_object() {
        for (source_key, compiled_key, want_array) in [
            ("dependencies", "dependentRequired", true),
            ("dependencies", "dependentSchemas", false),
        ] {
            if schema.get(compiled_key).is_some() {
                let map: serde_json::Map<_, _> = entries
                    .iter()
                    .filter(|(_, v)| v.is_array() == want_array)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                if !map.is_empty() {
                    extra.push((source_key, compiled_key, Value::Object(map)));
                }
            }
        }
    }
    if schema.get("$ref").is_some()
        && let Some(reference) = schema.get("$recursiveRef")
    {
        extra.push(("$recursiveRef", "$ref", reference.clone()));
    }
    extra
}

pub(crate) fn pattern_matches(pattern: &str, text: &str) -> bool {
    regress::Regex::new(pattern).is_ok_and(|regex| {
        regex
            .find_from_ucs2(&text.encode_utf16().collect::<Vec<_>>(), 0)
            .next()
            .is_some()
    })
}

#[cfg(test)]
mod stack_measurements {
    use serde_json::{Value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Probe {
        minimum: Arc<AtomicUsize>,
    }
    impl<'i> jsonschema::Keyword<'i> for Probe {
        fn validate(&self, _instance: &'i Value) -> Result<(), jsonschema::ValidationError<'i>> {
            self.minimum
                .fetch_min(stacker::remaining_stack().unwrap(), Ordering::SeqCst);
            Ok(())
        }
        fn is_valid(&self, _instance: &'i Value) -> bool {
            self.minimum
                .fetch_min(stacker::remaining_stack().unwrap(), Ordering::SeqCst);
            true
        }
    }
    #[test]
    fn foreign_stack_allocation_failure_is_returned_as_error() {
        let result = super::foreign_stack(usize::MAX, || panic!("callback must not run"));
        let crate::ThrownValue::Error(error) = result.unwrap_err() else {
            panic!("expected Error");
        };
        assert_eq!(
            error.message,
            "Unable to allocate stack for schema checking"
        );
    }
    #[test]
    fn foreign_schema_calls_measure_per_level_stack() {
        for shape in ["items", "properties", "allOf", "anyOf"] {
            for depth in [100, 200, 400] {
                let mut schema = json!({"stackDepthObservation":true});
                let mut input = json!(42);
                for _ in 0..depth {
                    match shape {
                        "properties" => {
                            schema = json!({"type":"object","properties":{"v":schema}});
                            input = json!({"v":input});
                        }
                        "allOf" | "anyOf" => schema = json!({shape:[schema]}),
                        _ => {
                            schema = json!({"type":"array","items":schema});
                            input = json!([input]);
                        }
                    }
                }
                let minimum = Arc::new(AtomicUsize::new(usize::MAX));
                let observation = minimum.clone();
                let (compile, check) = stacker::grow(32 * 1024 * 1024, || {
                    let start = stacker::remaining_stack().unwrap();
                    let checker = jsonschema::options()
                        .offline()
                        .with_draft(jsonschema::Draft::Draft202012)
                        .with_keyword("stackDepthObservation", move |_, _, _| {
                            observation
                                .fetch_min(stacker::remaining_stack().unwrap(), Ordering::SeqCst);
                            Ok(Box::new(Probe {
                                minimum: observation.clone(),
                            }))
                        })
                        .build(&schema)
                        .unwrap();
                    let compile = start - minimum.swap(usize::MAX, Ordering::SeqCst);
                    let start = stacker::remaining_stack().unwrap();
                    assert!(checker.is_valid(&input));
                    let check = start - minimum.load(Ordering::SeqCst);
                    (compile, check)
                });
                println!(
                    "{shape} depth={depth} compile={compile} check={check} compile_per_level={} check_per_level={}",
                    compile.div_ceil(depth),
                    check.div_ceil(depth)
                );
            }
        }
    }
}
