mod support;
use maestro_models::*;
use serde_json::{Value, json};

fn tool(schema: Value) -> ToolDeclaration {
    ToolDeclaration {
        name: "lookup".into(),
        description: "Look up data".into(),
        parameters: schema,
    }
}
fn call(arguments: Value) -> ToolCall {
    ToolCall::new(
        "call-id".into(),
        "lookup".into(),
        arguments.as_object().unwrap().clone(),
        Some("metadata".into()),
    )
}
fn validate(schema: Value, arguments: Value) -> Result<Value, ToolValidationError> {
    validate_tool_call(&[tool(schema)], &call(arguments)).map(Value::Object)
}

#[test]
fn valid_tool_arguments_return_owned_objects() {
    for (schema, arguments) in [
        (json!({"type":"object"}), json!({})),
        (
            json!({"type":"object","required":["x"],"properties":{"x":{"type":"integer","minimum":1,"maximum":3},"optional":{"enum":["a","b"]}},"additionalProperties":false}),
            json!({"x":2,"optional":"b"}),
        ),
        (
            json!({"type":"object","$defs":{"item":{"type":"object","properties":{"items":{"type":"array","items":{"type":"integer"}}},"required":["items"]}},"properties":{"nested":{"$ref":"#/$defs/item"}},"required":["nested"]}),
            json!({"nested":{"items":[1,2]}}),
        ),
    ] {
        let tools = vec![tool(schema)];
        let completed = call(arguments.clone());
        assert_eq!(completed.id, "call-id");
        assert_eq!(completed.name, "lookup");
        assert_eq!(completed.replay_metadata.as_deref(), Some("metadata"));
        assert_eq!(completed.arguments(), arguments.as_object());
        let mut result = validate_tool_call(&tools, &completed).unwrap();
        assert_eq!(Value::Object(result.clone()), arguments);
        result.insert("changed".into(), json!(true));
        assert_eq!(completed.arguments(), arguments.as_object());
    }
    assert_eq!(
        call(json!({"not-validated":true})).arguments(),
        json!({"not-validated":true}).as_object()
    );
}

#[test]
fn supported_primitive_coercions_are_exact() {
    let cases = [
        (json!("number"), json!(" 2.5 "), json!(2.5)),
        (json!("number"), json!("1e2"), json!(100)),
        (json!("integer"), json!("2.0"), json!(2)),
        (json!("integer"), json!("0x10"), json!(16)),
        (json!("number"), json!("0o10"), json!(8)),
        (json!("number"), json!("0b10"), json!(2)),
        (json!("number"), json!(".5"), json!(0.5)),
        (json!("number"), json!("+1."), json!(1)),
        (json!("integer"), json!(true), json!(1)),
        (json!("number"), json!(false), json!(0)),
        (json!("integer"), Value::Null, json!(0)),
        (json!("number"), Value::Null, json!(0)),
        (json!("boolean"), json!("true"), json!(true)),
        (json!("boolean"), json!("false"), json!(false)),
        (json!("boolean"), json!(1), json!(true)),
        (json!("boolean"), json!(0), json!(false)),
        (json!("boolean"), Value::Null, json!(false)),
        (json!("string"), json!(3), json!("3")),
        (
            json!("string"),
            json!(9007199254740993_u64),
            json!("9007199254740992"),
        ),
        (json!("string"), json!(2.5), json!("2.5")),
        (json!("string"), json!(true), json!("true")),
        (json!("string"), json!(false), json!("false")),
        (json!("string"), Value::Null, json!("")),
        (json!("null"), json!(""), Value::Null),
        (json!("null"), json!(0), Value::Null),
        (json!("null"), json!(false), Value::Null),
        (json!(["integer", "string"]), json!("42"), json!("42")),
        (json!(["string", "integer"]), Value::Null, json!("")),
        (json!(["integer", "string"]), Value::Null, json!(0)),
        (json!(["object", "boolean"]), json!("true"), json!(true)),
        (json!(["integer", "number"]), json!(2.5), json!(2.5)),
    ];
    for (kind, value, expected) in cases {
        let schema = json!({"type":"object","properties":{"x":{"type":kind}},"required":["x"]});
        assert_eq!(
            validate(schema, json!({"x":value})),
            Ok(json!({"x":expected})),
            "target {kind}, source {value}"
        );
    }
}

#[test]
fn nested_and_union_coercion_uses_schema_order() {
    let schema = json!({"$schema":"http://json-schema.org/draft-07/schema#","type":"object","properties":{
        "nested":{"type":"object","properties":{"x":{"type":"integer"}},"additionalProperties":{"type":"boolean"}},
        "list":{"type":"array","items":{"type":"number"}},
        "tuple":{"type":"array","items":[{"type":"integer"},{"type":"boolean"}]},
        "all":{"allOf":[{"type":"integer"},{"minimum":1}]},
        "any":{"anyOf":[{"type":"integer","minimum":5},{"type":"boolean"}]},
        "one":{"oneOf":[{"type":"integer"},{"type":"boolean"}]},
        "branch":{"anyOf":[{"type":"object","properties":{"x":{"type":"integer"}},"required":["missing"]},{"type":"object","properties":{"x":{"type":"boolean"}},"required":["x"]}]}
    }});
    let input = json!({"nested":{"x":"3","extra":"false"},"list":["2.5",null],"tuple":["4","true"],"all":"2","any":null,"one":"true","branch":{"x":null}});
    assert_eq!(
        validate(schema, input),
        Ok(
            json!({"nested":{"x":3,"extra":false},"list":[2.5,0],"tuple":[4,true],"all":2,"any":false,"one":true,"branch":{"x":false}})
        )
    );
    let ordered = json!({"type":"object","properties":{"x":{"anyOf":[{"type":"integer"},{"type":"boolean"}]}}});
    assert_eq!(validate(ordered, json!({"x":null})), Ok(json!({"x":0})));
    let ambiguous = json!({"type":"object","properties":{"x":{"oneOf":[{"type":"integer"},{"type":"number"}]}}});
    assert_eq!(
        validate(ambiguous, json!({"x":"2"})),
        Err(ToolValidationError::InvalidArguments)
    );
    let absent_container =
        json!({"type":"object","properties":{"x":{"properties":{"n":{"type":"integer"}}}}});
    assert_eq!(
        validate(absent_container, json!({"x":{"n":"2"}})),
        Err(ToolValidationError::InvalidArguments)
    );
    let tuple_branch = json!({"$schema":"http://json-schema.org/draft-07/schema#","type":"object","properties":{"x":{"anyOf":[{"type":"array","items":[{"type":"integer"},{"type":"boolean"}]}]}}});
    assert_eq!(
        validate(tuple_branch, json!({"x":["2","true"]})),
        Ok(json!({"x":[2,true]}))
    );
    for definitions in ["$defs", "definitions"] {
        let referenced = json!({"type":"object",definitions:{"range":{"minimum":5}},"properties":{"x":{"anyOf":[{"type":"integer","$ref":format!("#/{definitions}/range")},{"type":"boolean"}]}}});
        assert_eq!(
            validate(referenced.clone(), json!({"x":null})),
            Ok(json!({"x":false}))
        );
        assert_eq!(validate(referenced, json!({"x":"6"})), Ok(json!({"x":6})));
    }
    let root_relative = json!({"type":"object","properties":{"bounds":{"minimum":5},"a/b~é":{"anyOf":[{"type":"integer","$ref":"#/properties/bounds"},{"type":"boolean"}]}}});
    assert_eq!(
        validate(root_relative, json!({"a/b~é":null})),
        Ok(json!({"a/b~é":false}))
    );
    let no_defaults = json!({"type":"object","properties":{"x":{"type":"integer","default":2}}});
    assert_eq!(validate(no_defaults, json!({})), Ok(json!({})));
}

#[test]
fn union_branches_resolve_references_in_nested_resources() {
    let schema = json!({"$id":"https://example.invalid/root","type":"object","properties":{"x":{"$id":"child","$defs":{"limit":{"minimum":5}},"anyOf":[{"type":"integer","$ref":"#/$defs/limit"},{"type":"boolean"}]}}});
    assert_eq!(
        validate(schema.clone(), json!({"x":null})),
        Ok(json!({"x":false}))
    );
    assert_eq!(validate(schema, json!({"x":"7"})), Ok(json!({"x":7})));
}

#[test]
fn invalid_tool_arguments_and_schemas_fail_safely() {
    let secret = "RAW_SECRET_SENTINEL";
    assert_eq!(
        validate_tool_call(&[], &call(json!({"x":secret}))),
        Err(ToolValidationError::UnknownTool)
    );
    let events = support::conformance::run(vec![
        support::conformance::tool_start(0),
        ProviderUpdate::Error {
            failure: Failure::Transport,
        },
    ]);
    let AssistantContent::ToolCall(partial) = &support::conformance::terminal(&events).content[0]
    else {
        panic!()
    };
    assert_eq!(
        validate_tool_call(&[tool(json!({}))], partial),
        Err(ToolValidationError::IncompleteArguments)
    );
    for schema in [
        json!({"type":"invalid"}),
        json!({"type":42}),
        json!({"required":true}),
        json!({"$ref":"https://invalid.example/schema"}),
        json!({"$schema":"https://invalid.example/dialect"}),
        json!({"type":"object","properties":{"x":{"type":"integer","minimum":"bad"}}}),
    ] {
        assert_eq!(
            validate(schema, json!({"x":secret})),
            Err(ToolValidationError::InvalidSchema)
        );
    }
    for (kind, value) in [
        ("boolean", json!("1")),
        ("boolean", json!("0")),
        ("boolean", json!("TRUE")),
        ("null", json!("null")),
        ("integer", json!("2.5")),
        ("number", json!("  \n")),
        ("number", json!("NaN")),
        ("number", json!("Infinity")),
        ("number", json!("1e9999")),
        ("number", json!("0x")),
        ("number", json!("+0x10")),
        ("string", json!({})),
        ("number", json!([1])),
    ] {
        assert_eq!(
            validate(
                json!({"type":"object","properties":{"x":{"type":kind}}}),
                json!({"x":value})
            ),
            Err(ToolValidationError::InvalidArguments),
            "{kind}: {value}"
        );
    }
    for (schema, args) in [
        (json!({"required":["x"]}), json!({})),
        (
            json!({"additionalProperties":false}),
            json!({"extra":secret}),
        ),
        (json!({"properties":{"x":{"minimum":3}}}), json!({"x":2})),
        (json!({"properties":{"x":{"maximum":1}}}), json!({"x":2})),
        (
            json!({"properties":{"x":{"enum":["known"]}}}),
            json!({"x":secret}),
        ),
    ] {
        assert_eq!(
            validate(schema, args),
            Err(ToolValidationError::InvalidArguments)
        );
    }
    for (error, display) in [
        (ToolValidationError::UnknownTool, "unknown tool declaration"),
        (
            ToolValidationError::IncompleteArguments,
            "incomplete tool arguments",
        ),
        (
            ToolValidationError::InvalidSchema,
            "invalid or unresolvable tool schema",
        ),
        (
            ToolValidationError::InvalidArguments,
            "invalid tool arguments",
        ),
    ] {
        assert_eq!(error.to_string(), display);
        assert!(!format!("{error:?}").contains(secret));
        let _: &dyn std::error::Error = &error;
    }
}

#[test]
fn validation_never_mutates_or_executes() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let counter = AtomicUsize::new(0);
    let execute = || {
        counter.fetch_add(1, Ordering::SeqCst);
    };
    let tools = vec![tool(
        json!({"type":"object","properties":{"nested":{"type":"object","properties":{"x":{"type":"integer"}}}},"required":["nested"]}),
    )];
    let before = tools.clone();
    for (arguments, expected) in [
        (json!({"nested":{"x":2}}), Ok(json!({"nested":{"x":2}}))),
        (json!({"nested":{"x":"2"}}), Ok(json!({"nested":{"x":2}}))),
        (
            json!({"nested":{"x":"invalid"}}),
            Err(ToolValidationError::InvalidArguments),
        ),
    ] {
        let call = call(arguments);
        let original = call.clone();
        let result = validate_tool_call(&tools, &call);
        assert_eq!(result.clone().map(Value::Object), expected);
        if let Ok(mut object) = result {
            object["nested"]["x"] = json!(999);
        }
        assert_eq!(call, original);
        assert_eq!(tools, before);
    }
    let fake = Arc::new(ScriptedProvider::new(vec![support::conformance::steps(
        vec![support::conformance::done()],
    )]));
    let context = Context {
        tools: tools.clone(),
        messages: vec![Message::Assistant(AssistantMessage {
            content: vec![AssistantContent::ToolCall(call(json!({"invalid":true})))],
            ..{
                let events = support::conformance::run(vec![support::conformance::done()]);
                support::conformance::terminal(&events).clone()
            }
        })],
        ..support::conformance::context()
    };
    support::block_on(support::conformance::registry(fake.clone()).complete(
        support::conformance::model(),
        context,
        support::auth::local(),
    ));
    assert_eq!(fake.calls().len(), 1);
    let Message::Assistant(a) = &fake.calls()[0].context.messages[0] else {
        panic!()
    };
    let AssistantContent::ToolCall(c) = &a.content[0] else {
        panic!()
    };
    assert_eq!(c.arguments(), json!({"invalid":true}).as_object());
    assert_eq!(counter.load(Ordering::SeqCst), 0);
    execute();
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn remote_schema_references_fail_without_io() {
    use std::{
        io::Write,
        net::{SocketAddr, TcpListener, TcpStream},
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
            mpsc,
        },
        thread,
    };
    struct Responder {
        address: SocketAddr,
        stop: mpsc::Sender<()>,
        thread: Option<thread::JoinHandle<()>>,
        requests: Arc<AtomicUsize>,
    }
    impl Drop for Responder {
        fn drop(&mut self) {
            self.stop.send(()).unwrap();
            let _signal = TcpStream::connect(self.address).unwrap();
            self.thread.take().unwrap().join().unwrap();
        }
    }
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let count = requests.clone();
    let (stop, stopped) = mpsc::channel();
    let responder = Responder {
        address,
        stop,
        requests,
        thread: Some(thread::spawn(move || {
            loop {
                let (mut connection, _) = listener.accept().unwrap();
                if stopped.try_recv().is_ok() {
                    break;
                }
                count.fetch_add(1, Ordering::SeqCst);
                let body = "{\"type\":\"object\"}";
                write!(connection,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        })),
    };
    let outcome = validate(
        json!({"$ref":format!("http://{address}/schema")}),
        json!({}),
    );
    let count = responder.requests.clone();
    drop(responder);
    assert_eq!(outcome, Err(ToolValidationError::InvalidSchema));
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert_eq!(
        validate(json!({"$ref":"file:///unavailable-schema.json"}), json!({})),
        Err(ToolValidationError::InvalidSchema)
    );
}

#[test]
fn number_to_string_coercion_uses_standard_spelling() {
    let cases = [
        (json!(0), "0"),
        (json!(-0.0), "0"),
        (json!(42), "42"),
        (json!(-42), "-42"),
        (json!(2.5), "2.5"),
        (json!(-2.5), "-2.5"),
        (json!(1e-7), "1e-7"),
        (json!(-1e-7), "-1e-7"),
        (json!(1e-6), "0.000001"),
        (json!(-1e-6), "-0.000001"),
        (json!(1e20), "100000000000000000000"),
        (json!(-1e20), "-100000000000000000000"),
        (json!(1e21), "1e+21"),
        (json!(-1e21), "-1e+21"),
        (json!(1e23), "1e+23"),
        (json!(f64::from_bits(1)), "5e-324"),
        (json!(f64::MAX), "1.7976931348623157e+308"),
        (json!(1.2345678901234567), "1.2345678901234567"),
        (json!(9007199254740993_u64), "9007199254740992"),
        (json!(1000000000000000128_u64), "1000000000000000100"),
    ];
    for (value, expected) in cases {
        let schema = json!({"type":"object","properties":{"x":{"type":"string"}},"required":["x"]});
        let tools = vec![tool(schema.clone())];
        let completed = call(json!({"x":value}));
        let before = completed.clone();
        let declarations = tools.clone();
        assert_eq!(
            validate_tool_call(&tools, &completed).map(Value::Object),
            Ok(json!({"x":expected})),
            "{value}"
        );
        assert_eq!(completed, before);
        assert_eq!(tools, declarations);
        assert_eq!(
            validate(
                json!({"type":"object","properties":{"x":{"type":"string","enum":[expected]}}}),
                json!({"x":value})
            ),
            Ok(json!({"x":expected}))
        );
        let previous = if value.as_number().unwrap().is_f64() {
            value.as_f64().unwrap().to_string()
        } else {
            value.to_string()
        };
        if previous != expected {
            assert_eq!(
                validate(
                    json!({"type":"object","properties":{"x":{"type":"string","enum":[previous]}}}),
                    json!({"x":value})
                ),
                Err(ToolValidationError::InvalidArguments)
            );
        }
        assert_eq!(
            validate(
                json!({"type":"object","properties":{"x":{"type":"number"}}}),
                json!({"x":value})
            ),
            Ok(json!({"x":value}))
        );
    }
}

#[test]
fn whitespace_follows_the_standard_set() {
    let whitespace = "\u{0009}\u{000a}\u{000b}\u{000c}\u{000d}\u{0020}\u{00a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";
    for kind in ["number", "integer"] {
        let schema = json!({"type":"object","properties":{"x":{"type":kind}}});
        for c in whitespace.chars() {
            assert_eq!(
                validate(schema.clone(), json!({"x":format!("{c}42{c}")})),
                Ok(json!({"x":42})),
                "{kind} U+{:04X}",
                c as u32
            );
            assert_eq!(
                validate(schema.clone(), json!({"x":c.to_string()})),
                Err(ToolValidationError::InvalidArguments)
            );
            assert_eq!(
                validate(schema.clone(), json!({"x":format!("4{c}2")})),
                Err(ToolValidationError::InvalidArguments)
            );
        }
        for c in "\u{0085}\u{180e}\u{200b}".chars() {
            assert_eq!(
                validate(schema.clone(), json!({"x":format!("{c}42{c}")})),
                Err(ToolValidationError::InvalidArguments)
            );
        }
        let expected = if kind == "number" {
            Ok(json!({"x":42.5}))
        } else {
            Err(ToolValidationError::InvalidArguments)
        };
        assert_eq!(
            validate(schema, json!({"x":"\u{feff}42.5\u{feff}"})),
            expected
        );
    }
}
