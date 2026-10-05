use crate::{SettingsOrigin, SettingsSnapshot};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub(crate) fn merge(
    snapshot: &mut SettingsSnapshot,
    layer: &Map<String, Value>,
    origin: SettingsOrigin,
) {
    merge_object(
        &mut snapshot.values,
        layer,
        &mut Vec::new(),
        &mut snapshot.origins,
        &origin,
    );
}
fn merge_object(
    base: &mut Map<String, Value>,
    layer: &Map<String, Value>,
    path: &mut Vec<String>,
    origins: &mut BTreeMap<Vec<String>, SettingsOrigin>,
    origin: &SettingsOrigin,
) {
    for (key, value) in layer {
        path.push(key.clone());
        if let (Some(Value::Object(old)), Value::Object(new)) = (base.get_mut(key), value) {
            origins.remove(path);
            merge_object(old, new, path, origins, origin);
            if old.is_empty() {
                origins.insert(path.clone(), origin.clone());
            }
        } else {
            origins.retain(|p, _| !p.starts_with(path));
            base.insert(key.clone(), value.clone());
            record(value, path, origins, origin);
        }
        path.pop();
    }
}
fn record(
    value: &Value,
    path: &mut Vec<String>,
    origins: &mut BTreeMap<Vec<String>, SettingsOrigin>,
    origin: &SettingsOrigin,
) {
    if let Value::Object(object) = value
        && !object.is_empty()
    {
        for (key, child) in object {
            path.push(key.clone());
            record(child, path, origins, origin);
            path.pop();
        }
    } else {
        origins.insert(path.clone(), origin.clone());
    }
}

pub(crate) fn replace(
    map: &mut Map<String, Value>,
    path: &[String],
    value: Value,
    origin: &SettingsOrigin,
) -> Result<(), crate::SettingsError> {
    let invalid = || crate::SettingsError::InvalidPath {
        path: path.to_vec(),
        origin: origin.clone(),
    };
    let Some((last, ancestors)) = path.split_last() else {
        *map = value.as_object().ok_or_else(invalid)?.clone();
        return Ok(());
    };
    let mut current = map;
    for segment in ancestors {
        let child = current
            .entry(segment.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        current = child.as_object_mut().ok_or_else(invalid)?;
    }
    current.insert(last.clone(), value);
    Ok(())
}

pub(crate) fn replace_effective(
    snapshot: &mut SettingsSnapshot,
    path: &[String],
    value: Value,
    origin: &SettingsOrigin,
) -> Result<(), crate::SettingsError> {
    replace(&mut snapshot.values, path, value.clone(), origin)?;
    snapshot
        .origins
        .retain(|p, _| !p.starts_with(path) && !(path.starts_with(p) && p != path));
    if path.is_empty() {
        for (key, child) in value.as_object().expect("validated object root") {
            record(child, &mut vec![key.clone()], &mut snapshot.origins, origin);
        }
    } else {
        record(&value, &mut path.to_vec(), &mut snapshot.origins, origin);
    }
    Ok(())
}

pub(crate) fn lookup<'a>(
    map: &'a Map<String, Value>,
    path: &[String],
) -> Result<Option<&'a Value>, ()> {
    let Some((first, rest)) = path.split_first() else {
        return Err(());
    };
    let mut value = map.get(first);
    for segment in rest {
        value = match value {
            None => return Ok(None),
            Some(Value::Object(object)) => object.get(segment),
            Some(_) => return Err(()),
        };
    }
    Ok(value)
}

pub(crate) fn guard(
    baseline: &SettingsSnapshot,
    candidate: &Map<String, Value>,
    origin: &SettingsOrigin,
) -> Result<(), crate::SettingsError> {
    for lock in &baseline.locks {
        if lookup(&baseline.values, lock) != lookup(candidate, lock) {
            return Err(crate::SettingsError::LockConflict {
                path: lock.clone(),
                locked_by: SettingsOrigin::Manifest,
                attempted_by: origin.clone(),
            });
        }
    }
    Ok(())
}
