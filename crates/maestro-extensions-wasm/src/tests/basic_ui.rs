//! Basic retained-UI requests on both adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde::Deserialize;
use serde_json::{Value, json};

use super::scenario::{Driver, transformed};
use super::session_corpus::same;
use super::ui_host::receipts;

/// One corpus row: a request, the host receipt it must reach, or its supplied rejection.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Requested call.
    call: Value,
    /// Expected host method and operands.
    #[serde(default)]
    host: Option<Host>,
    /// Supplied rejection instead of a receipt.
    #[serde(default)]
    error: Option<String>,
}

/// What the host must have recorded.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Host {
    /// Method the host reached.
    method: String,
    /// Operands the host received.
    operands: Value,
}

/// The request corpus, one list per behaviour test.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Corpus {
    /// Notification requests.
    notifications: Vec<Row>,
    /// Status requests.
    status: Vec<Row>,
    /// Working-message requests.
    working_messages: Vec<Row>,
    /// Working-visibility requests.
    working_visibility: Vec<Row>,
    /// Working-indicator requests.
    working_indicator: Vec<Row>,
    /// Hidden-thinking-label requests.
    thinking_labels: Vec<Row>,
    /// Fixed-widget requests.
    widgets: Vec<Row>,
    /// Title requests.
    titles: Vec<Row>,
    /// Supplied rejections.
    errors: Vec<Row>,
}

/// The request corpus.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("ui_corpus.json")).unwrap()
}

/// Installs the UI probe and the retained UI route without changing the other scenarios.
async fn install(driver: &mut impl Driver) -> Result<(), String> {
    driver.run_plain_command("capture", "ui").await?;
    driver.run_plain_command("capture", "catalog").await
}

/// The answer the extension reported last under `prefix`.
fn answer(driver: &impl Driver, prefix: &str) -> Result<Value, String> {
    let text = driver
        .transcript()
        .into_iter()
        .rev()
        .find_map(|line| line.strip_prefix(prefix).map(str::to_owned))
        .ok_or_else(|| format!("missing answer for {prefix}"))?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

/// Runs one probe operation on the UI author and returns its answer.
async fn operate(driver: &mut impl Driver, operation: Value) -> Result<Value, String> {
    driver
        .run_plain_command("ui", &operation.to_string())
        .await?;
    answer(driver, "entry ui-answer ")
}

/// Runs one request of the retained UI route and returns its answer.
async fn route(driver: &mut impl Driver, request: Value) -> Result<Value, String> {
    driver
        .run_plain_command("catalog", &request.to_string())
        .await?;
    answer(driver, "entry catalog-answer ")
}

/// Runs one request of the retained UI route through an event context.
async fn routed_event(driver: &mut impl Driver, request: &Value) -> Result<Value, String> {
    let delivery = driver
        .deliver_input("event catalog", &request.to_string())
        .await?;
    transformed(delivery.decision)
}

/// Retains, aliases or drops one independently retained UI owner.
async fn retained_owner(driver: &mut impl Driver, method: &str) -> Result<(), String> {
    assert_eq!(
        route(driver, json!({"action":"ui","method":method})).await?,
        json!({"ok":"done"}),
        "retained route method {method}"
    );
    Ok(())
}

/// Runs one request through a retained UI owner of the supplied index.
async fn retained_call(
    driver: &mut impl Driver,
    index: usize,
    call: &Value,
) -> Result<Value, String> {
    route(
        driver,
        json!({"action":"ui","method":"call","args":[call, index]}),
    )
    .await
}

/// Runs one request through a fresh acquisition and checks what the host received.
async fn request(driver: &mut impl Driver, call: &Value, host: &Host) -> Result<(), String> {
    let from = driver.transcript().len();
    assert_eq!(
        operate(driver, json!({ "op": "acquire", "call": call })).await?,
        json!({ "ok": true }),
        "accepted request {call}"
    );
    let lines = driver.transcript()[from..].to_vec();
    let seen = receipts(&lines);
    assert_eq!(seen.len(), 1, "exactly one host request for {call}");
    assert_eq!(seen[0].method, host.method, "method of {call}");
    same(&seen[0].operands, &host.operands);
    let received = lines.iter().position(|line| line.starts_with("ui "));
    let done = lines.iter().position(|line| line == "entry ui-done none");
    assert!(
        matches!((received, done), (Some(received), Some(done)) if received < done),
        "the host receives {call} before the author continues"
    );
    Ok(())
}

/// Runs every row of one group through a fresh acquisition.
async fn requests(driver: &mut impl Driver, rows: &[Row], count: usize) -> Result<(), String> {
    assert_eq!(rows.len(), count, "corpus rows");
    install(driver).await?;
    for row in rows {
        let host = row
            .host
            .as_ref()
            .ok_or_else(|| "missing receipt".to_owned())?;
        request(driver, &row.call, host).await?;
    }
    Ok(())
}

/// Runs the notifications rows through a fresh acquisition on both adapters.
async fn notifications(driver: &mut impl Driver) -> Result<(), String> {
    let rows = corpus().notifications;
    requests(driver, &rows, 8).await
}

#[test]
fn notifications_keep_message_and_optional_severity() -> Result<(), String> {
    on_both_adapters!(notifications)
}

/// Runs the status rows through a fresh acquisition on both adapters.
async fn status_calls(driver: &mut impl Driver) -> Result<(), String> {
    let rows = corpus().status;
    requests(driver, &rows, 7).await
}

#[test]
fn status_calls_keep_keys_replacement_and_clear() -> Result<(), String> {
    on_both_adapters!(status_calls)
}

/// Runs the working-message rows through a fresh acquisition, then repeats them through one
/// retained owner so a reset stays a reset.
async fn working_messages(driver: &mut impl Driver) -> Result<(), String> {
    let rows = corpus().working_messages;
    requests(driver, &rows, 3).await?;
    retained_owner(driver, "retain").await?;
    let mut owners = Vec::new();
    for _ in [0, 1] {
        for row in &rows {
            let host = row
                .host
                .as_ref()
                .ok_or_else(|| "missing receipt".to_owned())?;
            let from = driver.transcript().len();
            assert_eq!(
                retained_call(driver, 0, &row.call).await?,
                json!({"ok":"done"}),
                "retained request {}",
                row.call
            );
            let seen = receipts(&driver.transcript()[from..]);
            assert_eq!(seen.len(), 1, "one retained request for {}", row.call);
            same(&seen[0].operands, &host.operands);
            owners.push(seen[0].key);
        }
    }
    assert!(
        owners.windows(2).all(|pair| pair[0] == pair[1]),
        "every repeated request kept its owner: {owners:?}"
    );
    Ok(())
}

#[test]
fn working_message_keeps_empty_and_reset_requests() -> Result<(), String> {
    on_both_adapters!(working_messages)
}

/// Runs the working indicator rows through a fresh acquisition on both adapters.
async fn working_indicator(driver: &mut impl Driver) -> Result<(), String> {
    let rows = corpus().working_indicator;
    requests(driver, &rows, 11).await
}

#[test]
fn working_indicator_keeps_independent_optional_fields() -> Result<(), String> {
    on_both_adapters!(working_indicator)
}

/// Runs the thinking-label rows through a fresh acquisition, then from the command and event
/// contexts on one retained owner.
async fn thinking_labels(driver: &mut impl Driver) -> Result<(), String> {
    let rows = corpus().thinking_labels;
    requests(driver, &rows, 3).await?;
    retained_owner(driver, "retain").await?;
    let mut owners = Vec::new();
    for row in &rows {
        let host = row
            .host
            .as_ref()
            .ok_or_else(|| "missing receipt".to_owned())?;
        let from = driver.transcript().len();
        assert_eq!(
            retained_call(driver, 0, &row.call).await?,
            json!({"ok":"done"})
        );
        let event = json!({"action":"ui","method":"call","args":[row.call, 0]});
        assert_eq!(routed_event(driver, &event).await?, json!({"ok":"done"}));
        let seen = receipts(&driver.transcript()[from..]);
        assert_eq!(
            seen.len(),
            2,
            "both contexts reached the host for {}",
            row.call
        );
        same(&seen[0].operands, &host.operands);
        same(&seen[1].operands, &host.operands);
        assert_eq!(
            seen[0].key, seen[1].key,
            "both contexts used one acquired owner for {}",
            row.call
        );
        owners.push(seen[0].key);
    }
    assert!(
        owners.windows(2).all(|pair| pair[0] == pair[1]),
        "one owner served every label: {owners:?}"
    );
    Ok(())
}

#[test]
fn thinking_labels_keep_empty_and_reset_requests() -> Result<(), String> {
    on_both_adapters!(thinking_labels)
}

/// Runs the titles rows through a fresh acquisition on both adapters.
async fn titles(driver: &mut impl Driver) -> Result<(), String> {
    let rows = corpus().titles;
    requests(driver, &rows, 2).await
}

#[test]
fn titles_keep_empty_and_literal_text() -> Result<(), String> {
    on_both_adapters!(titles)
}

/// Runs the widgets rows through a fresh acquisition on both adapters.
async fn widgets(driver: &mut impl Driver) -> Result<(), String> {
    let rows = corpus().widgets;
    requests(driver, &rows, 10).await
}

#[test]
fn fixed_widgets_keep_lines_placement_and_removal() -> Result<(), String> {
    on_both_adapters!(widgets)
}

/// Toggling visibility must not rewrite the message or the indicator options.
async fn visibility(driver: &mut impl Driver) -> Result<(), String> {
    let corpus = corpus();
    assert_eq!(corpus.working_visibility.len(), 2, "corpus rows");
    install(driver).await?;
    retained_owner(driver, "retain").await?;
    let before = driver.transcript().len();
    for set in [
        json!({ "op": "setWorkingMessage", "message": "busy" }),
        json!({ "op": "setWorkingIndicator", "options": {} }),
    ] {
        assert_eq!(retained_call(driver, 0, &set).await?, json!({"ok":"done"}));
    }
    for row in &corpus.working_visibility {
        assert_eq!(
            retained_call(driver, 0, &row.call).await?,
            json!({"ok":"done"})
        );
    }
    let seen = receipts(&driver.transcript()[before..]);
    let methods: Vec<&str> = seen.iter().map(|seen| seen.method.as_str()).collect();
    assert_eq!(
        methods,
        [
            "setWorkingMessage",
            "setWorkingIndicator",
            "setWorkingVisible",
            "setWorkingVisible"
        ],
        "visibility sends no replacement of the earlier requests"
    );
    same(
        &json!([seen[2].operands, seen[3].operands]),
        &json!([[true], [false]]),
    );
    Ok(())
}

#[test]
fn working_visibility_does_not_rewrite_other_requests() -> Result<(), String> {
    on_both_adapters!(visibility)
}

/// Every rejected request keeps the host's message and leaves the owner usable.
async fn rejections(driver: &mut impl Driver) -> Result<(), String> {
    let corpus = corpus();
    assert_eq!(corpus.errors.len(), 8, "corpus rows");
    install(driver).await?;
    retained_owner(driver, "retain").await?;
    let control = json!({ "op": "setTitle", "title": "after rejection" });
    for row in &corpus.errors {
        let error = row
            .error
            .as_ref()
            .ok_or_else(|| "missing error".to_owned())?;
        let operation = json!({ "op": "control", "error": error });
        assert_eq!(operate(driver, operation).await?, json!("controlled"));
        let from = driver.transcript().len();
        assert_eq!(
            retained_call(driver, 0, &row.call).await?,
            json!({ "error": error }),
            "exact host rejection of {}",
            row.call
        );
        assert_eq!(
            receipts(&driver.transcript()[from..]).len(),
            1,
            "one attempted host call for {}",
            row.call
        );
        assert_eq!(
            retained_call(driver, 0, &control).await?,
            json!({"ok":"done"}),
            "the same owner still works after {}",
            row.call
        );
    }
    Ok(())
}

#[test]
fn basic_ui_rejections_remain_catchable() -> Result<(), String> {
    on_both_adapters!(rejections)
}

/// The notification row used for the ownership walk.
fn notify() -> Value {
    json!({ "op": "notify", "message": "retained" })
}

/// The owner key of the one host request recorded after `from`.
fn sole_receipt(driver: &impl Driver, from: usize) -> usize {
    let seen = receipts(&driver.transcript()[from..]);
    assert_eq!(seen.len(), 1, "one host request");
    seen[0].key
}

/// Retains two aliases and returns the owner both of them reached.
async fn alias_pair(driver: &mut impl Driver) -> Result<usize, String> {
    let base = driver.live();
    for method in ["retain", "alias"] {
        retained_owner(driver, method).await?;
    }
    let from = driver.transcript().len();
    for index in [0, 1] {
        assert_eq!(
            retained_call(driver, index, &notify()).await?,
            json!({"ok":"done"})
        );
    }
    let seen = receipts(&driver.transcript()[from..]);
    assert_eq!(seen.len(), 2, "both aliases issued their own request");
    assert_eq!(seen[0].key, seen[1].key, "both aliases target one resource");
    assert_eq!(driver.live(), base + 1, "aliases share one acquisition");
    Ok(seen[0].key)
}

/// A retained alias answers after its originating context is rebound.
async fn rebound(driver: &mut impl Driver, shared: usize) -> Result<(), String> {
    let from = driver.transcript().len();
    let request = json!({"action":"ui","method":"call","args":[notify(), 0]});
    assert_eq!(routed_event(driver, &request).await?, json!({"ok":"done"}));
    assert_eq!(
        sole_receipt(driver, from),
        shared,
        "the retained alias keeps its resource after the callback returned"
    );
    Ok(())
}

/// A second acquisition stays independent of the shared aliases.
async fn independent(driver: &mut impl Driver, shared: usize) -> Result<(), String> {
    let base = driver.live();
    let from = driver.transcript().len();
    assert_eq!(
        operate(driver, json!({ "op": "acquire", "call": notify() })).await?,
        json!({ "ok": true })
    );
    assert_ne!(
        sole_receipt(driver, from),
        shared,
        "a second acquisition is independent"
    );
    retained_owner(driver, "retain").await?;
    assert_eq!(
        driver.live(),
        base + 1,
        "the second acquisition is retained"
    );
    let from = driver.transcript().len();
    for index in [2, 0] {
        assert_eq!(
            retained_call(driver, index, &notify()).await?,
            json!({"ok":"done"})
        );
    }
    let seen = receipts(&driver.transcript()[from..]);
    assert_eq!(seen.len(), 2, "each retained owner issued its own request");
    assert_ne!(
        seen[0].key, shared,
        "the second acquisition stays independent"
    );
    assert_eq!(seen[1].key, shared, "the aliases still share one owner");
    retained_owner(driver, "pop").await
}

/// Dropping one alias keeps the resource; the final alias releases it exactly once.
async fn dropped_alias(driver: &mut impl Driver, base: usize, shared: usize) -> Result<(), String> {
    assert_eq!(
        driver.live(),
        base + 1,
        "the second acquisition is released"
    );
    let from = driver.transcript().len();
    assert_eq!(
        retained_call(driver, 0, &notify()).await?,
        json!({"ok":"done"})
    );
    assert_eq!(
        sole_receipt(driver, from),
        shared,
        "the first resource answers while the other acquisition is gone"
    );
    retained_owner(driver, "pop").await?;
    assert_eq!(driver.live(), base + 1, "one alias remains lent");
    let from = driver.transcript().len();
    assert_eq!(
        retained_call(driver, 0, &notify()).await?,
        json!({"ok":"done"})
    );
    assert_eq!(
        sole_receipt(driver, from),
        shared,
        "the remaining alias answers"
    );
    retained_owner(driver, "pop").await?;
    assert_eq!(
        driver.live(),
        base,
        "the final alias releases the resource once"
    );
    Ok(())
}

/// Retained aliases keep one resource across rebinding, drops and a second acquisition.
async fn retention(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let base = driver.live();
    let shared = alias_pair(driver).await?;
    rebound(driver, shared).await?;
    independent(driver, shared).await?;
    dropped_alias(driver, base, shared).await
}

#[test]
fn retained_ui_calls_keep_identity_and_final_release() -> Result<(), String> {
    on_both_adapters!(retention)
}

/// Checks that the host recorded one notification with the supplied operands, and returns the
/// owner it reached.
fn received(driver: &impl Driver, from: usize, message: &str, kind: Option<&str>) -> usize {
    let seen = receipts(&driver.transcript()[from..]);
    assert_eq!(seen.len(), 1, "one notification for {message}");
    assert_eq!(seen[0].method, "notify");
    same(&seen[0].operands, &json!([message, kind]));
    seen[0].key
}

/// Checks that one notification reached the supplied owner.
fn notified(driver: &impl Driver, from: usize, message: &str, kind: Option<&str>, owner: usize) {
    assert_eq!(
        received(driver, from, message, kind),
        owner,
        "notification {message} used the retained owner"
    );
}

/// An event and a command callback each notify through a UI they acquire.
async fn admitted_callbacks(driver: &mut impl Driver) -> Result<(), String> {
    let event = json!({ "op": "acquire", "call": { "op": "notify", "message": "event" } });
    let from = driver.transcript().len();
    driver.deliver_input("event ui", &event.to_string()).await?;
    received(driver, from, "event", None);
    let from = driver.transcript().len();
    let command = json!({ "op": "acquire", "call": { "op": "notify", "message": "command" } });
    assert_eq!(operate(driver, command).await?, json!({ "ok": true }));
    received(driver, from, "command", None);
    Ok(())
}

/// The number of compaction requests the host admitted so far.
fn admissions(driver: &impl Driver) -> usize {
    driver
        .transcript()
        .iter()
        .filter(|line| line.starts_with("compact admission "))
        .count()
}

/// A retained listener and two separately admitted compaction callbacks report through the
/// owners they kept.
async fn retained_callbacks(driver: &mut impl Driver) -> Result<(), String> {
    let from = driver.transcript().len();
    assert_eq!(
        operate(driver, json!({ "op": "observe" })).await?,
        json!("observed")
    );
    let listener = received(driver, from, "observe", Some("Info"));
    let from = driver.transcript().len();
    let emit = json!({ "op": "emit", "channel": "ui-bus", "input": "payload" });
    assert_eq!(operate(driver, emit).await?, json!("emitted"));
    notified(driver, from, "bus \"payload\"", Some("Warning"), listener);
    for outcome in ["complete", "error"] {
        let complete = outcome == "complete";
        let from = driver.transcript().len();
        assert_eq!(
            operate(driver, json!({ "op": "compact" })).await?,
            json!("compacted")
        );
        let owner = received(driver, from, "compact", None);
        let identity = driver.identity(&format!("compact {} {outcome}", admissions(driver) - 1))?;
        let from = driver.transcript().len();
        if complete {
            driver
                .notify_compaction(
                    identity,
                    r#"{"summary":"reported","firstKeptEntryId":"e","tokensBefore":1}"#,
                    true,
                )
                .await?;
            notified(driver, from, "complete reported", None, owner);
        } else {
            driver.notify_compaction(identity, "failed", false).await?;
            notified(driver, from, "error failed", Some("Error"), owner);
        }
    }
    Ok(())
}

/// Notifications from every admitted callback reach the host before it completes.
async fn callbacks(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    admitted_callbacks(driver).await?;
    retained_callbacks(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0, "no UI resource stays lent");
    Ok(())
}

#[test]
fn basic_ui_requests_work_from_registered_callbacks() -> Result<(), String> {
    on_both_adapters!(callbacks)
}
