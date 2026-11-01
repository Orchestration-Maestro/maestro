//! Author programs corresponding to the recorded executable-schema inputs.

use maestro_models::{
    Base, DiagnosticErrorInfo, ParameterSchema, Refinement, SchemaCallback, SchemaCode, SchemaKind,
    SchemaNode,
};
use serde_json::{Value, json};
use std::collections::HashMap;

/// One authored node together with its nested annotation paths.
struct Program {
    /// Ordinary schema data.
    schema: Value,
    /// Declared conversion kinds.
    kinds: HashMap<String, SchemaKind>,
    /// Retained callbacks.
    code: SchemaCode,
}

impl Program {
    /// Declare one node's conversion kind.
    fn new(schema: Value, kind: SchemaKind) -> Self {
        Self {
            schema,
            kinds: HashMap::from([(String::new(), kind)]),
            code: SchemaCode::new(),
        }
    }

    /// Attach a nested program at its exact schema pointer.
    fn child(mut self, pointer: &str, child: Self) -> Self {
        *self.schema.pointer_mut(pointer).unwrap() = child.schema;
        self.kinds.extend(
            child
                .kinds
                .into_iter()
                .map(|(path, kind)| (format!("{pointer}{path}"), kind)),
        );
        self.code.extend(
            child
                .code
                .into_iter()
                .map(|(path, node)| (format!("{pointer}{path}"), node)),
        );
        self
    }

    /// Wrap a program as the required n member of an authored object.
    fn member(self) -> Self {
        Self::new(
            json!({"type":"object", "properties":{"n":{}}, "required":["n"]}),
            SchemaKind::Object,
        )
        .child("/properties/n", self)
    }

    /// Keep the executable declaration through the public schema owner.
    fn finish(self) -> ParameterSchema<SchemaCode> {
        ParameterSchema::<()>::authored(self.schema, self.kinds).with_code(self.code)
    }
}

/// An ordinary authored primitive.
fn number() -> Program {
    Program::new(json!({"type":"number"}), SchemaKind::Number)
}

/// An authored string primitive.
fn string() -> Program {
    Program::new(json!({"type":"string"}), SchemaKind::String)
}

/// A Base author that converts integer text before its positive-number check.
fn positive(mode: &str) -> Program {
    let mode = mode.to_owned();
    let convert_mode = mode.clone();
    let check_mode = mode.clone();
    let mut program = Program::new(json!({}), SchemaKind::Base);
    program.code.insert(
        String::new(),
        SchemaNode {
            base: Some(Base {
                convert: SchemaCallback::new(move |value| {
                    if convert_mode == "throw-convert" {
                        return Err(DiagnosticErrorInfo::error("conversion stopped"));
                    }
                    Ok(value
                        .as_str()
                        .and_then(|s| s.parse::<i64>().ok())
                        .map(Value::from))
                }),
                check: SchemaCallback::new(move |value| {
                    if check_mode == "throw-check" {
                        return Err(DiagnosticErrorInfo::error("check stopped"));
                    }
                    Ok(value.as_f64().is_some_and(|number| number > 0.0))
                }),
                errors: SchemaCallback::new(move |_| {
                    if mode == "throw-errors" {
                        return Err(DiagnosticErrorInfo::error("errors stopped"));
                    }
                    Ok(vec![json!({"message":"positive expected"})])
                }),
            }),
            refinements: Vec::new(),
        },
    );
    program
}

/// Refine a declaration with the source's positive, constant or throwing check.
fn refine(mut program: Program, mode: &str, message: &str) -> Program {
    let mode = mode.to_owned();
    let error_mode = mode.clone();
    let message = message.to_owned();
    program
        .code
        .entry(String::new())
        .or_insert_with(|| SchemaNode {
            base: None,
            refinements: Vec::new(),
        })
        .refinements
        .push(Refinement {
            check: SchemaCallback::new(move |value| {
                if mode == "throw-check" {
                    return Err(DiagnosticErrorInfo::error("check stopped"));
                }
                Ok(
                    mode == "always"
                        || (mode != "never" && value.as_f64().is_some_and(|n| n > 0.0)),
                )
            }),
            error: SchemaCallback::new(move |_| {
                if error_mode == "throw-error" {
                    return Err(DiagnosticErrorInfo::error("message stopped"));
                }
                Ok(message.clone())
            }),
        });
    program
}

/// Apply a source-authored mutation, separately from returning a replacement.
fn edit(
    value: &mut Value,
    operation: &str,
    key: &str,
) -> Result<Option<Value>, DiagnosticErrorInfo> {
    match operation {
        "replace" => return Ok(Some(json!({"n":5}))),
        "mutate" => value[key] = json!(if key == "n" { 5 } else { 7 }),
        "add" => {
            value.as_object_mut().unwrap().insert(
                if key == "n" { "added" } else { "extra" }.into(),
                json!(true),
            );
        }
        "delete" => {
            value.as_object_mut().unwrap().shift_remove(key);
        }
        "throw" => {
            value[key] = json!(9);
            return Err(DiagnosticErrorInfo::error("conversion stopped"));
        }
        _ => panic!("unknown edit"),
    }
    Ok(None)
}

/// An accepting Base whose conversion performs the selected object edit.
fn editing(operation: &str) -> Program {
    let operation = operation.to_owned();
    let mut program = Program::new(json!({}), SchemaKind::Base);
    program.code.insert(
        String::new(),
        SchemaNode {
            base: Some(Base {
                convert: SchemaCallback::new(move |value| edit(value, &operation, "n")),
                ..Base::default()
            }),
            refinements: Vec::new(),
        },
    );
    program
}

/// Compose a source-authored sequence, tuple, record or object with Base children.
fn container(id: &str) -> Program {
    match id {
        "base-array" | "base-wrap" | "nested-array-error" => Program::new(json!({"type":"array", "items":{}}), SchemaKind::Array).child("/items", positive("normal")),
        "base-tuple" => Program::new(json!({"type":"array", "items":[{},{}], "minItems":2, "maxItems":2}), SchemaKind::Tuple).child("/items/0", positive("normal")).child("/items/1", string()),
        "base-record" => Program::new(json!({"type":"object", "patternProperties":{"^.*$":{}}}), SchemaKind::Record).child("/patternProperties/^.*$", positive("normal")),
        "base-additional" => Program::new(json!({"type":"object", "properties":{"keep":{}}, "required":["keep"], "additionalProperties":{}}), SchemaKind::Object).child("/properties/keep", string()).child("/additionalProperties", positive("normal")),
        "base-additional-empty" => Program::new(json!({"type":"object", "properties":{}, "additionalProperties":{}}), SchemaKind::Object).child("/additionalProperties", positive("normal")),
        _ => panic!("unknown container"),
    }
}

/// Construct a callback program for a recorded input, never from its expected output.
pub(crate) fn parameters(id: &str) -> ParameterSchema<SchemaCode> {
    if let Some(parameters) = default_parameters(id) {
        return parameters;
    }
    let program = match id {
        "optional-base-missing" | "required-base-missing" => {
            let mut p = positive("normal").member();
            if id.starts_with("optional") {p.schema.as_object_mut().unwrap().shift_remove("required");}
            p
        }
        "throw-convert" | "throw-check" | "throw-errors" => positive(id).member(),
        "refine-prior-minimum" => {
            let mut p = number(); p.schema["minimum"] = json!(5); refine(p, "positive", "positive expected").member()
        }
        "refine-prior-type" => refine(number(), "positive", "positive expected").member(),
        "refine-throw-check" => refine(number(), "throw-check", "positive expected").member(),
        "refine-throw-error" => refine(number(), "throw-error", "positive expected").member(),
        "base-array" | "base-wrap" | "base-tuple" | "base-record" | "base-additional" | "base-additional-empty" | "nested-array-error" => container(id).member(),
        "base-ref" => {
            let mut target = positive("normal"); target.schema["$id"] = json!("X");
            Program::new(json!({"$defs":{"X":{}}, "$ref":"X"}), SchemaKind::Cyclic).child("/$defs/X", target).member()
        },
        "base-and-refine" | "base-and-refine-failure" => refine(positive("normal"), "positive", "positive expected").member(),
        "base-refine-distinct-failure" => refine(positive("normal"), "always", "positive expected").member(),
        "serialized-code" => return serde_json::from_value(serde_json::to_value(positive("normal").member().finish()).unwrap()).unwrap(),
        "escaped-member-error" => Program::new(json!({"type":"object","properties":{"a/b~c":{}},"required":["a/b~c"]}), SchemaKind::Object).child("/properties/a~1b~0c", positive("normal")),
        "root-base-scalar-replacement" | "root-base-scalar-admitted" => positive("normal"),
        "root-refine-error" => refine(number().member(), "never", "root rejected"),
        "refine-authored-string" | "refine-plain-string" => {
            let mut p = string(); if id == "refine-plain-string" {p.kinds.clear();}
            p.code.insert(String::new(), SchemaNode {base: None, refinements: vec![Refinement {
                check: SchemaCallback::new(|v| Ok(v == "null")),
                error: SchemaCallback::new(|_| Ok("expected null text".into())),
            }]}); p.member()
        }
        "plain-guard-without-base-kind" | "authored-guard-without-base-kind" => {
            let mut p = positive("normal"); p.schema = json!({"type":"number"});
            if id.starts_with("plain") {p.kinds.clear();} else {p.kinds.insert(String::new(), SchemaKind::Number);}
            p.member()
        }
        "convert-error-before-pattern-compile" => Program::new(json!({"type":"object","properties":{"n":{},"other":{"type":"string","pattern":"["}},"required":["n","other"]}), SchemaKind::Object).child("/properties/n", positive("throw-convert")),
        _ => return other_parameters(id),
    };
    program.finish()
}

/// Default Base and root refinement programs across every JSON kind.
fn default_parameters(id: &str) -> Option<ParameterSchema<SchemaCode>> {
    if id.starts_with("base-default-") || id.starts_with("root-base-default-") {
        let mut p = Program::new(json!({}), SchemaKind::Base);
        p.code.insert(
            String::new(),
            SchemaNode {
                base: Some(Base::default()),
                refinements: vec![],
            },
        );
        return Some(
            if id.starts_with("root-") {
                p
            } else {
                p.member()
            }
            .finish(),
        );
    }
    if id.starts_with("base-positive-") {
        return Some(positive("normal").member().finish());
    }
    if id.starts_with("root-refine-pass-") {
        return Some(
            refine(
                Program::new(json!({}), SchemaKind::Unknown),
                "always",
                "positive expected",
            )
            .finish(),
        );
    }
    if id.starts_with("root-refine-reject-") {
        return Some(
            refine(
                Program::new(json!({}), SchemaKind::Unknown),
                "never",
                "root rejected",
            )
            .finish(),
        );
    }
    None
}

/// Construct remaining composition and editing author programs.
fn other_parameters(id: &str) -> ParameterSchema<SchemaCode> {
    if let Some(op) = id.strip_prefix("root-") {
        return editing(op).finish();
    }
    if let Some(op) = id.strip_prefix("member-") {
        return Program::new(
            json!({"type":"object","properties":{"child":{}},"required":["child"]}),
            SchemaKind::Object,
        )
        .child("/properties/child", editing(op))
        .finish();
    }
    if let Some(parameters) = edited_parameters(id) {
        return parameters;
    }
    if id.starts_with("union-")
        || id == "refine-union-selection"
        || id == "valid-union-skips-error-factory"
    {
        return union(id).member().finish();
    }
    if id.starts_with("mixed-") {
        return mixed(id).finish();
    }
    if id == "refine-cyclic" {
        let child = refine(number(), "positive", "positive expected");
        let object = Program::new(
            json!({"$id":"X","type":"object","properties":{"value":{}},"required":["value"]}),
            SchemaKind::Object,
        )
        .child("/properties/value", child);
        return Program::new(json!({"$defs":{"X":{}},"$ref":"X"}), SchemaKind::Cyclic)
            .child("/$defs/X", object)
            .member()
            .finish();
    }
    if id == "refine-intersection" || id == "refine-nested-composition" {
        return intersection(id).member().finish();
    }
    if id == "refine-error-after-edit" || id.starts_with("error-before-later-edit-") {
        return diagnostic_edit(id).member().finish();
    }
    if id.starts_with("refine-") {
        return refine(number(), "positive", "positive expected")
            .member()
            .finish();
    }
    panic!("unknown executable program");
}

/// Check-phase editing callbacks using their source-selected method.
fn edited_parameters(id: &str) -> Option<ParameterSchema<SchemaCode>> {
    if let Some(op) = id.strip_prefix("refine-edit-") {
        let op = op.to_owned();
        let mut p = Program::new(
            json!({"type":"object","properties":{"value":{"type":"number"}},"required":["value"]}),
            SchemaKind::Object,
        );
        p.code.insert(
            String::new(),
            SchemaNode {
                base: None,
                refinements: vec![Refinement {
                    check: SchemaCallback::new(move |v| {
                        edit(v, &op, "value")?;
                        Ok(true)
                    }),
                    error: SchemaCallback::new(|_| Ok("Refine Error".into())),
                }],
            },
        );
        return Some(p.member().finish());
    }
    if let Some(op) = id.strip_prefix("base-check-edit-") {
        let op = op.to_owned();
        let mut p = positive("normal");
        p.code.get_mut("").unwrap().base = Some(Base {
            check: SchemaCallback::new(move |v| {
                edit(v, &op, "value")?;
                Ok(true)
            }),
            ..Base::default()
        });
        return Some(p.member().finish());
    }
    None
}

/// Authored union author programs, including an eagerly throwing later converter.
fn union(id: &str) -> Program {
    let (first, second) = match id {
        "union-first-matching" => (positive("normal"), string()),
        "refine-union-selection" => (refine(number(), "positive", "positive expected"), string()),
        "union-eager-later-throw" => {
            let mut first = positive("normal");
            first.code.get_mut("").unwrap().base = Some(Base {
                convert: SchemaCallback::new(|_| Ok(Some(json!(1)))),
                check: SchemaCallback::new(|v| Ok(v == 1)),
                ..Base::default()
            });
            let mut second = positive("normal");
            second.code.get_mut("").unwrap().base = Some(Base {
                convert: SchemaCallback::new(|_| {
                    Err(DiagnosticErrorInfo::error("later conversion stopped"))
                }),
                check: SchemaCallback::new(|_| Ok(false)),
                ..Base::default()
            });
            (first, second)
        }
        "valid-union-skips-error-factory" => {
            let mut first = refine(number(), "never", "unused");
            first.code.get_mut("").unwrap().refinements[0].error =
                SchemaCallback::new(|_| Err(DiagnosticErrorInfo::error("must not run")));
            (first, Program::new(json!({}), SchemaKind::Any))
        }
        _ => (
            positive("normal"),
            Program::new(json!({"type":"null"}), SchemaKind::Null),
        ),
    };
    Program::new(json!({"anyOf":[{},{}]}), SchemaKind::Union)
        .child("/anyOf/0", first)
        .child("/anyOf/1", second)
}

/// Authored and data-only siblings deliberately share JSON but not author intent.
fn mixed(id: &str) -> Program {
    let authored = match id {
        "mixed-number" => number(),
        "mixed-string" => string(),
        "mixed-array" => Program::new(json!({"type":"array","items":{}}), SchemaKind::Array)
            .child("/items", number()),
        "mixed-object" => Program::new(
            json!({"type":"object","properties":{"a":{}},"required":["a"]}),
            SchemaKind::Object,
        )
        .child("/properties/a", number()),
        _ => panic!("unknown sibling"),
    };
    let raw = authored.schema.clone();
    Program::new(json!({"type":"object","properties":{"code":{},"authored":{},"plain":raw},"required":["code","authored","plain"]}), SchemaKind::Object).child("/properties/code", positive("normal")).child("/properties/authored", authored)
}

/// Intersections preserve original callback node identities.
fn intersection(id: &str) -> Program {
    let (first, second) = if id == "refine-intersection" {
        let mut bound = number();
        bound.schema["minimum"] = json!(-5);
        (refine(number(), "positive", "positive expected"), bound)
    } else {
        (
            Program::new(
                json!({"type":"object","properties":{"x":{}},"required":["x"]}),
                SchemaKind::Object,
            )
            .child(
                "/properties/x",
                refine(number(), "positive", "positive expected"),
            ),
            Program::new(
                json!({"type":"object","properties":{"y":{}},"required":["y"]}),
                SchemaKind::Object,
            )
            .child("/properties/y", string()),
        )
    };
    Program::new(json!({"allOf":[{},{}]}), SchemaKind::Intersect)
        .child("/allOf/0", first)
        .child("/allOf/1", second)
}

/// Message factories observe values at their actual diagnostic phase.
fn diagnostic_edit(id: &str) -> Program {
    if id == "refine-error-after-edit" {
        let mut p = Program::new(
            json!({"type":"object","properties":{"value":{"type":"number"}},"required":["value"]}),
            SchemaKind::Object,
        );
        p.code.insert(
            String::new(),
            SchemaNode {
                base: None,
                refinements: vec![Refinement {
                    check: SchemaCallback::new(|v| {
                        v["added"] = json!(true);
                        Ok(false)
                    }),
                    error: SchemaCallback::new(|v| Ok(format!("added: {}", v["added"]))),
                }],
            },
        );
        return p;
    }
    let mut first = Program::new(json!({"type":"object","properties":{}}), SchemaKind::Object);
    first.code.insert(
        String::new(),
        SchemaNode {
            base: None,
            refinements: vec![Refinement {
                check: SchemaCallback::new(|_| Ok(false)),
                error: SchemaCallback::new(|v| {
                    Ok(if v["flag"] == true {
                        "B present"
                    } else {
                        "B absent"
                    }
                    .into())
                }),
            }],
        },
    );
    let mut second = Program::new(json!({"type":"object","properties":{}}), SchemaKind::Object);
    second.code.insert(
        String::new(),
        SchemaNode {
            base: None,
            refinements: vec![Refinement {
                check: SchemaCallback::new(|v| {
                    v["flag"] = json!(true);
                    Ok(true)
                }),
                error: SchemaCallback::new(|_| Ok("Refine Error".into())),
            }],
        },
    );
    Program::new(json!({"allOf":[{},{}]}), SchemaKind::Intersect)
        .child("/allOf/0", first)
        .child("/allOf/1", second)
}
