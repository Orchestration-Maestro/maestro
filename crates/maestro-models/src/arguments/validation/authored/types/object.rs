//! Object property modifiers represented from supplied required names.

use std::borrow::Cow;
use std::collections::HashSet;
use std::rc::Rc;

use serde_json::Value;

use crate::arguments::validation::authored::schema_view::{Member, Node, View};

#[cfg(test)]
thread_local! {
    /// Required-name work performed on this thread, read by the work-bound tests.
    pub(crate) static REQUIRED_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Charge one required-name operation to the work-bound tests.
#[cfg(test)]
pub(crate) fn charge_required_work() {
    REQUIRED_WORK.with(|count| count.set(count.get() + 1));
}

/// Charge one required-name operation to the work-bound tests.
#[cfg(not(test))]
pub(crate) fn charge_required_work() {}

/// The members an object declares, with the optionality its required names leave.
pub(crate) fn declared<'a>(
    view: View<'a>,
    mut node: impl FnMut(View<'a>) -> Rc<Node<'a>>,
) -> Option<Vec<Member<'a>>> {
    let properties = view.raw.get("properties").and_then(Value::as_object)?;
    let required: HashSet<&str> = view
        .raw
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .inspect(|_| charge_required_work())
        .collect();
    Some(
        properties
            .iter()
            .map(|(name, child)| {
                charge_required_work();
                Member {
                    name: Cow::Borrowed(name),
                    node: node(view.child(child)),
                    optional: !required.contains(name.as_str()),
                }
            })
            .collect(),
    )
}

/// The members an object-like node declares, compared through its own modifiers.
pub(crate) fn properties<'n, 'a>(node: &'n Node<'a>) -> Option<Cow<'n, [Member<'a>]>> {
    match node {
        Node::Object(object) => Some(Cow::Borrowed(&object.properties)),
        Node::Leaf(view) if view.kind == Some(&crate::SchemaKind::Object) => {
            declared(*view, |view| Rc::new(Node::Leaf(view))).map(Cow::Owned)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::{Map, Value, json};

    use super::REQUIRED_WORK;
    use crate::SchemaKind;
    use crate::arguments::validation::authored::context::Context;
    use crate::arguments::validation::authored::schema_view::Document;
    use crate::arguments::validation::authored::types::engine::instantiate::instantiate;
    use crate::arguments::validation::{Tool, ToolCall, validate_tool_arguments};

    /// An object requiring every one of the supplied number of text members, and its kinds.
    fn wide(width: usize) -> (Value, HashMap<String, SchemaKind>) {
        let names: Vec<String> = (0..width).map(|index| format!("p{index}")).collect();
        let properties: Map<String, Value> = names
            .iter()
            .map(|name| (name.clone(), json!({"type": "string"})))
            .collect();
        let schema = json!({"type": "object", "required": names, "properties": properties});
        let mut kinds = HashMap::from([(String::new(), SchemaKind::Object)]);
        for name in &names {
            kinds.insert(format!("/properties/{name}"), SchemaKind::String);
        }
        (schema, kinds)
    }

    /// Required-name work the supplied action performs.
    fn work(action: impl FnOnce()) -> usize {
        REQUIRED_WORK.with(|count| count.set(0));
        action();
        REQUIRED_WORK.with(std::cell::Cell::get)
    }

    /// Constructing an object reads its required names once per property.
    #[test]
    fn construction_indexes_each_required_name_once() {
        for width in [16, 32, 64] {
            let (schema, kinds) = wide(width);
            let document = Document::new(&schema, &kinds);
            let code = crate::arguments::validation::check::Code::default();
            let converter = Context::new(&document, &code);
            let performed = work(|| {
                let _ = instantiate(&converter, converter.document.root());
            });
            assert!(
                performed <= 2 * width,
                "{performed} required-name operations for {width} properties"
            );
        }
    }

    /// The public conversion route charges the same construction work.
    #[test]
    fn public_conversion_indexes_each_required_name_once() {
        for width in [16, 32, 64] {
            let (inner, inner_kinds) = wide(width);
            let schema = json!({
                "type": "object", "required": ["value"],
                "properties": {"value": {"allOf": [inner, {}]}},
            });
            let mut kinds = HashMap::from([
                (String::new(), SchemaKind::Object),
                ("/properties/value".to_owned(), SchemaKind::Intersect),
                ("/properties/value/allOf/1".to_owned(), SchemaKind::Unknown),
            ]);
            for (pointer, kind) in inner_kinds {
                kinds.insert(format!("/properties/value/allOf/0{pointer}"), kind);
            }
            let arguments: Map<String, Value> =
                schema["properties"]["value"]["allOf"][0]["properties"]
                    .as_object()
                    .expect("declared properties")
                    .keys()
                    .map(|name| (name.clone(), Value::Null))
                    .collect();
            let tool: Tool = Tool {
                name: "echo".to_owned(),
                description: String::new(),
                parameters: crate::ParameterSchema::authored(schema, kinds),
            };
            let call = ToolCall {
                id: "call".to_owned(),
                name: "echo".to_owned(),
                arguments: Value::Object(Map::from_iter([(
                    "value".to_owned(),
                    Value::Object(arguments),
                )])),
                thought_signature: None,
            };
            let mut prepared = Value::Null;
            let performed = work(|| {
                prepared = validate_tool_arguments(&tool, &call).expect("prepared arguments");
            });
            assert!(
                prepared["value"]
                    .as_object()
                    .expect("prepared members")
                    .values()
                    .all(|value| value == "null"),
                "prepared {prepared:?}"
            );
            assert!(
                performed <= 2 * width,
                "{performed} required-name operations for {width} properties"
            );
        }
    }
}
