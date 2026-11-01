//! Writable setup capabilities exercised at the author boundary.
use super::scenario::Driver;
use serde::Deserialize;
use serde_json::{Value, json};

/// Source-derived literal writer input and its independent observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Attribution without printing input payloads.
    name: String,
    /// Authored operation and injected host reply.
    input: Value,
    /// Expected wire operation.
    method: String,
    /// Literal wire operands.
    args: Value,
    /// Expected typed reply.
    answer: Value,
}
/// Invoke one author operation and isolate its resulting observations.
async fn ask(driver: &mut impl Driver, input: &Value) -> Result<(Value, Vec<String>), String> {
    let start = driver.transcript().len();
    driver
        .run_plain_command("setup", &input.to_string())
        .await?;
    let lines = driver.transcript()[start..].to_vec();
    let answer = lines
        .iter()
        .find_map(|l| l.strip_prefix("entry setup-answer "))
        .map_or(Value::Null, |s| serde_json::from_str(s).unwrap());
    Ok((answer, lines))
}
/// Every source payload case retains literal operands and the typed host reply.
async fn forwarding(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", "setup-register")
        .await?;
    let cases: Vec<Case> = serde_json::from_str(include_str!("setup_corpus.json")).unwrap();
    assert_eq!(cases.len(), 103);
    for case in cases {
        let (answer, lines) = ask(driver, &case.input).await?;
        assert_eq!(answer, case.answer, "{}", case.name);
        let calls: Vec<Value> = lines
            .iter()
            .filter_map(|l| l.strip_prefix(&format!("writer {} ", case.method)))
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        super::session_corpus::same(&json!(calls), &json!([case.args]));
    }
    all_routes(driver).await?;
    outbound_messages(driver).await?;
    authored_options(driver).await?;
    authored_failures(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_setup_writer_forwards_every_operation() -> Result<(), String> {
    on_both_adapters!(forwarding)
}
/// Seventeen routes reject without an effect and recover on the same retained writer.
async fn all_routes(driver: &mut impl Driver) -> Result<(), String> {
    ask(
        driver,
        &json!({"method":"reader","parent":"original","retain":true}),
    )
    .await?;
    let routes = routes();
    for (method, reply) in routes {
        let mut input = json!({"retained":true,"method":method,"fromId":"branch target 雪","args":["./a/../雪\n","other Ω"],"options":{"id":"","parentSession":"relative"},"message":{"role":"user","content":"z","timestamp":3},"compaction":{"summary":"s","firstKeptEntryId":"cut","tokensBefore":1.25}});
        if method == "appendCustomMessageEntry" {
            input["message"] = json!({"customType":"kind","content":"text","display":false});
        }
        for error in [None, Some("host failed Ω\nline"), None] {
            input["control"] = json!({"value":reply,"error":error});
            let (answer, lines) = ask(driver, &input).await?;
            assert_eq!(
                answer,
                error.map_or_else(|| reply.clone(), |e| json!({"error":e})),
                "route {method}"
            );
            let calls: Vec<Value> = lines
                .iter()
                .filter_map(|line| line.strip_prefix("writer "))
                .map(|call| {
                    let (method, args) = call.split_once(' ').unwrap();
                    json!([method, serde_json::from_str::<Value>(args).unwrap()])
                })
                .collect();
            let expected = if error.is_none() {
                json!([route_call(method)])
            } else {
                json!([])
            };
            super::session_corpus::same(&json!(calls), &expected);
        }
    }
    for method in ["newSession", "createBranchedSession"] {
        for reply in [Value::Null, json!(""), json!("./file")] {
            assert_eq!(
                ask(
                    driver,
                    &json!({"method":method,"retained":true,"control":{"value":reply}})
                )
                .await?
                .0,
                reply
            );
        }
    }
    ask(driver, &json!({"drop":true})).await?;
    Ok(())
}
/// Setup completes its write before continuation reads the captured session.
async fn operation_identity(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", "setup-register")
        .await?;
    assert_eq!(
        ask(
            driver,
            &json!({"method":"reader","control":{"value":"entry"}})
        )
        .await?
        .0,
        json!("none")
    );
    command_options(driver).await?;
    driver.setup_pair().await?;
    for target in ["A 雪", "B Ω"] {
        bound_target(driver, target).await?;
    }
    retained_views(driver).await?;
    ask(driver, &json!({"drop":true})).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_setup_and_continuation_stay_with_their_operation() -> Result<(), String> {
    on_both_adapters!(operation_identity)
}
/// Normal, cancelled, uninvoked and rejected callbacks release their operation owners.
async fn ownership(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", "setup-register")
        .await?;
    for setup in [false, true] {
        for continuation in [false, true] {
            for mode in [
                "success",
                "cancelled",
                "unused",
                "setup-error",
                "continuation-error",
                "host-error",
            ] {
                ownership_case(driver, (setup, continuation), mode).await?;
            }
        }
    }
    dropped_operations(driver).await?;
    let (_, lines) = ask(
        driver,
        &json!({"method":"reader","reenter":true,"control":{"unused":true}}),
    )
    .await?;
    assert!(lines.iter().any(|l| l == "register event setup-reentrant"));
    let result = driver
        .deliver_input("event setup-reentrant", "reentry")
        .await?;
    assert!(matches!(
        result.decision,
        super::scenario::Decision::Returned(Ok(None))
    ));
    invalid_setup(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_setup_callbacks_release_once_on_every_exit() -> Result<(), String> {
    on_both_adapters!(ownership)
}
/// Raw host context and its independently measured native outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecoderCase {
    /// Attribution.
    name: String,
    /// Original selected document.
    input: String,
    /// Accepted context or native error.
    expected: Value,
}
/// Selected records are decoded after the import without stricter field policy.
async fn decoders(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", "setup-register")
        .await?;
    let cases: Vec<DecoderCase> = serde_json::from_str(include_str!("setup_decoder.json")).unwrap();
    assert_eq!(cases.len(), 24);
    for case in cases {
        decoder_case(driver, &case).await?;
    }
    decoded_variants(driver).await?;
    let (_, lines) = ask(
        driver,
        &json!({"method":"getChildren","control":{"raw":"[broken","error":"host wins Ω"}}),
    )
    .await?;
    assert!(lines.contains(&format!(
        "entry setup-answer {}",
        json!({"error":"host wins Ω"})
    )));
    assert_eq!(
        ask(
            driver,
            &json!({"method":"getChildren","control":{"raw":"[broken"}})
        )
        .await?
        .0,
        json!({"error":"expected value at line 1 column 2"})
    );
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_setup_context_decodes_host_records_without_extra_policy() -> Result<(), String> {
    on_both_adapters!(decoders)
}

/// Distinct typed results for each writable route.
fn routes() -> [(&'static str, Value); 17] {
    [
        ("setSessionFile", json!(null)),
        ("newSession", json!("./relative/../file")),
        ("isPersisted", json!(true)),
        ("getChildren", json!([])),
        (
            "buildSessionContext",
            json!({"messages":[],"thinkingLevel":"custom","model":null}),
        ),
        ("appendMessage", json!("message-id")),
        ("appendThinkingLevelChange", json!("thinking-id")),
        ("appendModelChange", json!("model-id")),
        ("appendCompaction", json!("compact-id")),
        ("appendCustomEntry", json!("custom-id")),
        ("appendSessionInfo", json!("info-id")),
        ("appendCustomMessageEntry", json!("custom-message-id")),
        ("appendLabelChange", json!("label-id")),
        ("branch", json!(null)),
        ("resetLeaf", json!(null)),
        ("branchWithSummary", json!("summary-id")),
        ("createBranchedSession", json!("")),
    ]
}

/// Literal operation and operand expectations independent of adapter dispatch.
fn route_call(method: &str) -> Value {
    match method {
        "setSessionFile" => json!(["set_session_file", ["./a/../雪\n"]]),
        "newSession" => {
            json!(["new_session",[{"present":true,"value":{"id":"","parentSession":"relative"}}]])
        }
        "isPersisted" => json!(["is_persisted", []]),
        "getChildren" => json!(["get_children", ["./a/../雪\n"]]),
        "buildSessionContext" => json!(["build_session_context", []]),
        "appendMessage" => {
            json!(["append_message",[{"present":true,"value":{"role":"user","content":"z","timestamp":3}}]])
        }
        "appendThinkingLevelChange" => json!(["append_thinking_level_change", ["./a/../雪\n"]]),
        "appendModelChange" => json!(["append_model_change", ["./a/../雪\n", "other Ω"]]),
        "appendCompaction" => {
            json!(["append_compaction",[{"present":true,"value":{"summary":"s","firstKeptEntryId":"cut","tokensBefore":1.25}},null]])
        }
        "appendCustomEntry" => json!(["append_custom_entry",["./a/../雪\n",{"present":false}]]),
        "appendSessionInfo" => json!(["append_session_info", ["./a/../雪\n"]]),
        "appendCustomMessageEntry" => {
            json!(["append_custom_message_entry",[{"present":true,"value":{"customType":"kind","content":"text","display":false}}]])
        }
        "appendLabelChange" => json!(["append_label_change", ["./a/../雪\n", null]]),
        "branch" => json!(["branch", ["./a/../雪\n"]]),
        "resetLeaf" => json!(["reset_leaf", []]),
        "branchWithSummary" => {
            json!(["branch_with_summary",["branch target 雪","./a/../雪\n",{"present":false},null]])
        }
        "createBranchedSession" => json!(["create_branched_session", ["./a/../雪\n"]]),
        _ => panic!("missing route expectation: {method}"),
    }
}

/// One closed host outcome proves admission and release independently.
async fn ownership_case(
    driver: &mut impl Driver,
    callbacks: (bool, bool),
    mode: &str,
) -> Result<(), String> {
    let (setup, continuation) = callbacks;
    let mut input =
        json!({"method":"reader","setup":setup,"continuation":continuation,"control":{}});
    match mode {
        "cancelled" => input["control"]["cancelled"] = json!(true),
        "unused" => input["control"]["unused"] = json!(true),
        "setup-error" => input["setupError"] = json!("setup failed Ω"),
        "continuation-error" => input["continuationError"] = json!("continuation failed Ω"),
        "host-error" => driver.reject_next_session(),
        _ => (),
    }
    let live = driver.live();
    let dropped = driver.dropped().len();
    let (_, lines) = ask(driver, &input).await?;
    let result: Value = serde_json::from_str(
        lines
            .iter()
            .find_map(|l| l.strip_prefix("entry setup-outcome "))
            .unwrap(),
    )
    .unwrap();
    let expected = match mode {
        "cancelled" => json!({"cancelled":true}),
        "host-error" => json!({"error":"session rejected"}),
        "setup-error" if setup => json!({"error":"setup failed Ω"}),
        "continuation-error" if continuation => {
            json!({"error":"continuation failed Ω"})
        }
        _ => json!({"cancelled":false}),
    };
    assert_eq!(result, expected, "mode {mode}");
    assert_eq!(driver.live(), live, "mode {mode}");
    assert_captures(&lines, callbacks);
    assert_eq!(
        driver.dropped().len() - dropped,
        usize::from(setup) + usize::from(continuation)
    );
    let admitted = !matches!(mode, "host-error" | "cancelled" | "unused");
    assert_eq!(
        lines
            .iter()
            .filter(|l| l.starts_with("entry setup-answer "))
            .count(),
        usize::from(setup && admitted)
    );
    assert_eq!(
        lines
            .iter()
            .filter(|l| l.starts_with("entry setup-continuation "))
            .count(),
        usize::from(continuation && admitted && !(mode == "setup-error" && setup))
    );
    Ok(())
}

/// Distinct inputs to the overlapping operations.
pub(super) fn pair_input(target: &str) -> String {
    json!({"method":"appendSessionInfo","args":[target],"parent":"unwritten","continuation":true,"hold":true}).to_string()
}
/// Named barriers close the observation set before either continuation may run.
pub(super) fn pair_phase(lines: &[String], second_completed: bool) {
    let continued: Vec<_> = lines
        .iter()
        .filter(|l| l.starts_with("entry setup-continuation "))
        .collect();
    let completed: Vec<_> = lines
        .iter()
        .filter(|l| l.starts_with("entry setup-outcome "))
        .collect();
    if second_completed {
        assert_eq!(continued, [&"entry setup-continuation \"B\"".to_owned()]);
        assert_eq!(completed.len(), 1);
    } else {
        assert!(continued.is_empty());
        assert!(completed.is_empty());
    }
}

/// Both returned operations expose their own written name, in completion order.
pub(super) fn pair_completed(lines: &[String]) {
    let continued: Vec<_> = lines
        .iter()
        .filter(|line| line.starts_with("entry setup-continuation "))
        .map(String::as_str)
        .collect();
    assert_eq!(
        continued,
        [
            "entry setup-continuation \"B\"",
            "entry setup-continuation \"A\""
        ]
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("entry setup-outcome "))
            .count(),
        2
    );
}

/// Each distinct closure and future capture drops exactly once.
fn assert_captures(lines: &[String], callbacks: (bool, bool)) {
    let (setup, continuation) = callbacks;
    let captures: Vec<Value> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("entry setup-drop "))
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(
        captures
            .iter()
            .filter(|c| **c == json!("setup-closure"))
            .count(),
        usize::from(setup)
    );
    assert_eq!(
        captures
            .iter()
            .filter(|c| **c == json!("setup-future"))
            .count(),
        usize::from(setup)
    );
    assert_eq!(
        captures
            .iter()
            .filter(|c| **c == json!("continuation-future"))
            .count(),
        usize::from(continuation)
    );
}

/// The operation to drop after its setup future reaches an acknowledged host barrier.
pub(super) fn drop_input() -> String {
    json!({"method":"reader","continuation":true,"dropPhase":"setup"}).to_string()
}

/// Wrong-kind and taken/released identities cannot run author setup again.
async fn invalid_setup(driver: &mut impl Driver) -> Result<(), String> {
    let released = driver.dropped().last().copied().unwrap();
    let released = driver.reborrow(released)?;
    let wrong = driver.identity("command setup")?;
    let unknown = driver.unregistered()?;
    for handler in [released, wrong, unknown] {
        let logical = driver
            .callback_error(handler)
            .rsplit(' ')
            .next()
            .unwrap()
            .to_owned();
        let before = driver.transcript().len();
        let live = driver.live();
        assert_eq!(
            driver.notify_setup(handler).await,
            Err(format!("no pending continuation for identity {logical}"))
        );
        assert_eq!(driver.transcript().len(), before);
        assert_eq!(driver.live(), live);
    }
    let dropped = driver.dropped().len();
    driver.repeat_release(released).await?;
    driver.repeat_release(released).await?;
    assert_eq!(driver.dropped().len(), dropped);
    Ok(())
}

/// Existing typed message variants cross the writer route without record reshaping.
async fn outbound_messages(driver: &mut impl Driver) -> Result<(), String> {
    let messages = super::session_corpus::messages();
    assert_eq!(messages.as_array().unwrap().len(), 8);
    for message in messages.as_array().unwrap() {
        let (answer, lines) = ask(
            driver,
            &json!({"method":"appendMessage","message":message,"control":{"value":"message-id"}}),
        )
        .await?;
        assert_eq!(answer, json!("message-id"));
        let raw = lines
            .iter()
            .find_map(|l| l.strip_prefix("writer append_message "))
            .unwrap();
        super::session_corpus::same(
            &serde_json::from_str(raw).unwrap(),
            &json!([{"present":true,"value":message}]),
        );
    }
    Ok(())
}
/// Writer option presences are independent of command callback options.
async fn authored_options(driver: &mut impl Driver) -> Result<(), String> {
    for id in [None, Some(""), Some("id Ω")] {
        for parent in [None, Some(""), Some("./a/../b")] {
            let mut options = json!({});
            if let Some(id) = id {
                options["id"] = json!(id);
            }
            if let Some(parent) = parent {
                options["parentSession"] = json!(parent);
            }
            let (_, lines) = ask(
                driver,
                &json!({"method":"newSession","options":options,"control":{"value":null}}),
            )
            .await?;
            assert!(lines.contains(&format!(
                "writer new_session {}",
                json!([{"present":true,"value":options}])
            )));
        }
    }
    let (_, lines) = ask(
        driver,
        &json!({"method":"newSession","control":{"value":null}}),
    )
    .await?;
    assert!(lines.contains(&format!(
        "writer new_session {}",
        json!([{"present":false}])
    )));
    Ok(())
}
/// Every message and selected entry variant survives decoding in host order with repetition.
async fn decoded_variants(driver: &mut impl Driver) -> Result<(), String> {
    let rows: Vec<Value> = serde_json::from_str(include_str!("session_corpus.json")).unwrap();
    let mut entries: Vec<Value> = rows
        .iter()
        .filter(|r| r["family"] == "entry-variants" && r["input"]["type"] == "session_before_tree")
        .map(|r| {
            super::session_corpus::materialize(&r["input"]["preparation"]["entriesToSummarize"][0])
        })
        .collect();
    assert_eq!(entries.len(), 9);
    entries.push(entries[0].clone());
    for parent in ["parent", "does-not-match"] {
        let value = json!(entries);
        let (answer, lines) = ask(
            driver,
            &json!({"method":"getChildren","args":[parent],"control":{"value":value}}),
        )
        .await?;
        super::session_corpus::same(&answer, &value);
        assert!(lines.contains(&format!("writer get_children {}", json!([parent]))));
        assert_eq!(
            answer
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["z", "a", "z", "a", "z", "a", "z", "a", "z", "z"]
        );
    }
    let messages = super::session_corpus::messages();
    let context = json!({"messages":messages,"thinkingLevel":"open Ω","model":{"provider":"","modelId":"literal"}});
    super::session_corpus::same(
        &ask(
            driver,
            &json!({"method":"buildSessionContext","control":{"value":context}}),
        )
        .await?
        .0,
        &context,
    );
    Ok(())
}

/// Distinguish never-polled closures from independently admitted futures.
async fn dropped_operations(driver: &mut impl Driver) -> Result<(), String> {
    for phase in ["before", "setup"] {
        let dropped = driver.dropped().len();
        let live = driver.live();
        let start = driver.transcript().len();
        if phase == "setup" {
            driver.setup_drop().await?;
        } else {
            ask(
                driver,
                &json!({"method":"reader","continuation":true,"dropPhase":phase}),
            )
            .await?;
        }
        let lines = driver.transcript()[start..].to_vec();
        assert!(lines.contains(&format!("entry setup-dropped {}", json!(phase))));
        assert_captures(&lines, (true, true));
        assert_eq!(
            driver.dropped().len() - dropped,
            if phase == "before" { 0 } else { 2 }
        );
        assert_eq!(driver.live(), live);
    }
    Ok(())
}

/// Decode one selected raw context after the witnessed host effect.
async fn decoder_case(driver: &mut impl Driver, case: &DecoderCase) -> Result<(), String> {
    let (answer, lines) = ask(
        driver,
        &json!({"method":"buildSessionContext","control":{"raw":case.input}}),
    )
    .await?;
    assert_eq!(answer, case.expected, "{}", case.name);
    assert_eq!(
        lines
            .iter()
            .filter(|l| *l == "writer build_session_context []")
            .count(),
        1
    );
    assert_eq!(
        ask(
            driver,
            &json!({"method":"getChildren","control":{"value":[]}})
        )
        .await?
        .0,
        json!([])
    );
    Ok(())
}

/// A selected authored failure has no import effect and does not undo the previous write.
async fn authored_failures(driver: &mut impl Driver) -> Result<(), String> {
    ask(driver,&json!({"method":"appendSessionInfo","args":["earlier write"],"retain":true,"control":{"value":"entry"}})).await?;
    for method in ["appendMessage", "appendCompaction"] {
        let (answer,lines) = ask(driver,&json!({"method":method,"retained":true,"nonfinite":true,"message":{"role":"user","content":"typed","timestamp":1},"compaction":{"summary":"s","firstKeptEntryId":"cut","tokensBefore":1}})).await?;
        assert_eq!(
            answer,
            json!({"error":"extension wrote a non-finite number (Infinity or NaN)"})
        );
        assert!(!lines.iter().any(|l| l.starts_with("writer ")));
        assert_eq!(
            ask(driver, &json!({"method":"reader","retained":true}))
                .await?
                .0,
            json!("earlier write")
        );
    }
    ask(driver, &json!({"drop":true})).await?;
    Ok(())
}
/// An independently retained reader stays live after its writer's final alias is removed.
async fn retained_views(driver: &mut impl Driver) -> Result<(), String> {
    ask(driver,&json!({"method":"appendSessionInfo","args":["captured"],"retain":true,"retainReader":true,"control":{"value":"entry"}})).await?;
    ask(driver, &json!({"method":"reader","parent":"decoy"})).await?;
    ask(
        driver,
        &json!({"method":"appendSessionInfo","args":["updated captured"],"retained":true}),
    )
    .await?;
    let live = driver.live();
    ask(driver, &json!({"dropWriter":true})).await?;
    assert_eq!(driver.live(), live - 2);
    assert_eq!(
        ask(driver, &json!({"retainedReader":true})).await?.0,
        json!("updated captured")
    );
    ask(driver, &json!({"drop":true})).await?;
    Ok(())
}

/// Parent presence and independent callback options survive the same typed command import.
async fn command_options(driver: &mut impl Driver) -> Result<(), String> {
    for parent in [None, Some(""), Some("parent Ω")] {
        for callbacks in [(false, false), (true, false), (false, true), (true, true)] {
            let (setup, continuation) = callbacks;
            let (answer,lines) = ask(driver,&json!({"method":"reader","parent":parent,"setup":setup,"continuation":continuation,"control":{"record_options":true}})).await?;
            assert_eq!(
                answer,
                if setup {
                    json!(parent.unwrap_or("none"))
                } else {
                    Value::Null
                }
            );
            assert!(lines.contains(&format!(
                "setup-options {}",
                json!({"present":true,"parent":parent,"setup":setup,"continuation":continuation})
            )));
        }
    }
    command_outcomes(driver).await?;
    Ok(())
}

/// One pending setup writes and later reads its own captured target.
async fn bound_target(driver: &mut impl Driver, target: &str) -> Result<(), String> {
    let start = driver.transcript().len();
    driver.command_held("setup", &json!({"method":"appendSessionInfo","args":[target],"parent":"unwritten","continuation":true,"hold":true,"retain":true,"control":{"value":"entry"}}).to_string()).await?;
    let lines = driver.transcript()[start..].to_vec();
    let pending = lines
        .iter()
        .position(|l| l.starts_with("command suspended after wait-for-idle"))
        .unwrap();
    let expected = format!("entry setup-continuation {}", json!(target));
    let continuations: Vec<_> = lines
        .iter()
        .filter(|line| line.starts_with("entry setup-continuation "))
        .collect();
    assert_eq!(continuations, [&expected]);
    let continued = lines.iter().position(|line| line == &expected).unwrap();
    let done = lines
        .iter()
        .position(|l| l.starts_with("entry setup-outcome "))
        .unwrap();
    assert!(pending < continued && continued < done);
    ask(
        driver,
        &json!({"method":"reader","parent":"decoy","control":{}}),
    )
    .await?;
    assert_eq!(
        ask(driver, &json!({"method":"inheritedReader","retained":true}))
            .await?
            .0,
        json!(target)
    );
    Ok(())
}

/// Awaited host outcomes and host stale failures retain their own error boundaries.
async fn command_outcomes(driver: &mut impl Driver) -> Result<(), String> {
    for cancelled in [false, true] {
        let start = driver.transcript().len();
        driver.command_held("setup",&json!({"method":"reader","setup":false,"continuation":false,"control":{"hold_return":true,"cancelled":cancelled}}).to_string()).await?;
        let lines = driver.transcript()[start..].to_vec();
        assert!(lines.contains(&"command suspended after setup host pending".to_owned()));
        assert!(lines.contains(&format!(
            "entry setup-outcome {}",
            json!({"cancelled":cancelled})
        )));
    }
    for error in ["This extension ctx is stale", "custom stale Ω\nline", ""] {
        let (_, lines) = ask(
            driver,
            &json!({"method":"reader","continuation":true,"control":{"session_error":error}}),
        )
        .await?;
        assert!(lines.contains(&format!("entry setup-outcome {}", json!({"error":error}))));
        assert!(
            !lines.iter().any(|l| l.starts_with("entry setup-answer ")
                || l.starts_with("entry setup-continuation "))
        );
    }
    assert_eq!(
        ask(
            driver,
            &json!({"method":"reader","control":{"value":"entry"}})
        )
        .await?
        .0,
        json!("none")
    );
    Ok(())
}

/// Writable children share the entry payload decoder used by history and events.
async fn opaque_children(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", "setup-register")
        .await?;
    for payload in [
        Some(json!({"z": {"b": [false, null, "Ω"]}, "a": 1})),
        Some(json!([null, true, {"z": 1, "a": 2}])),
        Some(json!(2.5)),
        Some(json!(9_007_199_254_740_993_u64)),
        Some(json!("literal \"text\"")),
        Some(Value::Null),
        None,
    ] {
        let entries = super::session_records::opaque_entries(payload.as_ref());
        let (answer, lines) = ask(
            driver,
            &json!({"method":"getChildren", "args":["root"],
            "control":{"value":entries}}),
        )
        .await?;
        assert_eq!(answer, json!(entries));
        assert!(lines.contains(&format!("writer get_children {}", json!(["root"]))));
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}

#[test]
fn maestro_setup_children_roundtrip_arbitrary_entry_payloads() -> Result<(), String> {
    on_both_adapters!(opaque_children)
}
