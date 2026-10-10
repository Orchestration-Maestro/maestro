//! Tool record and exact-name observations.
use crate::{
    BashToolCallEvent, BashToolResultEvent, CustomToolCallEvent, CustomToolResultEvent,
    EditToolCallEvent, EditToolResultEvent, FindToolCallEvent, FindToolResultEvent,
    GrepToolCallEvent, GrepToolResultEvent, LsToolCallEvent, LsToolResultEvent, Presence,
    ReadToolCallEvent, ReadToolResultEvent, ToolCallEvent, ToolResultEvent, WriteToolCallEvent,
    WriteToolResultEvent, is_bash_tool_result, is_edit_tool_result, is_find_tool_result,
    is_grep_tool_result, is_ls_tool_result, is_read_tool_result, is_tool_call_event_type,
    is_write_tool_result,
};
use serde_json::Value;

#[test]
fn maestro_tool_name_checks_are_exact() {
    let rows: Vec<Value> = serde_json::from_str(include_str!("tool_names.json")).unwrap();
    assert_eq!(rows.len(), 376);
    for row in rows {
        let name = row["name"].as_str().unwrap();
        let call: CustomToolCallEvent = ToolCallEvent {
            tool_call_id: "id".into(),
            tool_name: name.into(),
            input: if row["decoy"] == true {
                "call decoy"
            } else {
                "{}"
            }
            .into(),
        };
        let result: CustomToolResultEvent = ToolResultEvent {
            tool_call_id: "id".into(),
            tool_name: name.into(),
            input: if row["decoy"] == true {
                "call decoy"
            } else {
                "{}"
            }
            .into(),
            content: vec![],
            is_error: false,
            details: if row["decoy"] == true {
                Presence::Present("irrelevant payload".into())
            } else {
                Presence::Missing
            },
        };
        let observed = if row.get("query").is_some() {
            is_tool_call_event_type(row["query"].as_str().unwrap(), &call)
        } else {
            match row["helper"].as_str().unwrap() {
                "isBashToolResult" => is_bash_tool_result(&result),
                "isReadToolResult" => is_read_tool_result(&result),
                "isEditToolResult" => is_edit_tool_result(&result),
                "isWriteToolResult" => is_write_tool_result(&result),
                "isGrepToolResult" => is_grep_tool_result(&result),
                "isFindToolResult" => is_find_tool_result(&result),
                "isLsToolResult" => is_ls_tool_result(&result),
                _ => is_tool_call_event_type(row["query"].as_str().unwrap(), &call),
            }
        };
        assert_eq!(observed, row["expected"].as_bool().unwrap(), "{row}");
    }
}

/// Constructs every source-named call alias rather than a neighbouring variant.
fn assert_call_alias(event: &Value) {
    macro_rules! alias {
        ($alias:ty) => {{
            let value: $alias = serde_json::from_value(event.clone()).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), {
                let mut plain = event.clone();
                plain.as_object_mut().unwrap().remove("type");
                plain
            });
        }};
    }
    match event["toolName"].as_str().unwrap() {
        "bash" => alias!(BashToolCallEvent),
        "read" => alias!(ReadToolCallEvent),
        "edit" => alias!(EditToolCallEvent),
        "write" => alias!(WriteToolCallEvent),
        "grep" => alias!(GrepToolCallEvent),
        "find" => alias!(FindToolCallEvent),
        "ls" => alias!(LsToolCallEvent),
        _ => alias!(CustomToolCallEvent),
    }
}

/// Constructs every source-named result alias and its actual details record.
fn assert_result_alias(event: &Value) {
    macro_rules! alias {
        ($alias:ty) => {{
            let value: $alias = serde_json::from_value(event.clone()).unwrap();
            let mut plain = event.clone();
            plain.as_object_mut().unwrap().remove("type");
            assert_eq!(serde_json::to_value(value).unwrap(), plain);
        }};
    }
    match event["toolName"].as_str().unwrap() {
        "bash" => alias!(BashToolResultEvent),
        "read" => alias!(ReadToolResultEvent),
        "edit" => alias!(EditToolResultEvent),
        "write" => alias!(WriteToolResultEvent),
        "grep" => alias!(GrepToolResultEvent),
        "find" => alias!(FindToolResultEvent),
        "ls" => alias!(LsToolResultEvent),
        _ => alias!(CustomToolResultEvent),
    }
}

/// The recorded built-in input and detail classes cross both adapters.
async fn builtin_records(driver: &mut impl super::scenario::Driver) -> Result<(), String> {
    use super::documents::{Answer, ask};
    let rows: Vec<Value> = serde_json::from_str(include_str!("tool_records.json")).unwrap();
    assert_eq!(rows.len(), 161);
    for row in rows {
        let event = &row["event"];
        if event["type"] == "tool_call" {
            assert_call_alias(event);
            assert_eq!(
                ask(
                    driver,
                    event,
                    &serde_json::json!({"sessionEdit":"tool extra"})
                )
                .await?,
                Answer::returned(&row["edited"], None)
            );
        } else {
            assert_result_alias(event);
            assert_eq!(
                ask(driver, event, &serde_json::json!({})).await?,
                Answer::returned(&row["edited"], None)
            );
        }
    }
    Ok(())
}

#[test]
fn maestro_tool_events_keep_builtin_inputs_and_custom_extras() -> Result<(), String> {
    on_both_adapters!(builtin_records)
}

/// Overrides retain details and parameters independently of the registered name.
async fn override_payloads(driver: &mut impl super::scenario::Driver) -> Result<(), String> {
    use super::documents::{Answer, ask};
    use serde_json::json;
    for name in ["bash", "read", "edit", "write", "grep", "find", "ls"] {
        for details in [
            json!({"blocked":true}),
            json!({"lines":12,"error":"denied"}),
            json!(["replacement", false, 3]),
        ] {
            let event = json!({"type":"tool_result","toolCallId":"c","toolName":name,"input":"{}","content":[],"isError":false,"details":details});
            assert_eq!(
                ask(driver, &event, &json!({})).await?,
                Answer::returned(&event, None),
                "{name}"
            );
        }
    }
    Ok(())
}

#[test]
fn maestro_named_overrides_keep_arbitrary_details() -> Result<(), String> {
    on_both_adapters!(override_payloads)
}

/// An override need not use the original tool's parameter schema.
async fn override_inputs(driver: &mut impl super::scenario::Driver) -> Result<(), String> {
    use super::documents::{Answer, ask};
    use serde_json::json;
    for name in ["bash", "read", "edit", "write", "grep", "find", "ls"] {
        let event =
            json!({"type":"tool_call","toolCallId":"c","toolName":name,"input":{"override":true}});
        assert_eq!(
            ask(driver, &event, &json!({})).await?,
            Answer::returned(&event, None),
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn maestro_named_overrides_keep_arbitrary_inputs() -> Result<(), String> {
    on_both_adapters!(override_inputs)
}

/// Per-handler answers retain absent and falsy fields without applying host reduction.
async fn event_answers(driver: &mut impl super::scenario::Driver) -> Result<(), String> {
    use super::documents::{Answer, ask, returns};
    use serde_json::json;
    let rows: Vec<Value> = serde_json::from_str(include_str!("tool_answers.json")).unwrap();
    assert_eq!(rows.len(), 33);
    for row in rows {
        let family = row["family"].as_str().unwrap();
        let event = if family == "tool_call" {
            json!({"type":family,"toolName":"custom","toolCallId":"c","input":"{}"})
        } else {
            json!({"type":family,"toolName":"custom","toolCallId":"c","input":"{}","content":[],"isError":true})
        };
        let result = row.get("result");
        let directive = result.map_or_else(|| json!({}), |r| returns(family, r));
        assert_eq!(
            ask(driver, &event, &directive).await?,
            Answer::returned(&event, result.cloned()),
            "{row}"
        );
    }
    Ok(())
}

#[test]
fn maestro_event_results_keep_absent_and_falsy_replacements() -> Result<(), String> {
    on_both_adapters!(event_answers)
}

/// Opaque strings remain byte-identical and ordered maps expose their real key sequence.
async fn custom_json(driver: &mut impl super::scenario::Driver) -> Result<(), String> {
    use super::documents::{Answer, ask};
    use serde_json::json;
    let rows: Vec<Value> = serde_json::from_str(include_str!("tool_custom.json")).unwrap();
    assert_eq!(rows.len(), 8);
    for row in rows {
        let event =
            json!({"type":"tool_call","toolName":"custom","toolCallId":"c","input":row["input"]});
        let mut edited = event.clone();
        edited["input"] = row["edited"].clone();
        assert_eq!(
            ask(driver, &event, &json!({"sessionEdit":"tool custom"})).await?,
            Answer::returned(&edited, None)
        );
        let inner: Value = serde_json::from_str(row["edited"].as_str().unwrap()).unwrap();
        assert_eq!(
            inner
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["z", "a"]
        );
    }
    let opaque = " {\"z\":\"Ω😀\\u0061\", \"a\":false, \"z\":0} ";
    let call = json!({"type":"tool_call","toolName":"custom","toolCallId":"c","input":opaque});
    assert_call_alias(&call);
    assert_eq!(
        ask(driver, &call, &json!({})).await?,
        Answer::returned(&call, None)
    );
    let result = json!({"type":"tool_result","toolName":"custom","toolCallId":"c","input":opaque,"content":[
        {"type":"text","text":"z","textSignature":"sig-Ω"},
        {"type":"image","data":"AQI=","mimeType":"image/png"},
        {"type":"text","text":"a","textSignature":"other"},
        {"type":"text","text":"z","textSignature":"sig-Ω"}
    ],"isError":false,"details":opaque});
    assert_result_alias(&result);
    assert_eq!(
        ask(driver, &result, &json!({})).await?,
        Answer::returned(&result, None)
    );
    let handler = driver.identity("event probe")?;
    let raw = r#"{"type":"tool_call","toolName":"bash","toolCallId":"c","input":{"command":"Ω😀","z":1,"\u0061":false,"a":true,"middle":0}}"#;
    let delivered = driver.deliver(handler, raw, "{}", None).await?;
    let event: Value = serde_json::from_str(&delivered.event?.unwrap()).unwrap();
    assert_eq!(
        event["input"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["command", "z", "a", "middle"]
    );
    assert_eq!(event["input"]["a"], true);
    assert_eq!(event["input"]["command"], "Ω😀");
    Ok(())
}

#[test]
fn maestro_custom_json_keeps_order_and_valid_unicode() -> Result<(), String> {
    on_both_adapters!(custom_json)
}

/// User shell data is supplied output, not execution policy.
async fn user_bash(driver: &mut impl super::scenario::Driver) -> Result<(), String> {
    use super::documents::{Answer, ask, returns};
    let rows: Vec<Value> = serde_json::from_str(include_str!("tool_shell.json")).unwrap();
    assert_eq!(rows.len(), 26);
    for row in rows {
        let result = row.get("result");
        let directive = result.map_or_else(|| serde_json::json!({}), |r| returns("user_bash", r));
        assert_eq!(
            ask(driver, &row["event"], &directive).await?,
            Answer::returned(&row["event"], result.cloned())
        );
    }
    Ok(())
}

#[test]
fn maestro_user_bash_results_preserve_presence_and_flags() -> Result<(), String> {
    on_both_adapters!(user_bash)
}

/// Supplied truncation accounting with independently observable fields.
fn truncation() -> Value {
    serde_json::json!({"content":"body","truncated":true,"truncatedBy":"bytes","totalLines":1.0,"totalBytes":2.0,"outputLines":3.0,"outputBytes":4.0,"lastLinePartial":true,"firstLineExceedsLimit":false,"maxLines":5.0,"maxBytes":6.0})
}

/// Numeric fields at the typed author boundary, with their selected event document.
fn numeric_events() -> Vec<(Value, &'static str, Vec<&'static str>)> {
    use serde_json::json;
    let mut cases = vec![];
    for (name, input, fields) in [
        ("bash", json!({"command":"x"}), vec!["timeout"]),
        ("read", json!({"path":"p"}), vec!["offset", "limit"]),
        ("grep", json!({"pattern":"p"}), vec!["context", "limit"]),
        ("find", json!({"pattern":"p"}), vec!["limit"]),
        ("ls", json!({}), vec!["limit"]),
    ] {
        for field in fields {
            cases.push((
                json!({"type":"tool_call","toolName":name,"toolCallId":"c","input":input}),
                field,
                vec!["input", field],
            ));
        }
    }
    for (name, details, field) in [
        ("edit", json!({"diff":"d"}), "firstChangedLine"),
        ("grep", json!({}), "matchLimitReached"),
        ("find", json!({}), "resultLimitReached"),
        ("ls", json!({}), "entryLimitReached"),
    ] {
        cases.push((json!({"type":"tool_result","toolName":name,"toolCallId":"c","input":"{}","content":[],"isError":false,"details":details}),field,vec!["details",field]));
    }
    for name in ["bash", "read", "grep", "find", "ls"] {
        for field in [
            "totalLines",
            "totalBytes",
            "outputLines",
            "outputBytes",
            "maxLines",
            "maxBytes",
        ] {
            cases.push((json!({"type":"tool_result","toolName":name,"toolCallId":"c","input":"{}","content":[],"isError":false,"details":{"truncation":truncation()}}),field,vec!["details","truncation",field]));
        }
    }
    cases
}

/// Reads the number at the independently specified wire location.
fn number_at<'a>(event: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(event, |value, key| &value[*key])
}

/// Finite values preserve their bits; nonfinite edits and answers fail independently.
async fn numeric_output(driver: &mut impl super::scenario::Driver) -> Result<(), String> {
    use super::scenario::Decision;
    use serde_json::json;
    let handler = driver.identity("event probe")?;
    let message = "extension wrote a non-finite number (Infinity or NaN)";
    for (event, field, path) in numeric_events() {
        for value in [0.0_f64, -0.0, 1.25, -1.25, f64::from_bits(1), f64::MAX] {
            let directive =
                json!({"numberBits":format!("{:016x}",value.to_bits()),"numberField":field});
            let answer = driver
                .deliver(handler, &event.to_string(), &directive.to_string(), None)
                .await?;
            let edited: Value = serde_json::from_str(&answer.event?.unwrap()).unwrap();
            assert_eq!(
                number_at(&edited, &path).as_f64().unwrap().to_bits(),
                value.to_bits(),
                "{field}"
            );
        }
        for value in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let directive = json!({"numberBits":format!("{:016x}",value.to_bits()),"numberField":field,"ending":{"returns":{"family":"tool_call","value":{}}}});
            let answer = driver
                .deliver(handler, &event.to_string(), &directive.to_string(), None)
                .await?;
            assert_eq!(
                serde_json::from_str::<Value>(&answer.event?.unwrap()).unwrap(),
                event
            );
            assert_eq!(answer.decision, Decision::Failed(message.into()));
        }
    }
    numeric_shell_result(driver).await
}

/// Shell result numbers use the same finite-output check independently of the event.
async fn numeric_shell_result(driver: &mut impl super::scenario::Driver) -> Result<(), String> {
    use super::scenario::Decision;
    use serde_json::json;
    let handler = driver.identity("event probe")?;
    let message = "extension wrote a non-finite number (Infinity or NaN)";
    let event = json!({"type":"user_bash","command":"x","cwd":"..","excludeFromContext":false});
    for value in [
        0.0_f64,
        -0.0,
        1.25,
        -1.25,
        f64::from_bits(1),
        f64::MAX,
        f64::INFINITY,
        f64::NAN,
    ] {
        let directive = json!({"resultNumberBits":format!("{:016x}",value.to_bits()),"ending":{"returns":{"family":"user_bash","value":{"result":{"output":"out","cancelled":false,"truncated":true}}}}});
        let answer = driver
            .deliver(handler, &event.to_string(), &directive.to_string(), None)
            .await?;
        assert_eq!(
            serde_json::from_str::<Value>(&answer.event?.unwrap()).unwrap(),
            event
        );
        if value.is_finite() {
            let Decision::Returned(Ok(Some(result))) = answer.decision else {
                panic!("no finite result")
            };
            assert_eq!(
                serde_json::from_str::<Value>(&result).unwrap()["result"]["exitCode"]
                    .as_f64()
                    .unwrap()
                    .to_bits(),
                value.to_bits()
            );
        } else {
            assert_eq!(answer.decision, Decision::Returned(Err(message.into())));
        }
    }
    Ok(())
}

#[test]
fn maestro_tool_numeric_fields_reject_nonfinite_output() -> Result<(), String> {
    on_both_adapters!(numeric_output)
}

/// Transport retains payload shapes without selecting a schema by name.
async fn retained_payloads(driver: &mut impl super::scenario::Driver) -> Result<(), String> {
    use super::documents::{Answer, ask};
    use serde_json::json;
    let read = json!({"type":"tool_call","toolCallId":"c","toolName":"read","input":{"path":"p","content":"write decoy","offset":-1.25,"new":"kept"}});
    assert_eq!(
        ask(driver, &read, &json!({})).await?,
        Answer::returned(&read, None)
    );
    let custom =
        json!({"type":"tool_call","toolCallId":"c","toolName":"read ","input":" {\"path\":42} "});
    assert_eq!(
        ask(driver, &custom, &json!({})).await?,
        Answer::returned(&custom, None)
    );
    let result = json!({"type":"tool_result","toolCallId":"c","toolName":"edit","input":"{}","content":[],"isError":false,"details":{"diff":"d","truncation":"wrong-owner","unread":{"nested":[1,2,3]}}});
    assert_eq!(
        ask(driver, &result, &json!({})).await?,
        Answer::returned(&result, None)
    );
    let positional = json!({"type":"tool_result","toolCallId":"c","toolName":"edit","input":"{}","content":[],"isError":false,"details":["d",2.5]});
    assert_eq!(
        ask(driver, &positional, &json!({})).await?,
        Answer::returned(&positional, None)
    );
    retained_literal_shapes(driver).await?;
    decoder_refusals(driver).await?;
    let handler = driver.identity("event probe")?;
    let deep = format!("{}0{}", "[".repeat(150), "]".repeat(150));
    let raw = format!(
        r#"{{"type":"tool_result","toolCallId":"c","toolName":"edit","input":"{{}}","content":[],"isError":false,"details":{{"diff":"d","unread":{deep}}},"unread":{deep}}}"#
    );
    let answer = driver.deliver(handler, &raw, "{}", None).await?;
    let edited = answer.event?.unwrap();
    assert!(edited.contains(&format!(r#""details":{{"diff":"d","unread":{deep}}}"#)));
    arbitrary_preparation(driver).await?;
    driver.release_all().await;
    Ok(())
}

#[test]
fn maestro_tool_decoding_keeps_payloads_without_schema_validation() -> Result<(), String> {
    on_both_adapters!(retained_payloads)
}

/// Invalid selected records fail before the synchronous producer can enter its handler.
async fn decoder_refusals(driver: &mut impl super::scenario::Driver) -> Result<(), String> {
    let handler = driver.identity("event probe")?;
    for raw in [
        r#"["tool_call","c","read",{"path":"p"}]"#,
        r#"{"type":"tool_call","toolCallId":"c","toolName":{},"input":"{}"}"#,
    ] {
        let entered = driver
            .transcript()
            .iter()
            .filter(|s| *s == "entry entered")
            .count();
        assert!(
            driver.deliver(handler, raw, "{}", None).await.is_err(),
            "{raw}"
        );
        assert_eq!(
            driver
                .transcript()
                .iter()
                .filter(|s| *s == "entry entered")
                .count(),
            entered
        );
    }
    Ok(())
}

/// Preparation forwards all native JSON value classes without tool-schema checking.
async fn arbitrary_preparation(driver: &mut impl super::scenario::Driver) -> Result<(), String> {
    use serde_json::json;
    let config = super::tool_callbacks::configuration();
    driver
        .run_plain_command("capture", &format!("tools {config}"))
        .await?;
    for value in [
        Value::Null,
        json!(false),
        json!(0),
        json!(""),
        json!([]),
        json!({}),
        json!(["Ω", false]),
        json!({"z":0,"a":1}),
    ] {
        let mut expected = value.clone();
        if expected.is_object() {
            expected["mark"] = json!("after");
        }
        let prepared = driver.prepare("echo", &value.to_string()).await?;
        assert_eq!(serde_json::from_str::<Value>(&prepared).unwrap(), expected);
    }
    Ok(())
}

/// Transport retains object-valued literals without decoding their enum views.
async fn retained_literal_shapes(driver: &mut impl super::scenario::Driver) -> Result<(), String> {
    use super::documents::{Answer, ask};
    use serde_json::json;
    for mode in ["lines", "bytes"] {
        let mut supplied = truncation();
        supplied["truncatedBy"] = json!({mode:null});
        let event = json!({"type":"tool_result","toolName":"bash","toolCallId":"c","input":"{}","content":[],"isError":false,"details":{"truncation":supplied}});
        assert_eq!(
            ask(driver, &event, &json!({})).await?,
            Answer::returned(&event, None)
        );
    }
    Ok(())
}
