use percent_encoding::percent_decode_str;
use serde_json::Value;
use url::Url;

use super::check::{Batch, Instruction, Job, Mode};

pub(super) fn instructions<'a>(
    root: &'a Value,
    job: &Job<'a>,
    scopes: &[&'a Value],
) -> Vec<Instruction<'a>> {
    const FALSE: Value = Value::Bool(false);
    let mut instructions = Vec::new();
    for keyword in ["$ref", "$recursiveRef", "$dynamicRef"] {
        let Some(reference) = job.schema.get(keyword).and_then(Value::as_str) else {
            continue;
        };
        let target = target(root, reference, keyword, scopes)
            .filter(|schema| schema.is_object() || schema.is_boolean())
            .unwrap_or(&FALSE);
        let jobs = [Job {
            schema: target,
            ..job.clone()
        }]
        .into();
        instructions.push(Instruction::Children(Batch {
            mode: Mode::All,
            jobs,
            results: Vec::new(),
        }));
    }
    instructions
}

fn target<'a>(
    root: &'a Value,
    reference: &str,
    keyword: &str,
    scopes: &[&'a Value],
) -> Option<&'a Value> {
    let base = scopes
        .iter()
        .rev()
        .find(|schema| schema.get("$id").and_then(Value::as_str).is_some())
        .copied()
        .unwrap_or(root);
    if keyword == "$recursiveRef" && base.get("$recursiveAnchor") == Some(&Value::Bool(true)) {
        let anchor = scopes
            .iter()
            .find(|schema| schema.get("$recursiveAnchor") == Some(&Value::Bool(true)))
            .copied()?;
        return resolve(anchor, reference);
    }
    let schema = if reference.starts_with('#') {
        base
    } else {
        root
    };
    let target = resolve(schema, reference)?;
    if keyword == "$dynamicRef"
        && !reference
            .split('#')
            .nth(1)
            .is_some_and(|fragment| fragment.starts_with('/'))
        && let Some(name) = target.get("$dynamicAnchor").and_then(Value::as_str)
    {
        for scope in scopes {
            if scope.get("$id").and_then(Value::as_str).is_some()
                && let Some(anchor) = resource_anchor(scope, name)
            {
                return Some(anchor);
            }
            if scope.get("$dynamicAnchor").and_then(Value::as_str) == Some(name) {
                return Some(scope);
            }
        }
    }
    Some(target)
}

fn resource_anchor<'a>(root: &'a Value, name: &str) -> Option<&'a Value> {
    let mut pending = vec![root];
    while let Some(schema) = pending.pop() {
        if !std::ptr::eq(schema, root) && schema.get("$id").and_then(Value::as_str).is_some() {
            continue;
        }
        if schema.get("$dynamicAnchor").and_then(Value::as_str) == Some(name) {
            return Some(schema);
        }
        if let Value::Object(object) = schema {
            pending.extend(
                super::diagnostics::entries(object)
                    .into_iter()
                    .rev()
                    .map(|(_, schema)| schema),
            );
        }
    }
    None
}

fn resolve<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    let root_uri = root_uri(root)?;
    let target = root_uri.join(reference).ok()?;
    let mut identity = target.clone();
    identity.set_fragment(None);
    let fragment = percent_decode_str(target.fragment().unwrap_or(""))
        .decode_utf8()
        .ok()?;
    let mut pending = vec![(root, root_uri)];
    let mut result = None;
    while let Some((schema, inherited)) = pending.pop() {
        let uri = inherited_uri(schema, root, inherited)?;
        let mut resource_identity = uri.clone();
        resource_identity.set_fragment(None);
        if resource_identity == identity {
            if fragment.is_empty()
                && (std::ptr::eq(schema, root)
                    || schema.get("$id").and_then(Value::as_str).is_some())
            {
                result = Some(schema);
                continue;
            }
            if fragment.starts_with('/')
                && (std::ptr::eq(schema, root)
                    || schema.get("$id").and_then(Value::as_str).is_some())
                && let Some(target) = pointer(schema, &fragment)
            {
                result = Some(target);
                continue;
            }
            if ["$anchor", "$dynamicAnchor"]
                .iter()
                .any(|key| schema.get(key).and_then(Value::as_str) == Some(&fragment))
            {
                result = Some(schema);
                continue;
            }
        }
        match schema {
            Value::Object(object) => {
                pending.extend(
                    super::diagnostics::entries(object)
                        .into_iter()
                        .rev()
                        .map(|(_, schema)| schema)
                        .map(|schema| (schema, uri.clone())),
                );
            }
            Value::Array(array) => {
                pending.extend(array.iter().rev().map(|schema| (schema, uri.clone())));
            }
            _ => {}
        }
    }
    result
}

fn pointer<'a>(root: &'a Value, fragment: &str) -> Option<&'a Value> {
    let mut value = root;
    for segment in fragment.strip_prefix('/')?.split('/') {
        let key = segment.replace("~1", "/").replace("~0", "~");
        if matches!(key.as_str(), "__proto__" | "constructor" | "prototype") {
            return None;
        }
        value = match value {
            Value::Object(object) => object.get(&key)?,
            Value::Array(array) => {
                if key != "0" && key.starts_with('0') {
                    return None;
                }
                array.get(key.parse::<usize>().ok()?)?
            }
            _ => return None,
        };
    }
    Some(value)
}

#[derive(PartialEq, Eq)]
pub(super) struct ScopeIdentity<'a> {
    resource: usize,
    recursive: Option<usize>,
    dynamic: std::collections::BTreeMap<&'a str, usize>,
}

pub(super) fn scope_identity<'a>(root: &'a Value, scopes: &[&'a Value]) -> ScopeIdentity<'a> {
    let resource = scopes
        .iter()
        .rev()
        .find(|schema| schema.get("$id").and_then(Value::as_str).is_some())
        .copied()
        .unwrap_or(root);
    let recursive = scopes
        .iter()
        .find(|schema| schema.get("$recursiveAnchor") == Some(&Value::Bool(true)))
        .map(|schema| std::ptr::from_ref(*schema).addr());
    let mut dynamic = std::collections::BTreeMap::new();
    for schema in scopes {
        if let Some(name) = schema.get("$dynamicAnchor").and_then(Value::as_str) {
            dynamic
                .entry(name)
                .or_insert_with(|| std::ptr::from_ref(*schema).addr());
        }
    }
    ScopeIdentity {
        resource: std::ptr::from_ref(resource).addr(),
        recursive,
        dynamic,
    }
}

fn root_uri(root: &Value) -> Option<Url> {
    let default = Url::parse("http://unknown/").ok()?;
    root.get("$id")
        .and_then(Value::as_str)
        .map_or_else(|| Some(default.clone()), |id| default.join(id).ok())
}

fn inherited_uri(schema: &Value, root: &Value, inherited: Url) -> Option<Url> {
    if std::ptr::eq(schema, root) {
        Some(inherited)
    } else {
        schema
            .get("$id")
            .and_then(Value::as_str)
            .map_or_else(|| Some(inherited.clone()), |id| inherited.join(id).ok())
    }
}
