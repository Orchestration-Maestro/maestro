//! Resolution of a plain reference to the schema node it names.
//!
//! The search is source-shaped: a node that answers the reference itself wins, and otherwise
//! the child subtrees are folded in order, keeping the last one that answers. A pointer
//! fragment therefore never prefers an earlier sibling declaration.

use serde_json::Value;

use super::super::super::references::{Location, children};

/// Search a resource for the node a pointer fragment addresses.
///
/// Matching starts at the supplied node: a node whose own subtree holds the pointer answers
/// first, and its children are not searched again below it. A node that does not answer hands
/// its children over in canonical order, each replacing a previous match.
pub(crate) fn from_value<'a>(start: &Location<'a>, fragment: &str) -> Option<Location<'a>> {
    if let Some(found) = pointer(start, fragment) {
        return Some(found);
    }
    children(start.schema)
        .into_iter()
        .fold(None, |result, child| {
            from_value(&start.child(child), fragment).or(result)
        })
}

/// Traverse decoded pointer segments, entering each crossed resource once.
pub(crate) fn pointer<'a>(start: &Location<'a>, fragment: &str) -> Option<Location<'a>> {
    let mut location = start.clone();
    for segment in fragment.strip_prefix('/')?.split('/') {
        let key = segment.replace("~1", "/").replace("~0", "~");
        if matches!(key.as_str(), "__proto__" | "constructor" | "prototype") {
            return None;
        }
        let next = match location.schema {
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
        location = location.child(next);
    }
    Some(location)
}
