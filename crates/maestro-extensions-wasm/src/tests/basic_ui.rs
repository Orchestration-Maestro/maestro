//! Basic retained-UI requests on both adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde::Deserialize;
use serde_json::{Value, json};

use super::scenario::Driver;
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

/// Installs the author probe without changing the other scenarios.
async fn install(driver: &mut impl Driver) -> Result<(), String> {
    driver.run_plain_command("capture", "ui").await
}

/// Runs one probe operation through the command route and returns its answer.
async fn operate(driver: &mut impl Driver, operation: Value) -> Result<Value, String> {
    driver
        .run_plain_command("ui", &operation.to_string())
        .await?;
    let text = driver
        .transcript()
        .into_iter()
        .rev()
        .find_map(|line| line.strip_prefix("entry ui-answer ").map(str::to_owned))
        .ok_or_else(|| "missing answer".to_owned())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
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

/// Runs the working messages rows through a fresh acquisition on both adapters.
async fn working_messages(driver: &mut impl Driver) -> Result<(), String> {
    let rows = corpus().working_messages;
    requests(driver, &rows, 3).await
}

#[test]
fn working_message_keeps_empty_and_reset_requests() -> Result<(), String> {
    on_both_adapters!(working_messages)
}

/// Runs the working indicator rows through a fresh acquisition on both adapters.
async fn working_indicator(driver: &mut impl Driver) -> Result<(), String> {
    let rows = corpus().working_indicator;
    requests(driver, &rows, 10).await
}

#[test]
fn working_indicator_keeps_independent_optional_fields() -> Result<(), String> {
    on_both_adapters!(working_indicator)
}

/// Runs the thinking labels rows through a fresh acquisition on both adapters.
async fn thinking_labels(driver: &mut impl Driver) -> Result<(), String> {
    let rows = corpus().thinking_labels;
    requests(driver, &rows, 3).await
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
    requests(driver, &rows, 9).await
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
    operate(driver, json!({ "op": "retain" })).await?;
    let before = driver.transcript().len();
    for set in [
        json!({ "op": "setWorkingMessage", "message": "busy" }),
        json!({ "op": "setWorkingIndicator", "options": {} }),
    ] {
        let operation = json!({ "op": "retained", "call": set });
        assert_eq!(operate(driver, operation).await?, json!({ "ok": true }));
    }
    for row in &corpus.working_visibility {
        let operation = json!({ "op": "retained", "call": row.call });
        assert_eq!(operate(driver, operation).await?, json!({ "ok": true }));
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

/// Every rejected request keeps the host's message and leaves the handle usable.
async fn rejections(driver: &mut impl Driver) -> Result<(), String> {
    let corpus = corpus();
    assert_eq!(corpus.errors.len(), 8, "corpus rows");
    install(driver).await?;
    operate(driver, json!({ "op": "retain" })).await?;
    let control = json!({ "op": "setTitle", "title": "after rejection" });
    for row in &corpus.errors {
        let error = row
            .error
            .as_ref()
            .ok_or_else(|| "missing error".to_owned())?;
        let operation = json!({ "op": "control", "error": error });
        assert_eq!(operate(driver, operation).await?, json!("controlled"));
        let from = driver.transcript().len();
        let operation = json!({ "op": "retained", "call": row.call });
        assert_eq!(
            operate(driver, operation).await?,
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
        let operation = json!({ "op": "retained", "call": control });
        assert_eq!(
            operate(driver, operation).await?,
            json!({ "ok": true }),
            "the same handle still works after {}",
            row.call
        );
    }
    Ok(())
}

#[test]
fn basic_ui_rejections_remain_catchable() -> Result<(), String> {
    on_both_adapters!(rejections)
}

/// Retains two aliases of one acquisition, calls both and returns its identity.
async fn retained_aliases(driver: &mut impl Driver) -> Result<usize, String> {
    for operation in [json!({ "op": "retain" }), json!({ "op": "alias" })] {
        assert_eq!(operate(driver, operation).await?, json!("retained"));
    }
    let notify = json!({ "op": "notify", "message": "retained" });
    let from = driver.transcript().len();
    for index in [0, 1] {
        let operation = json!({ "op": "retained", "index": index, "call": notify });
        assert_eq!(operate(driver, operation).await?, json!({ "ok": true }));
    }
    let seen = receipts(&driver.transcript()[from..]);
    assert_eq!(seen.len(), 2, "both aliases issued their own request");
    assert_eq!(seen[0].key, seen[1].key, "both aliases target one resource");
    Ok(seen[0].key)
}

/// Retained aliases keep one resource while a second acquisition stays independent.
async fn retention(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let base = driver.live();
    let acquired = retained_aliases(driver).await?;
    assert_eq!(driver.live(), base + 1, "aliases share one acquisition");
    let notify = json!({ "op": "notify", "message": "retained" });
    let from = driver.transcript().len();
    let operation = json!({ "op": "acquire", "call": notify });
    assert_eq!(operate(driver, operation).await?, json!({ "ok": true }));
    assert_ne!(
        receipts(&driver.transcript()[from..])[0].key,
        acquired,
        "a second acquisition is independent"
    );
    assert_eq!(
        driver.live(),
        base + 1,
        "only the retained acquisition is lent"
    );
    for _ in [0, 1] {
        assert_eq!(
            operate(driver, json!({ "op": "pop" })).await?,
            json!("dropped")
        );
    }
    assert_eq!(
        driver.live(),
        base,
        "dropping one alias keeps the resource and the last releases it once"
    );
    Ok(())
}

#[test]
fn retained_ui_calls_keep_identity_and_final_release() -> Result<(), String> {
    on_both_adapters!(retention)
}

/// Checks that the host recorded one notification with the supplied operands.
fn notified(driver: &impl Driver, from: usize, message: &str, kind: Option<&str>) {
    let seen = receipts(&driver.transcript()[from..]);
    assert_eq!(seen.len(), 1, "one retained notification for {message}");
    assert_eq!(seen[0].method, "notify");
    same(&seen[0].operands, &json!([message, kind]));
}

/// An event and a command callback each notify through a UI they acquire.
async fn admitted_callbacks(driver: &mut impl Driver) -> Result<(), String> {
    let event = json!({ "op": "acquire", "call": { "op": "notify", "message": "event" } });
    let from = driver.transcript().len();
    driver.deliver_input("event ui", &event.to_string()).await?;
    notified(driver, from, "event", None);
    let from = driver.transcript().len();
    let command = json!({ "op": "acquire", "call": { "op": "notify", "message": "command" } });
    assert_eq!(operate(driver, command).await?, json!({ "ok": true }));
    notified(driver, from, "command", None);
    Ok(())
}

/// A retained listener and the compaction callbacks report through one retained handle.
async fn retained_callbacks(driver: &mut impl Driver) -> Result<(), String> {
    operate(driver, json!({ "op": "retain" })).await?;
    assert_eq!(
        operate(driver, json!({ "op": "observe" })).await?,
        json!("observed")
    );
    let admission = admissions(driver) - 1;
    let from = driver.transcript().len();
    let emit = json!({ "op": "emit", "channel": "ui-bus", "input": "payload" });
    assert_eq!(operate(driver, emit).await?, json!("emitted"));
    notified(driver, from, "bus \"payload\"", Some("Warning"));
    let complete = driver.identity(&format!("compact {admission} complete"))?;
    let from = driver.transcript().len();
    driver
        .notify_compaction(
            complete,
            r#"{"summary":"reported","firstKeptEntryId":"e","tokensBefore":1}"#,
            true,
        )
        .await?;
    notified(driver, from, "complete reported", None);
    let error = driver.identity(&format!("compact {admission} error"))?;
    let from = driver.transcript().len();
    driver.notify_compaction(error, "failed", false).await?;
    notified(driver, from, "error failed", Some("Error"));
    Ok(())
}

/// Notifications from every admitted callback reach the host before it completes.
async fn callbacks(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    admitted_callbacks(driver).await?;
    retained_callbacks(driver).await?;
    assert_eq!(
        operate(driver, json!({ "op": "pop" })).await?,
        json!("dropped")
    );
    driver.release_all().await;
    assert_eq!(driver.live(), 0, "no UI resource stays lent");
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

#[test]
fn basic_ui_requests_work_from_registered_callbacks() -> Result<(), String> {
    on_both_adapters!(callbacks)
}
