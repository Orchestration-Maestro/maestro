use maestro_request::parameter_schema::{Refinement, SchemaCallback, SchemaCode, SchemaNode};
use serde_json::Value;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Build the author refinements supplied by a recorded schema.
pub(crate) fn code(schema: &Value) -> SchemaCode {
    let counts = Rc::new(RefCell::new(HashMap::<String, usize>::new()));
    let mut code = SchemaCode::new();
    let mut pending = vec![(String::new(), schema)];
    while let Some((pointer, node)) = pending.pop() {
        if let Some(effect) = node.get("x-effect") {
            code.insert(
                pointer.clone(),
                SchemaNode {
                    base: None,
                    refinements: vec![refinement(effect.clone(), Rc::clone(&counts))],
                },
            );
        }
        match node {
            Value::Object(object) => pending.extend(object.iter().map(|(key, child)| {
                (
                    format!("{pointer}/{}", maestro_json::escape_pointer_token(key)),
                    child,
                )
            })),
            Value::Array(array) => pending.extend(
                array
                    .iter()
                    .enumerate()
                    .map(|(i, child)| (format!("{pointer}/{i}"), child)),
            ),
            _ => {}
        }
    }
    code
}

/// Build one recorded check and its separate diagnostic factory.
fn refinement(effect: Value, counts: Rc<RefCell<HashMap<String, usize>>>) -> Refinement {
    let label = effect["label"].as_str().unwrap().to_owned();
    let message = format!("reject {label}");
    Refinement {
        check: SchemaCallback::new(move |value| {
            let count = {
                let mut counts = counts.borrow_mut();
                let count = counts.entry(label.clone()).or_default();
                *count += 1;
                *count
            };
            Ok(apply(&effect, value, count))
        }),
        error: SchemaCallback::new(move |_| Ok(message.clone())),
    }
}

/// Apply one author edit program to the live callback operand.
fn apply(effect: &Value, value: &mut Value, count: usize) -> bool {
    let Some(object) = value.as_object_mut() else {
        return effect["reject"] != true;
    };
    for (key, value) in effect["put"].as_object().unwrap() {
        let value = if effect["toggle"] == true {
            serde_json::json!(count)
        } else {
            value.clone()
        };
        object.insert(key.clone(), value);
    }
    for key in effect["remove"].as_array().unwrap() {
        object.shift_remove(key.as_str().unwrap());
    }
    effect["reject"] != true
}
