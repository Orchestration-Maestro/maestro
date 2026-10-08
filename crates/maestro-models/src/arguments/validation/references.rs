//! Offline schema resource resolution with recursive and dynamic scopes.

use percent_encoding::percent_decode_str;
use serde_json::Value;
use url::Url;

use super::check::{Batch, Instruction, Job, Mode};

/// Schedule all reference keywords through the shared offline resolver.
pub(super) fn instructions<'a>(
    root: &'a Value,
    job: &Job<'a>,
    scopes: &[&'a Value],
) -> Vec<Instruction<'a>> {
    /// Rejecting target schema used when offline resolution fails.
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

/// Resolve a reference in its active resource and anchor scopes.
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
    let base_uri = scopes
        .iter()
        .filter_map(|schema| schema.get("$id").and_then(Value::as_str))
        .try_fold(Url::parse("http://unknown/").ok()?, |uri, id| {
            uri.join(id).ok()
        })?;
    if keyword == "$recursiveRef" && base.get("$recursiveAnchor") == Some(&Value::Bool(true)) {
        let anchor = scopes
            .iter()
            .find(|schema| schema.get("$recursiveAnchor") == Some(&Value::Bool(true)))
            .copied()?;
        let uri = root_uri(anchor)?;
        return resolve(anchor, reference, &uri, &uri);
    }
    let schema = if reference.starts_with('#') || keyword == "$recursiveRef" {
        base
    } else {
        root
    };
    let schema_uri = if std::ptr::eq(schema, base) {
        base_uri.clone()
    } else {
        root_uri(root)?
    };
    let target = resolve(schema, reference, &base_uri, &schema_uri)?;
    if keyword == "$dynamicRef"
        && !reference.split('#').nth(1).is_some_and(|fragment| {
            percent_decode_str(fragment)
                .decode_utf8_lossy()
                .starts_with('/')
        })
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

/// Find a dynamic anchor without crossing another resource boundary.
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

/// Search resolved identifiers, pointers and anchors.
fn resolve<'a>(
    root: &'a Value,
    reference: &str,
    base_uri: &Url,
    root_uri: &Url,
) -> Option<&'a Value> {
    let target = base_uri.join(reference).ok()?;
    let identity = target.as_str().split('#').next()?;
    let fragment = percent_decode_str(target.fragment().unwrap_or(""))
        .decode_utf8()
        .ok()?;
    let mut pending = vec![(root, root_uri.clone())];
    let mut result = None;
    while let Some((schema, inherited)) = pending.pop() {
        let uri = inherited_uri(schema, root, inherited)?;
        if uri.as_str().split('#').next()? == identity {
            let id = schema.get("$id").and_then(Value::as_str);
            let base = std::ptr::eq(schema, root) || id.is_some();
            if (id.is_some_and(|id| id.starts_with('#')) && uri == target)
                || (fragment.is_empty() && base)
            {
                result = Some(schema);
                continue;
            }
            if fragment.starts_with('/')
                && base
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

/// Traverse decoded pointer segments while rejecting unsafe property keys.
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
                if !key.bytes().all(|byte| byte.is_ascii_digit())
                    || (key != "0" && key.starts_with('0'))
                {
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
/// Active resource and anchor bindings used to distinguish reference cycles.
pub(super) struct ScopeIdentity<'a> {
    /// Identity of the innermost active resource schema.
    resource: usize,
    /// Identity of the outermost active recursive anchor, if any.
    recursive: Option<usize>,
    /// Outermost active schema identity for each dynamic anchor name.
    dynamic: std::collections::BTreeMap<&'a str, usize>,
}

/// Capture resource and anchor bindings for non-progress cycle detection.
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

/// Resolve the root identifier against the offline default base.
fn root_uri(root: &Value) -> Option<Url> {
    let default = Url::parse("http://unknown/").ok()?;
    match root.get("$id").and_then(Value::as_str) {
        Some(id) => default.join(id).ok(),
        None => Some(default),
    }
}

/// Carry the enclosing URI unless a child declares its own identifier.
fn inherited_uri(schema: &Value, root: &Value, inherited: Url) -> Option<Url> {
    if std::ptr::eq(schema, root) {
        Some(inherited)
    } else {
        match schema.get("$id").and_then(Value::as_str) {
            Some(id) => inherited.join(id).ok(),
            None => Some(inherited),
        }
    }
}
