//! Session actions observed through both author adapters.
use super::scenario::Driver;
use serde_json::{Value, json};
/// A single synchronous action request with its own transcript slice.
async fn ask(driver: &mut impl Driver, method: &str, input: Value) -> Result<Value, String> {
    let before = driver.transcript().len();
    driver
        .run_plain_command(
            "capture",
            &format!("action {}", json!({"method":method,"input":input})),
        )
        .await?;
    let lines = driver.transcript();
    let answer = lines[before..]
        .iter()
        .find_map(|line| line.strip_prefix("entry action-answer "))
        .ok_or("no action answer")?;
    serde_json::from_str(answer).map_err(|e| e.to_string())
}
/// Independently recorded action observations.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Recorded inputs and results.
    records: Vec<Case>,
}
/// One unique source query.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Operation family.
    group: String,
    /// Authored operands.
    input: Value,
    /// Source-observed outcome.
    expected: Value,
}
/// Load the complete recorded inventory.
fn corpus() -> Result<Vec<Case>, String> {
    let corpus: Corpus =
        serde_json::from_str(include_str!("action_corpus.json")).map_err(|e| e.to_string())?;
    assert_eq!(corpus.records.len(), 305);
    Ok(corpus.records)
}
/// Configure an action response through the same authored instance.
async fn control(driver: &mut impl Driver, value: Value) -> Result<(), String> {
    driver
        .run_plain_command(
            "capture",
            &format!("action {}", json!({"method":"control","input":value})),
        )
        .await
}
/// One ordinary case, including empty and absent results.
async fn ordinary_case(driver: &mut impl Driver, case: &Case) -> Result<(), String> {
    let (method, reply, expected) = ordinary_expected(case)?;
    let config = if reply.is_object() && reply.get("error").is_some() {
        json!({"error":reply["error"]})
    } else {
        json!({"value":reply})
    };
    control(driver, config).await?;
    let before = driver.transcript().len();
    let actual = if case.input["mode"] == "pending" {
        driver
            .command_held(
                "capture",
                &format!("action {}", json!({"method":method,"input":case.input})),
            )
            .await?;
        let lines = driver.transcript();
        assert_pending_model(&lines[before..]);
        serde_json::from_str(
            lines[before..]
                .iter()
                .find_map(|l| l.strip_prefix("entry action-answer "))
                .ok_or("pending answer absent")?,
        )
        .map_err(|e| e.to_string())?
    } else {
        ask(driver, method, case.input.clone()).await?
    };
    assert_eq!(actual, expected, "ordinary group {}", case.group);
    let lines = driver.transcript();
    if let Some(expected) = case.expected.get("args") {
        assert_operands(&lines[before..], case, method, expected);
    }
    if let Some(expected) = case.expected.get("model") {
        let observed = lines[before..]
            .iter()
            .find_map(|l| l.strip_prefix("action setModel "))
            .ok_or("no selected model")?;
        let observed: Value = serde_json::from_str(observed).unwrap();
        super::session_corpus::same(&observed[0], expected);
        super::selection_events::assert_order(&observed[0]["compat"], &expected["compat"]);
    }
    Ok(())
}
/// Compare each action's host operands, retaining optional member presence.
fn assert_operands(lines: &[String], case: &Case, method: &str, expected: &Value) {
    if method == "appendEntry" {
        let observed = lines
            .iter()
            .find_map(|l| l.strip_prefix("entry state Ω "))
            .unwrap();
        assert_eq!(
            observed,
            if expected[1]["present"] == false {
                "none".to_owned()
            } else {
                expected[1]["value"].to_string()
            }
        );
        assert_eq!(case.input["customType"], expected[0]["value"]);
        return;
    }
    let prefix = format!("action {method} ");
    let observed: Value =
        serde_json::from_str(lines.iter().find_map(|l| l.strip_prefix(&prefix)).unwrap()).unwrap();
    let observed = match method {
        "sendMessage" | "sendUserMessage" => observed,
        "setLabel" => json!([{"present":true,"value":observed[0]},observed[1]]),
        _ => json!([{"present":true,"value":observed[0]}]),
    };
    super::session_corpus::same(&observed, expected);
}
/// Literal options, data, metadata and replies remain unchanged.
async fn actions(driver: &mut impl Driver) -> Result<(), String> {
    let mut count = 0;
    for case in corpus()? {
        if matches!(
            case.group.as_str(),
            "customMessages"
                | "userMessages"
                | "entries"
                | "names"
                | "nameReads"
                | "labels"
                | "activeWrites"
                | "activeReads"
                | "toolInventory"
                | "commandInventory"
                | "thinkingWrites"
                | "thinkingReads"
                | "models"
        ) {
            ordinary_case(driver, &case).await?;
            count += 1;
        }
    }
    assert_eq!(count, 202);
    live_reads(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
/// Repeated getters obtain the host's current value from the same facade.
async fn live_reads(driver: &mut impl Driver) -> Result<(), String> {
    for case in corpus()?.into_iter().filter(|c| c.group == "liveReads") {
        let method = case.input["method"].as_str().unwrap();
        for (field, expected) in [("first", "before"), ("second", "after")] {
            let reply = &case.input[field];
            control(driver, json!({"value":reply})).await?;
            let expected = if method == "getSessionName" {
                json!({"present":true,"value":case.expected[expected]})
            } else {
                case.expected[expected].clone()
            };
            assert_eq!(
                ask(driver, method, json!({})).await?,
                json!({"ok":expected})
            );
        }
    }
    Ok(())
}
#[test]
fn maestro_session_actions_keep_delivery_and_payload_presence() -> Result<(), String> {
    on_both_adapters!(actions)
}
/// Construct typed legal operands for each rejected action.
fn error_input(method: &str) -> Value {
    match method {
        "sendMessage" => {
            json!({"message":{"customType":"","content":"","display":false},"options":{"present":false}})
        }
        "sendUserMessage" => json!({"content":"","options":{"present":false}}),
        "appendEntry" => json!({"customType":"state Ω","data":{"present":false}}),
        "setActiveTools" => json!({"names":[]}),
        "setThinkingLevel" => json!({"level":"off"}),
        "setModel" => json!({"model":super::selection_events::model()}),
        _ => json!({}),
    }
}
/// Rejections remain catchable before effects, including nonfinite model encoding.
async fn errors(driver: &mut impl Driver) -> Result<(), String> {
    let mut count = 0;
    for case in corpus()?.into_iter().filter(|c| {
        matches!(
            c.group.as_str(),
            "preinitialization" | "stale" | "hostFailures"
        )
    }) {
        error_case(driver, &case).await?;
        count += 1;
    }
    assert_eq!(count, 52);
    control(driver, json!({"value":true})).await?;
    let before = driver.transcript().len();
    let mut input = error_input("setModel");
    input["nonfinite"] = json!(true);
    assert_eq!(
        ask(driver, "setModel", input).await?,
        json!({"error":"extension wrote a non-finite number (Infinity or NaN)"})
    );
    assert!(
        !driver.transcript()[before..]
            .iter()
            .any(|l| l.starts_with("action setModel "))
    );
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_action_errors_keep_messages_and_leave_the_instance_usable() -> Result<(), String> {
    on_both_adapters!(errors)
}
/// Admit through command or ordinary context without driving any notification.
async fn compact(driver: &mut impl Driver, input: Value) -> Result<Value, String> {
    let before = driver.transcript().len();
    if input["view"] == "ordinary" {
        let delivered = driver
            .deliver_input("event action-compact", &input.to_string())
            .await?;
        assert!(matches!(
            delivered.decision,
            super::scenario::Decision::Returned(Ok(None))
        ));
    } else {
        driver
            .run_plain_command("capture", &format!("compact {input}"))
            .await?;
    }
    let lines = driver.transcript();
    serde_json::from_str(
        lines[before..]
            .iter()
            .find_map(|l| l.strip_prefix("entry compact-return "))
            .ok_or("admission did not return")?,
    )
    .map_err(|e| e.to_string())
}
/// Optional compaction records keep their presence through every context view.
async fn compaction_presence(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", r#"compact {"view":"register"}"#)
        .await?;
    for view in ["ordinary", "command", "replacement"] {
        let mut observations = Vec::new();
        for absent in [true, false] {
            let start = driver.transcript().len();
            assert_eq!(
                compact(driver, json!({"view":view,"absent":absent})).await?,
                json!({"ok":null})
            );
            let admissions: Vec<_> = driver.transcript()[start..]
                .iter()
                .filter(|line| line.starts_with("compact admission "))
                .cloned()
                .collect();
            assert_eq!(admissions.len(), 1);
            observations.push(admissions);
        }
        assert_ne!(
            observations[0], observations[1],
            "compaction import erased options in {view}"
        );
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}

#[test]
fn compaction_options_preserve_presence() -> Result<(), String> {
    on_both_adapters!(compaction_presence)
}

/// Notification delivery stays closed until the synchronous admission has returned.
async fn compaction_returns(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", r#"compact {"view":"register"}"#)
        .await?;
    forwarding(driver).await?;
    let before = driver.transcript().len();
    let live = driver.live();
    assert_eq!(
        compact(
            driver,
            json!({"view":"ordinary","complete":true,"error":true,"name":"first"})
        )
        .await?,
        json!({"ok":null})
    );
    assert_eq!(
        driver.live(),
        live + 2,
        "accepted callbacks remain owned after admission"
    );
    let first = driver.identity(&format!("compact {} complete", admissions(driver) - 1))?;
    assert!(
        !driver.transcript()[before..]
            .iter()
            .any(|l| l.starts_with("entry compact-complete ")
                || l.starts_with("entry compact-error "))
    );
    assert_eq!(
        compact(driver, json!({"complete":true,"name":"second"})).await?,
        json!({"ok":null})
    );
    let second = driver.identity(&format!("compact {} complete", admissions(driver) - 1))?;
    assert_eq!(
        driver
            .notify_compaction(
                second,
                r#"{"summary":"second-result","firstKeptEntryId":"cut","tokensBefore":1}"#,
                true,
            )
            .await,
        Ok(())
    );
    assert_eq!(
        driver
            .notify_compaction(
                first,
                r#"{"summary":"first-result","firstKeptEntryId":"cut","tokensBefore":1}"#,
                true,
            )
            .await,
        Ok(())
    );
    assert_reverse_notifications(driver, before)?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_compaction_returns_before_its_notifications() -> Result<(), String> {
    on_both_adapters!(compaction_returns)
}
/// Each admitted request has a unique host registration name.
fn admissions(driver: &impl Driver) -> usize {
    driver
        .transcript()
        .iter()
        .filter(|l| l.starts_with("compact admission "))
        .count()
}
/// Source options cross all three inherited context views without instruction defaults.
async fn forwarding(driver: &mut impl Driver) -> Result<(), String> {
    let mut count = 0;
    for case in corpus()?.into_iter().filter(|c| {
        matches!(
            c.group.as_str(),
            "compactForwarding" | "compactAbsent" | "compactStale"
        )
    }) {
        for view in ["ordinary", "command", "replacement"] {
            forward_view(driver, &case, view).await?;
        }
        count += 1;
    }
    assert_eq!(count, 14);
    control(driver, json!({})).await
}
/// Complete or fail a request after admission, forwarding completion rejection to its error callback.
async fn notification_case(driver: &mut impl Driver, case: &Case) -> Result<(), String> {
    let mut input =
        json!({"complete":true,"error":true,"instructions":{"present":true,"value":""}});
    if case.input["mode"] == "completion-error" {
        input["completeError"] = json!("notification rejected Ω");
    }
    let before = driver.transcript().len();
    compact(driver, input).await?;
    let request = admissions(driver) - 1;
    let complete = driver.identity(&format!("compact {request} complete"))?;
    let error = driver.identity(&format!("compact {request} error"))?;
    if case.input["mode"] == "operation-error" {
        driver
            .notify_compaction(error, "compaction failed Ω\nline", false)
            .await?;
    } else {
        assert_eq!(
            case.input["result"].get("details").is_some(),
            case.input["detail"]["present"] == true
        );
        if case.input["detail"]["present"] == true {
            assert_eq!(
                case.input["result"]["details"],
                case.input["detail"]["value"]
            );
        }
        let result = driver
            .notify_compaction(complete, &case.input["result"].to_string(), true)
            .await;
        if case.input["mode"] == "completion-error" {
            assert_eq!(result, Err("notification rejected Ω".into()));
            driver
                .notify_compaction(error, &result.unwrap_err(), false)
                .await?;
        } else {
            result?;
        }
    }
    let trace = notification_trace(driver, before);
    super::session_corpus::same(&json!(trace), &case.expected["trace"]);
    Ok(())
}
/// All optional callback combinations and finite numeric signs retain their distinct outcomes.
async fn optional_notifications(driver: &mut impl Driver) -> Result<(), String> {
    for case in corpus()?
        .into_iter()
        .filter(|c| c.group == "optionalNotifications")
    {
        let before = driver.transcript().len();
        compact(driver, case.input.clone()).await?;
        let request = admissions(driver) - 1;
        let success = case.input["outcome"] == "success";
        if case.input[if success { "complete" } else { "error" }] == true {
            let handler = driver.identity(&format!(
                "compact {request} {}",
                if success { "complete" } else { "error" }
            ))?;
            driver
                .notify_compaction(
                    handler,
                    if success {
                        r#"{"summary":"s","firstKeptEntryId":"e","tokensBefore":1}"#
                    } else {
                        "failed"
                    },
                    success,
                )
                .await?;
        }
        let seen: Vec<Value> = driver.transcript()[before..]
            .iter()
            .filter_map(|l| {
                if l.starts_with("entry compact-complete ") {
                    Some(json!("complete"))
                } else if l.starts_with("entry compact-error ") {
                    Some(json!("error"))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(json!(seen), case.expected["seen"]);
    }
    compaction_numbers(driver).await?;
    Ok(())
}
/// Success records, operation errors and both author failures remain independently visible.
async fn notifications(driver: &mut impl Driver) -> Result<(), String> {
    for case in corpus()?.into_iter().filter(|c| c.group == "notifications") {
        notification_case(driver, &case).await?;
    }
    optional_notifications(driver).await?;
    compact(
        driver,
        json!({"error":true,"errorError":"error notification rejected 雪\nline"}),
    )
    .await?;
    let handler = driver.identity(&format!("compact {} error", admissions(driver) - 1))?;
    let before = driver.transcript().len();
    assert_eq!(
        driver.notify_compaction(handler, "", false).await,
        Err("error notification rejected 雪\nline".into())
    );
    let messages: Vec<_> = driver.transcript()[before..]
        .iter()
        .filter(|l| l.starts_with("entry compact-error "))
        .cloned()
        .collect();
    assert_eq!(messages, ["entry compact-error \"\""]);
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_compaction_notifications_keep_results_and_callback_errors() -> Result<(), String> {
    on_both_adapters!(notifications)
}
/// Read capture destruction only from this completed operation's transcript slice.
fn destroyed(driver: &impl Driver, start: usize) -> Vec<Value> {
    driver.transcript()[start..]
        .iter()
        .filter_map(|l| l.strip_prefix("entry compact-drop "))
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}
/// Each rejected admission rolls back exactly the identities and captures it allocated.
async fn rejected_ownership(driver: &mut impl Driver) -> Result<(), String> {
    for complete in [false, true] {
        for error in [false, true] {
            control(driver, json!({"error":"admission refused 雪"})).await?;
            let live = driver.live();
            let dropped = driver.dropped().len();
            let before = driver.transcript().len();
            assert_eq!(
                compact(
                    driver,
                    json!({"complete":complete,"error":error,"name":"rejected"})
                )
                .await?,
                json!({"error":"admission refused 雪"})
            );
            assert_eq!(driver.live(), live);
            assert_eq!(
                driver.dropped().len() - dropped,
                usize::from(complete) + usize::from(error)
            );
            let captures = destroyed(driver, before);
            assert_eq!(captures.len(), usize::from(complete) + usize::from(error));
            assert_eq!(captures.contains(&json!("complete rejected")), complete);
            assert_eq!(captures.contains(&json!("error rejected")), error);
        }
    }
    control(driver, json!({})).await
}
/// Either release order preserves the other callback, including repeat invocation.
async fn independent_ownership(driver: &mut impl Driver) -> Result<(), String> {
    for complete_first in [false, true] {
        release_order(driver, complete_first).await?;
    }
    Ok(())
}
/// Callback identity entries and retained authored aliases have independent ownership.
async fn aliases_and_reentry(driver: &mut impl Driver) -> Result<(), String> {
    let before = driver.transcript().len();
    let live = driver.live();
    compact(
        driver,
        json!({"complete":true,"retainAlias":true,"name":"alias"}),
    )
    .await?;
    let first = format!("compact {} complete", admissions(driver) - 1);
    compact(driver, json!({"alias":true})).await?;
    let second = format!("compact {} complete", admissions(driver) - 1);
    let survivor = driver.identity(&second)?;
    assert!(driver.release(&first).await);
    assert!(destroyed(driver, before).is_empty());
    driver
        .notify_compaction(
            survivor,
            r#"{"summary":"alias survives","firstKeptEntryId":"e","tokensBefore":1}"#,
            true,
        )
        .await?;
    assert!(driver.release(&second).await);
    assert_eq!(driver.live(), live);
    assert!(destroyed(driver, before).is_empty());
    driver
        .run_plain_command("capture", r#"compact {"view":"drop-alias"}"#)
        .await?;
    assert_eq!(destroyed(driver, before), [json!("complete alias")]);
    reentrant_release(driver).await?;
    Ok(())
}
/// Wrong-kind, missing and released dispatches do not disturb independent captures.
async fn ownership(driver: &mut impl Driver) -> Result<(), String> {
    rejected_ownership(driver).await?;
    independent_ownership(driver).await?;
    aliases_and_reentry(driver).await?;
    let unknown = driver.unregistered()?;
    let command = driver.identity("command capture")?;
    for id in [unknown, command] {
        for complete in [false, true] {
            let before = driver.transcript().len();
            assert_eq!(
                driver.notify_compaction(id, "{}", complete).await,
                Err(driver.callback_error(id))
            );
            assert_eq!(driver.transcript().len(), before);
        }
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_compaction_release_preserves_independent_callback_owners() -> Result<(), String> {
    on_both_adapters!(ownership)
}
/// Original selected wire record with an independently measured native decoder outcome.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DecoderCase {
    /// Owning public operation.
    kind: String,
    /// Case attribution without payload contents.
    name: String,
    /// Untouched raw document, retaining duplicate members and unread depth.
    input: String,
    /// Accepted record or native failure.
    result: Value,
}
/// Inventory lists wrap one selected record, shifting only its native column location.
fn inventory_error(error: &str) -> String {
    let (prefix, column) = error.rsplit_once(" column ").unwrap();
    format!("{prefix} column {}", column.parse::<usize>().unwrap() + 1)
}
/// Each operation uses the existing native selected-record decoding rules.
async fn decoders(driver: &mut impl Driver) -> Result<(), String> {
    let cases: Vec<DecoderCase> =
        serde_json::from_str(include_str!("action_decoder.json")).map_err(|e| e.to_string())?;
    assert_eq!(cases.len(), 26);
    compact(driver, json!({"complete":true,"wire":true})).await?;
    let complete = driver.identity(&format!("compact {} complete", admissions(driver) - 1))?;
    for case in cases {
        decoder_case(driver, &case, complete).await?;
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_action_results_use_their_existing_record_decoders() -> Result<(), String> {
    on_both_adapters!(decoders)
}
/// The host forwards the existing emissions ledger to a receiver after the notification returns.
async fn emissions(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", r#"compact {"view":"receiver"}"#)
        .await?;
    for complete in [false, true] {
        for fail in [false, true] {
            emission_case(driver, complete, fail.then_some("after emission 雪\nline")).await?;
        }
    }
    for complete in [false, true] {
        rejected_emission(driver, complete).await?;
    }
    driver.reject_bus_emission(None);
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_compaction_notifications_reuse_callback_scoped_emission() -> Result<(), String> {
    on_both_adapters!(emissions)
}

/// Select source case operands and expected outcome.
fn ordinary_expected(case: &Case) -> Result<(&'static str, Value, Value), String> {
    Ok(match case.group.as_str() {
        "customMessages" => ("sendMessage", Value::Null, json!({"ok":null})),
        "userMessages" => ("sendUserMessage", Value::Null, json!({"ok":null})),
        "entries" => ("appendEntry", Value::Null, json!({"ok":null})),
        "names" => ("setSessionName", Value::Null, json!({"ok":null})),
        "labels" => ("setLabel", Value::Null, json!({"ok":null})),
        "activeWrites" => ("setActiveTools", Value::Null, json!({"ok":null})),
        "thinkingWrites" => ("setThinkingLevel", Value::Null, json!({"ok":null})),
        "nameReads" => (
            "getSessionName",
            case.input["value"]["value"].clone(),
            json!({"ok":case.expected["value"]}),
        ),
        "activeReads" => (
            "getActiveTools",
            case.input["names"].clone(),
            json!({"ok":case.expected["names"]}),
        ),
        "toolInventory" => (
            "getAllTools",
            case.input["value"].clone(),
            json!({"ok":case.expected["value"]}),
        ),
        "commandInventory" => (
            "getCommands",
            case.input["value"].clone(),
            json!({"ok":case.expected["value"]}),
        ),
        "thinkingReads" => (
            "getThinkingLevel",
            case.input["level"].clone(),
            json!({"ok":case.expected["level"]}),
        ),
        "models" => (
            "setModel",
            case.input["outcome"].clone(),
            case.expected["answer"].clone(),
        ),
        _ => return Err("unexpected ordinary case".into()),
    })
}

/// A rejected action precedes effects and leaves the instance usable.
async fn error_case(driver: &mut impl Driver, case: &Case) -> Result<(), String> {
    let method = case.input["method"].as_str().unwrap();
    let message = case.expected["message"].as_str().unwrap();
    if case.group == "preinitialization" {
        assert_eq!(
            case.expected["phase"],
            if method == "setModel" {
                "await"
            } else {
                "call"
            }
        );
    }
    if case.group == "stale" {
        assert_eq!(case.expected["hostCalls"], 0);
        if case.input["customMessage"]["present"] == true {
            assert_eq!(
                case.input["customMessage"]["value"],
                case.expected["message"]
            );
        }
    }
    control(driver, json!({"error":message})).await?;
    let before = driver.transcript().len();
    assert_eq!(
        ask(driver, method, error_input(method)).await?,
        json!({"error":message}),
        "rejected route {method}"
    );
    assert!(
        !driver.transcript()[before..]
            .iter()
            .any(|l| l.starts_with("action "))
    );
    control(driver, json!({"value":"healthy 雪"})).await?;
    assert_eq!(
        ask(driver, "getSessionName", json!({})).await?,
        json!({"ok":{"present":true,"value":"healthy 雪"}})
    );

    Ok(())
}
/// Preserve each view's compact admission and error boundary.
async fn forward_view(driver: &mut impl Driver, case: &Case, view: &str) -> Result<(), String> {
    let mut input = case.input.clone();
    input["view"] = json!(view);
    if case.group == "compactAbsent" {
        assert_eq!(case.expected["value"]["present"], false);
        input["absent"] = json!(true);
    }
    control(
        driver,
        if case.group == "compactStale" {
            json!({"error":case.expected["error"]})
        } else {
            json!({})
        },
    )
    .await?;
    let before = driver.transcript().len();
    let actual = compact(driver, input).await?;
    let lines = driver.transcript();
    if case.group == "compactStale" {
        assert_eq!(actual, case.expected);
        assert!(
            !lines[before..]
                .iter()
                .any(|l| l.starts_with("compact admission "))
        );
    } else {
        assert_eq!(actual, json!({"ok":null}));
        assert!(lines[before..].contains(&format!(
            "compact binding {}",
            if view == "replacement" {
                "/replacement"
            } else {
                "/work"
            }
        )));
        let observed: Value = serde_json::from_str(
            lines[before..]
                .iter()
                .find_map(|l| l.strip_prefix("compact admission "))
                .unwrap(),
        )
        .unwrap();
        let mut expected = if case.group == "compactAbsent" {
            json!({"instructions":{"present":false},"complete":false,"error":false})
        } else {
            case.expected.clone()
        };
        expected["present"] = json!(case.group != "compactAbsent");
        assert_eq!(observed, expected);
    }

    Ok(())
}

/// Classify notification effects without discarding their order.
fn notification_trace(driver: &impl Driver, before: usize) -> Vec<Value> {
    driver.transcript()[before..]
        .iter()
        .filter_map(|line| {
            if let Some(v) = line.strip_prefix("compact admission ") {
                let v: Value = serde_json::from_str(v).unwrap();
                Some(json!({"kind":"requested","instructions":v["instructions"]}))
            } else if line.starts_with("entry compact-return ") {
                Some(json!({"kind":"returned"}))
            } else if let Some(v) = line.strip_prefix("entry compact-complete ") {
                let v: Value = serde_json::from_str(v).unwrap();
                Some(json!({"kind":"complete","value":v["value"]}))
            } else {
                line.strip_prefix("entry compact-error ").map(
                    |v| json!({"kind":"error","message":serde_json::from_str::<Value>(v).unwrap()}),
                )
            }
        })
        .collect()
}

/// Finite token values retain their signed zero and fractional values.
async fn compaction_numbers(driver: &mut impl Driver) -> Result<(), String> {
    for case in corpus()?
        .into_iter()
        .filter(|c| c.group == "compactNumbers")
    {
        compact(driver, json!({"complete":true})).await?;
        let handler = driver.identity(&format!("compact {} complete", admissions(driver) - 1))?;
        let number = if case.input["negativeZero"] == true {
            -0.0
        } else {
            case.input["value"].as_f64().unwrap()
        };
        let before = driver.transcript().len();
        driver
            .notify_compaction(
                handler,
                &json!({"summary":"s","firstKeptEntryId":"e","tokensBefore":number}).to_string(),
                true,
            )
            .await?;
        let observed: Value = serde_json::from_str(
            driver.transcript()[before..]
                .iter()
                .find_map(|l| l.strip_prefix("entry compact-complete "))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            observed["value"]["tokensBefore"].as_f64(),
            case.expected["value"].as_f64()
        );
        assert_eq!(observed["negativeZero"], case.expected["negativeZero"]);
    }

    Ok(())
}

/// Release one callback and repeatedly invoke its independent survivor.
async fn release_order(driver: &mut impl Driver, complete_first: bool) -> Result<(), String> {
    let before = driver.transcript().len();
    let live = driver.live();
    compact(
        driver,
        json!({"complete":true,"error":true,"name":"independent"}),
    )
    .await?;
    let request = admissions(driver) - 1;
    let complete_name = format!("compact {request} complete");
    let error_name = format!("compact {request} error");
    let complete = driver.identity(&complete_name)?;
    let error = driver.identity(&error_name)?;
    assert_eq!(driver.live(), live + 2);
    assert!(destroyed(driver, before).is_empty());
    let (first, last, released, survivor, dropped) = if complete_first {
        (
            &complete_name,
            &error_name,
            complete,
            error,
            "complete independent",
        )
    } else {
        (
            &error_name,
            &complete_name,
            error,
            complete,
            "error independent",
        )
    };
    let diagnostic = driver.callback_error(released);
    let logical: u32 = diagnostic.rsplit(' ').next().unwrap().parse().unwrap();
    assert!(driver.release(first).await);
    assert!(!driver.release(first).await);
    assert_eq!(driver.live(), live + 1);
    assert_eq!(destroyed(driver, before), [json!(dropped)]);
    surviving_callback(driver, survivor, !complete_first).await?;
    released_dispatch(driver, logical, complete_first).await?;
    final_release(driver, last, live, before).await?;

    Ok(())
}

/// Removing a capture before destruction permits registration from Drop.
async fn reentrant_release(driver: &mut impl Driver) -> Result<(), String> {
    let live = driver.live();
    compact(
        driver,
        json!({"complete":true,"reenter":true,"name":"reenter"}),
    )
    .await?;
    let name = format!("compact {} complete", admissions(driver) - 1);
    assert_eq!(driver.live(), live + 2);
    assert!(driver.release(&name).await);
    assert_eq!(driver.live(), live + 1);
    let name = format!("compact {} complete", admissions(driver) - 1);
    let handler = driver.identity(&name)?;
    let start = driver.transcript().len();
    driver
        .notify_compaction(
            handler,
            r#"{"summary":"reentrant works","firstKeptEntryId":"e","tokensBefore":1}"#,
            true,
        )
        .await?;
    assert!(
        driver.transcript()[start..]
            .contains(&"entry reentrant-complete \"reentrant works\"".into())
    );
    assert!(driver.release(&name).await);
    assert_eq!(driver.live(), live);

    Ok(())
}

/// Exercise one native record admission through its public operation.
async fn decoder_case(
    driver: &mut impl Driver,
    case: &DecoderCase,
    complete: u32,
) -> Result<(), String> {
    let before = driver.transcript().len();
    if case.kind == "compact" {
        let result = driver.notify_compaction(complete, &case.input, true).await;
        if let Some(error) = case.result["error"].as_str() {
            assert_eq!(result, Err(error.to_owned()), "{}", case.name);
            assert!(
                !driver.transcript()[before..]
                    .iter()
                    .any(|l| l.starts_with("entry compact-complete "))
            );
        } else {
            result?;
            let recorded: Value = serde_json::from_str(
                driver.transcript()[before..]
                    .iter()
                    .find_map(|l| l.strip_prefix("entry compact-wire "))
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(recorded, case.result["ok"], "{}", case.name);
        }
    } else {
        let method = match case.kind.as_str() {
            "tool" => "getAllTools",
            "command" => "getCommands",
            "thinking" => "getThinkingLevel",
            _ => return Err("unknown decoder operation".into()),
        };
        let list = case.kind != "thinking";
        let raw = if list {
            format!("[{}]", case.input)
        } else {
            case.input.clone()
        };
        control(driver, json!({"raw":raw})).await?;
        let actual = ask(driver, method, json!({"raw":true})).await?;
        let expected = if let Some(error) = case.result["error"].as_str() {
            json!({"error":if list {inventory_error(error)}else{error.to_owned()}})
        } else {
            json!({"ok":if list {json!([case.result["ok"]])}else{case.result["ok"].clone()}})
        };
        assert_eq!(actual, expected, "decoder {} {}", case.kind, case.name);
    }

    Ok(())
}

/// An emission preceding callback failure remains available to host routing.
async fn emission_case(
    driver: &mut impl Driver,
    complete: bool,
    error: Option<&str>,
) -> Result<(), String> {
    let channel = "\u{feff}channel 雪\u{85}\n ";
    let payload = json!({"z":[false,null,"雪"],"a":0});
    let mut input =
        json!({"complete":complete,"error":!complete,"emit":{"channel":channel,"payload":payload}});
    if let Some(error) = error {
        input[if complete {
            "completeError"
        } else {
            "errorError"
        }] = json!(error);
    }
    compact(driver, input).await?;
    let handler = driver.identity(&format!(
        "compact {} {}",
        admissions(driver) - 1,
        if complete { "complete" } else { "error" }
    ))?;
    let before = driver.transcript().len();
    let emitted = driver.bus_emissions().len();
    let result = driver
        .notify_compaction(
            handler,
            if complete {
                r#"{"summary":"s","firstKeptEntryId":"e","tokensBefore":1}"#
            } else {
                "failure 雪"
            },
            complete,
        )
        .await;
    assert_eq!(result, error.map_or(Ok(()), |e| Err(e.to_owned())));
    assert_eq!(
        driver.bus_emissions()[emitted..],
        [(channel.to_owned(), payload.clone())]
    );
    route_emission(driver, before, channel, &payload).await?;
    Ok(())
}

/// A callback catches the existing emission error without inventing another channel.
async fn rejected_emission(driver: &mut impl Driver, complete: bool) -> Result<(), String> {
    driver.reject_bus_emission(Some("emission denied 雪\nline".into()));
    compact(driver,json!({"complete":complete,"error":!complete,"emit":{"channel":"rejected","payload":false},"catchEmit":true})).await?;
    let handler = driver.identity(&format!(
        "compact {} {}",
        admissions(driver) - 1,
        if complete { "complete" } else { "error" }
    ))?;
    let before = driver.transcript().len();
    let emitted = driver.bus_emissions().len();
    driver
        .notify_compaction(
            handler,
            if complete {
                r#"{"summary":"s","firstKeptEntryId":"e","tokensBefore":1}"#
            } else {
                "failed"
            },
            complete,
        )
        .await?;
    assert!(
        driver.transcript()[before..]
            .contains(&"entry emit-error \"emission denied 雪\\nline\"".into())
    );
    assert_eq!(driver.bus_emissions().len(), emitted);

    Ok(())
}

/// Repeated notifications leave the surviving callback registered.
async fn surviving_callback(
    driver: &mut impl Driver,
    handler: u32,
    complete: bool,
) -> Result<(), String> {
    for _ in 0..2 {
        let start = driver.transcript().len();
        let live = driver.live();
        driver
            .notify_compaction(
                handler,
                if complete {
                    r#"{"summary":"survivor","firstKeptEntryId":"e","tokensBefore":1}"#
                } else {
                    "survivor 雪"
                },
                complete,
            )
            .await?;
        assert_eq!(
            driver.live(),
            live,
            "notification does not consume its callback entry"
        );
        assert!(
            driver.transcript()[start..]
                .iter()
                .any(|l| l.starts_with(if complete {
                    "entry compact-complete "
                } else {
                    "entry compact-error "
                }))
        );
    }

    Ok(())
}

/// Route a recorded emission only after its originating callback returned.
async fn route_emission(
    driver: &mut impl Driver,
    before: usize,
    channel: &str,
    payload: &Value,
) -> Result<(), String> {
    let emission = driver.bus_emissions().pop().unwrap();
    let received = json!({"channel":emission.0,"payload":emission.1});
    let returned = driver.transcript().len();
    let outcome = driver
        .deliver_input("event action-receiver", &received.to_string())
        .await?;
    assert!(matches!(
        outcome.decision,
        super::scenario::Decision::Returned(Ok(None))
    ));
    assert!(returned > before);
    let recorded: Value = serde_json::from_str(
        driver.transcript()[returned..]
            .iter()
            .find_map(|l| l.strip_prefix("entry compact-received "))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(recorded, json!({"channel":channel,"payload":payload}));

    Ok(())
}

/// A new borrow of a released identity receives the existing diagnostic.
async fn released_dispatch(
    driver: &mut impl Driver,
    logical: u32,
    complete: bool,
) -> Result<(), String> {
    let alias = driver.reborrow(logical)?;
    let dropped = driver.dropped().len();
    driver.repeat_release(alias).await?;
    driver.repeat_release(alias).await?;
    assert_eq!(driver.dropped().len(), dropped);
    assert_eq!(
        driver.notify_compaction(alias, "{}", complete).await,
        Err(driver.callback_error(alias))
    );
    Ok(())
}
/// Release the last independent owner and observe both capture destructions.
async fn final_release(
    driver: &mut impl Driver,
    name: &str,
    live: usize,
    before: usize,
) -> Result<(), String> {
    assert!(driver.release(name).await);
    assert_eq!(driver.live(), live);
    assert_eq!(destroyed(driver, before).len(), 2);
    Ok(())
}

/// Named host settlement closes the model operation before its authored answer.
fn assert_pending_model(lines: &[String]) {
    let started = lines
        .iter()
        .position(|l| l == "model selection started")
        .unwrap();
    let pending = lines
        .iter()
        .position(|l| l == "command suspended after model selection started")
        .unwrap();
    let settled = lines
        .iter()
        .position(|l| l.starts_with("action setModel "))
        .unwrap();
    let answer = lines
        .iter()
        .position(|l| l.starts_with("entry action-answer "))
        .unwrap();
    assert!(started < pending && pending < settled && settled < answer);
}

/// Distinct outstanding callbacks preserve the source reverse-completion trace.
fn assert_reverse_notifications(driver: &impl Driver, before: usize) -> Result<(), String> {
    let seen: Vec<Value> = driver.transcript()[before..]
        .iter()
        .filter_map(|l| l.strip_prefix("entry compact-complete "))
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let oracle = corpus()?
        .into_iter()
        .find(|c| c.group == "reverseNotifications")
        .unwrap();
    assert_eq!(oracle.input, json!({}));
    let trace: Vec<Value> = seen
        .iter()
        .map(|v| json!({"name":v["name"],"summary":v["value"]["summary"]}))
        .collect();
    assert_eq!(json!(trace), oracle.expected["trace"]);

    Ok(())
}

/// Both adapters transport open command sources through the existing inventory getter.
async fn shared_command_inventory(driver: &mut impl Driver) -> Result<(), String> {
    use crate::{SlashCommandInfo, SlashCommandSource, SourceInfo, SourceOrigin, SourceScope};
    let provenance = SourceInfo {
        path: "commands/example".into(),
        source: "package:fixture".into(),
        scope: SourceScope::Project,
        origin: SourceOrigin::Package,
        base_dir: Some("commands".into()),
    };
    let commands = [
        SlashCommandInfo {
            name: "example".into(),
            description: Some(String::new()),
            source: SlashCommandSource("future-source".into()),
            source_info: provenance.clone(),
        },
        SlashCommandInfo {
            name: "other".into(),
            description: None,
            source: SlashCommandSource(SlashCommandSource::EXTENSION.into()),
            source_info: provenance,
        },
    ];
    let wire = serde_json::to_value(commands).map_err(|e| e.to_string())?;
    assert_eq!(wire[0]["description"], "");
    assert!(wire[1].get("description").is_none());
    control(driver, json!({"value":wire})).await?;
    assert_eq!(
        ask(driver, "getCommands", json!({})).await?,
        json!({"ok":wire})
    );
    Ok(())
}

#[test]
fn command_inventory_uses_shared_open_source_records() -> Result<(), String> {
    on_both_adapters!(shared_command_inventory)
}
