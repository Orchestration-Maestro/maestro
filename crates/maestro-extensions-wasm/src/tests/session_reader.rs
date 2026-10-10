//! Retained session queries through both author adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::scenario::{Decision, Driver};
use serde::Deserialize;
use serde_json::{Value, json};

/// Strict corpus container.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Named host states.
    corpus: Vec<State>,
}
/// One recorded host state.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    /// Diagnostic state identity.
    name: String,
    /// Unique recorded queries.
    queries: Vec<Query>,
}
/// A query and its independently recorded source result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    /// Public operation selected by the author.
    method: String,
    /// Literal operands asserted at the host boundary.
    args: Vec<Expected>,
    /// Recorded source result.
    expected: Expected,
}
/// Distinguishes source absence from a supplied value.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Source result classification.
    kind: String,
    /// Supplied result, absent for undefined.
    value: Option<Value>,
}
/// Read every committed field through the strict fixture records.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("reader_corpus.json")).unwrap()
}
/// Install the same author probes in each adapter.
async fn install(driver: &mut impl Driver) -> Result<(), String> {
    driver.run_plain_command("capture", "reader").await
}
/// Execute an ordinary or command operation and recover its public answer.
async fn ask(driver: &mut impl Driver, request: Value, command: bool) -> Result<Value, String> {
    if command {
        driver
            .run_plain_command("reader", &request.to_string())
            .await?;
        let transcript = driver.transcript();
        let text = transcript
            .last()
            .ok_or("missing command answer")?
            .strip_prefix("entry reader-answer ")
            .ok_or("wrong command answer")?;
        return serde_json::from_str(text).map_err(|e| e.to_string());
    }
    let handler = driver.identity("event reader")?;
    let input = json!({"type":"input","text":request.to_string(),"source":"rpc"});
    let delivery = driver
        .deliver(handler, &input.to_string(), "/work", None)
        .await?;
    let Decision::Returned(Ok(Some(text))) = delivery.decision else {
        return Err("reader probe failed to return".into());
    };
    let result: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    assert_eq!(result["action"], "transform");
    serde_json::from_str(result["text"].as_str().ok_or("missing returned text")?)
        .map_err(|e| e.to_string())
}
/// Assert one completed public operation and its exact return value.
async fn check(
    driver: &mut impl Driver,
    request: Value,
    command: bool,
    expected: Value,
) -> Result<(), String> {
    assert_eq!(ask(driver, request, command).await?, expected);
    Ok(())
}
/// Build a host reply for one query; no source algorithms run in the guest.
fn supplied(query: &Query) -> Value {
    match query.expected.kind.as_str() {
        "value" | "null" | "undefined" => {}
        _ => panic!("unknown corpus classification"),
    }
    let value = query.expected.value.clone().unwrap_or(Value::Null);
    if query.method == "getTree" {
        return json!({"tree":value});
    }
    let args = operands(query);
    if query.method == "getSessionDir" {
        return json!({"replies":[{"method":query.method,"args":args,"value":value},{"method":"getCwd","args":[],"value":"distinct working directory"}]});
    }
    json!({"replies":[{"method":query.method,"args":args,"value":value}]})
}
/// Decode each saved argument's explicit absence classification.
fn operands(query: &Query) -> Value {
    if query.method == "getBranch" && query.args.is_empty() {
        return json!([null]);
    }
    json!(
        query
            .args
            .iter()
            .map(|arg| match arg.kind.as_str() {
                "value" => arg.value.clone().expect("value operand"),
                "undefined" => Value::Null,
                _ => panic!("unexpected argument classification"),
            })
            .collect::<Vec<_>>()
    )
}
/// All recorded scalar, entry and list routes, including exact literal operands.
async fn queries(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let mut count = 0;
    for state in corpus().corpus {
        assert!(!state.name.is_empty());
        for query in state.queries {
            let actual = ask(driver, json!({"action":"query","method":query.method,"args":operands(&query),"control":{"bind":supplied(&query)}}), false).await?;
            let expected = query.expected.value.unwrap_or(Value::Null);
            assert!(
                actual.get("error").is_none(),
                "state {} route {}: {}",
                state.name,
                query.method,
                actual["error"]
            );
            super::session_corpus::same(&actual["ok"], &expected);
            count += 1;
        }
    }
    assert_eq!(count, 55);
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_session_reader_forwards_every_query() -> Result<(), String> {
    on_both_adapters!(queries)
}

/// Query actual returned entry records from the existing session fixtures.
async fn records(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let rows: Vec<Value> = serde_json::from_str(include_str!("session_corpus.json")).unwrap();
    let mut count = 0;
    let mut seen = std::collections::HashSet::new();
    for row in rows.iter().filter(|r| {
        [
            "entry-variants",
            "entry-optionals",
            "nested-messages",
            "custom-content",
        ]
        .contains(&r["family"].as_str().unwrap())
    }) {
        let Some(entries) = row["input"].get("branchEntries") else {
            continue;
        };
        if !seen.insert(entries.to_string()) {
            continue;
        }
        let entries = super::session_corpus::materialize(entries);
        let entry = entries[0].clone();
        record_node(driver, &entry).await?;
        let id = entry["id"].clone();
        for (method, args, expected) in [
            ("getEntry", json!([id]), entry),
            ("getEntries", json!([]), entries),
        ] {
            let data = json!({"replies":[{"method":method,"args":args,"value":expected}]});
            let actual = ask(
                driver,
                json!({"action":"query","method":method,"args":args,"control":{"bind":data}}),
                false,
            )
            .await?;
            assert!(actual.get("error").is_none());
            super::session_corpus::same(&actual["ok"], &expected);
        }
        count += 1;
    }
    assert_eq!(count, 34);
    for data in ["false", "0", "\"\"", "{\"z\":0,\"a\":false}", "[]"] {
        let entry = json!({"type":"custom","id":"data","parentId":null,"timestamp":"t","customType":"open","data":data});
        let actual = ask(driver, json!({"action":"query","method":"getEntries","control":{"bind":{"replies":[{"method":"getEntries","args":[],"value":[entry]}]}}}), false).await?;
        assert_eq!(actual, json!({"ok":[entry]}));
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
/// The node accessor decodes the same selected record without dropping its payload.
async fn record_node(driver: &mut impl Driver, entry: &Value) -> Result<(), String> {
    let tree = json!({"roots":[0],"nodes":[{"entry":entry,"children":[]}]});
    check(
        driver,
        json!({"action":"query","method":"getTree","control":{"bind":{"tree":tree}}}),
        false,
        json!({"ok":tree}),
    )
    .await
}
#[test]
fn maestro_reader_records_keep_declared_fields() -> Result<(), String> {
    on_both_adapters!(records)
}

/// Descendants retain the captured node after ancestors and readers are dropped.
async fn trees(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    for state in corpus()
        .corpus
        .into_iter()
        .filter(|state| ["populated", "deep-tree"].contains(&state.name.as_str()))
    {
        tree_lifetime(driver, state).await?;
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
/// Observe the final owners of one tree allocation.
async fn tree_lifetime(driver: &mut impl Driver, state: State) -> Result<(), String> {
    let baseline = driver.live();
    let query = state
        .queries
        .into_iter()
        .find(|q| q.method == "getTree")
        .unwrap();
    let tree = query.expected.value.as_ref().unwrap();
    let depth = if state.name == "deep-tree" { 259 } else { 1 };
    check(
        driver,
        json!({"action":"retain","control":{"bind":supplied(&query)}}),
        false,
        json!({"ok":"kept"}),
    )
    .await?;
    assert_eq!(driver.live(), baseline + 1);
    check(
        driver,
        json!({"action":"retain-node","retained":true,"args":[depth]}),
        false,
        json!({"ok":"kept"}),
    )
    .await?;
    assert_eq!(driver.live(), baseline + 2);
    check(
        driver,
        json!({"action":"drop-reader"}),
        false,
        json!({"ok":"dropped"}),
    )
    .await?;
    assert_eq!(driver.live(), baseline + 1);
    saved_node_reads(driver, &tree["nodes"][depth]).await?;
    check(
        driver,
        json!({"action":"drop"}),
        false,
        json!({"ok":"dropped"}),
    )
    .await?;
    assert_eq!(driver.live(), baseline);
    Ok(())
}
/// Read the captured descendant through each accessor after its owners disappear.
async fn saved_node_reads(driver: &mut impl Driver, expected_node: &Value) -> Result<(), String> {
    for (method, field) in [
        ("entry", "entry"),
        ("label", "label"),
        ("labelTimestamp", "labelTimestamp"),
    ] {
        let expected = &expected_node[field];
        check(
            driver,
            json!({"action":"node","method":method}),
            false,
            json!({"ok":expected}),
        )
        .await?;
    }
    check(
        driver,
        json!({"action":"node","method":"children"}),
        false,
        json!({"ok":{"roots":[],"nodes":[]}}),
    )
    .await?;
    Ok(())
}
#[test]
fn maestro_reader_tree_keeps_order_and_independent_lifetimes() -> Result<(), String> {
    on_both_adapters!(trees)
}

/// Distinct host identity used to distinguish acquired and retained readers.
fn identity(value: &str) -> Value {
    json!({"replies":[{"method":"getSessionId","args":[],"value":value}]})
}
/// Both getters resolve now while old handles retain their own live session.
async fn acquisitions(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    for command in [false, true] {
        let old = if command {
            "old-command"
        } else {
            "old-ordinary"
        };
        check(
            driver,
            json!({"action":"keep-context","control":{"bind":identity("original")}}),
            command,
            json!({"ok":"kept"}),
        )
        .await?;
        check(
            driver,
            json!({"action":old,"method":"getSessionId"}),
            command,
            json!({"ok":"original"}),
        )
        .await?;
        check(
            driver,
            json!({"action":"retain"}),
            command,
            json!({"ok":"kept"}),
        )
        .await?;
        check(driver, json!({"action":"query","method":"getSessionId","retained":true,"control":{"update":identity("changed original")}}), command, json!({"ok":"changed original"})).await?;
        check(driver, json!({"action":old,"method":"getSessionId","control":{"bind":identity("replacement")}}), command, json!({"ok":"replacement"})).await?;
        check(
            driver,
            json!({"action":"query","method":"getSessionId","retained":true}),
            command,
            json!({"ok":"changed original"}),
        )
        .await?;
        check(
            driver,
            json!({"action":"drop"}),
            command,
            json!({"ok":"dropped"}),
        )
        .await?;
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_reader_getters_resolve_at_call_time() -> Result<(), String> {
    on_both_adapters!(acquisitions)
}

/// Forwarded stale-acquisition default with the product's public identity.
const STALE_READER: &str = "This extension ctx is stale after session replacement or reload. Do not use a captured maestro or command ctx after ctx.newSession(), ctx.fork(), ctx.switchSession(), or ctx.reload(). For newSession, fork, and switchSession, move post-replacement work into withSession and use the ctx passed to withSession. For reload, do not use the old ctx after await ctx.reload().";
/// Stale contexts reject new acquisition without revoking extracted resources.
async fn retained(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    for command in [false, true] {
        for stale in [STALE_READER, "stale 雪"] {
            retained_state(driver, command, stale).await?;
        }
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
/// Retain the independent resources before invalidating acquisition.
async fn retained_state(
    driver: &mut impl Driver,
    command: bool,
    stale: &str,
) -> Result<(), String> {
    let baseline = driver.live();
    let entry = json!({"type":"custom","id":"saved","parentId":null,"timestamp":"t","customType":"saved","data":"false"});
    let mut data = identity("old");
    data["tree"] = json!({"roots":[0],"nodes":[{"entry":entry,"children":[],"label":"saved label","labelTimestamp":"literal"}]});
    check(
        driver,
        json!({"action":"retain","control":{"bind":data}}),
        command,
        json!({"ok":"kept"}),
    )
    .await?;
    check(
        driver,
        json!({"action":"alias","retained":true}),
        command,
        json!({"ok":"aliased"}),
    )
    .await?;
    assert_eq!(driver.live(), baseline + 1);
    check(
        driver,
        json!({"action":"retain-node","retained":true}),
        command,
        json!({"ok":"kept"}),
    )
    .await?;
    check(
        driver,
        json!({"action":"keep-context"}),
        command,
        json!({"ok":"kept"}),
    )
    .await?;
    stale_reads(driver, command, stale, &entry, baseline).await
}
/// Check stale acquisition, surviving aliases and synchronous final release.
async fn stale_reads(
    driver: &mut impl Driver,
    command: bool,
    stale: &str,
    entry: &Value,
    baseline: usize,
) -> Result<(), String> {
    let old = if command {
        "old-command"
    } else {
        "old-ordinary"
    };
    check(
        driver,
        json!({"action":old,"method":"getSessionId","control":{"stale":stale}}),
        command,
        json!({"error":stale}),
    )
    .await?;
    check(driver, json!({"action":old,"method":"getSessionId","control":{"stale":"ignored second invalidation"}}), command, json!({"error":stale})).await?;
    check(driver, json!({"action":"query","method":"getSessionId","retained":true,"control":{"update":identity("live retained")}}), command, json!({"ok":"live retained"})).await?;
    check(
        driver,
        json!({"action":"node","method":"entry"}),
        command,
        json!({"ok":entry}),
    )
    .await?;
    check(
        driver,
        json!({"action":"drop-alias"}),
        command,
        json!({"ok":"dropped"}),
    )
    .await?;
    release_reader_aliases(driver, command, baseline).await
}
/// Observe last-alias release and a healthy new acquisition.
async fn release_reader_aliases(
    driver: &mut impl Driver,
    command: bool,
    baseline: usize,
) -> Result<(), String> {
    let retained_contexts = 1;
    assert_eq!(driver.live(), baseline + retained_contexts + 2);
    check(
        driver,
        json!({"action":"query","method":"getSessionId","retained":true}),
        command,
        json!({"ok":"live retained"}),
    )
    .await?;
    check(
        driver,
        json!({"action":"drop-reader"}),
        command,
        json!({"ok":"dropped"}),
    )
    .await?;
    assert_eq!(driver.live(), baseline + retained_contexts + 1);
    check(
        driver,
        json!({"action":"query","method":"getSessionId","control":{"bind":identity("healthy")}}),
        command,
        json!({"ok":"healthy"}),
    )
    .await?;
    check(
        driver,
        json!({"action":"drop"}),
        command,
        json!({"ok":"dropped"}),
    )
    .await?;
    assert_eq!(driver.live(), baseline);
    Ok(())
}
#[test]
fn maestro_retained_reader_survives_stale_context() -> Result<(), String> {
    on_both_adapters!(retained)
}

/// Every reader route and node accessor propagates catchable host failures.
async fn errors(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    reader_failures(driver).await?;
    node_failures(driver).await?;
    malformed_entries(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
/// Reader failures followed by a healthy read on the same handle.
async fn reader_failures(driver: &mut impl Driver) -> Result<(), String> {
    let state = corpus().corpus.into_iter().next().unwrap();
    let mut methods = std::collections::HashSet::new();
    for query in state
        .queries
        .into_iter()
        .filter(|query| methods.insert(query.method.clone()))
    {
        check(
            driver,
            json!({"action":"retain","control":{"bind":{"error":"host failure 雪"}}}),
            false,
            json!({"ok":"kept"}),
        )
        .await?;
        check(
            driver,
            json!({"action":"query","method":query.method,"args":operands(&query),"retained":true}),
            false,
            json!({"error":"host failure 雪"}),
        )
        .await?;
        let healthy = ask(driver, json!({"action":"query","method":query.method,"args":operands(&query),"retained":true,"control":{"update":supplied(&query)}}), false).await?;
        super::session_corpus::same(&healthy["ok"], &query.expected.value.unwrap_or(Value::Null));
        check(
            driver,
            json!({"action":"drop"}),
            false,
            json!({"ok":"dropped"}),
        )
        .await?;
    }
    assert_eq!(methods.len(), 13);
    Ok(())
}
/// Independent node query failure routes.
async fn node_failures(driver: &mut impl Driver) -> Result<(), String> {
    let tree = json!({"roots":[0],"nodes":[{"entry":{"type":"label","id":"l","parentId":null,"timestamp":"t","targetId":"entry"},"children":[]}]});
    check(driver, json!({"action":"retain-node","control":{"bind":{"tree":tree,"node_error":"node failure Ω"}}}), false, json!({"ok":"kept"})).await?;
    for method in ["entry", "children", "label", "labelTimestamp"] {
        check(
            driver,
            json!({"action":"node","method":method}),
            false,
            json!({"error":"node failure Ω"}),
        )
        .await?;
    }
    Ok(())
}
/// Selected decoding errors never turn into partial list success.
async fn malformed_entries(driver: &mut impl Driver) -> Result<(), String> {
    for (method, args, raw) in [
        ("getEntry", json!(["literal"]), "{"),
        ("getLeafEntry", json!([]), "{\"type\":\"custom\"}"),
        (
            "getEntries",
            json!([]),
            "[{\"type\":\"label\",\"id\":\"valid\",\"parentId\":null,\"timestamp\":\"t\",\"targetId\":\"x\"},{\"type\":\"custom\"}]",
        ),
        ("getBranch", json!([null]), "[false]"),
    ] {
        let reply = json!({"replies":[{"method":method,"args":args,"value":null,"raw":raw}]});
        check(
            driver,
            json!({"action":"retain","control":{"bind":reply}}),
            false,
            json!({"ok":"kept"}),
        )
        .await?;
        let actual = ask(
            driver,
            json!({"action":"query","method":method,"args":args,"retained":true}),
            false,
        )
        .await?;
        assert!(actual["error"].as_str().is_some());
        assert!(actual.get("ok").is_none());
        let reply = if ["getEntries", "getBranch"].contains(&method) {
            json!([])
        } else {
            Value::Null
        };
        check(driver, json!({"action":"query","method":method,"args":args,"retained":true,"control":{"update":{"replies":[{"method":method,"args":args,"value":reply}]}}}), false, json!({"ok":reply})).await?;
        check(
            driver,
            json!({"action":"drop"}),
            false,
            json!({"ok":"dropped"}),
        )
        .await?;
    }
    Ok(())
}
#[test]
fn maestro_reader_errors_are_catchable_without_partial_success() -> Result<(), String> {
    on_both_adapters!(errors)
}

/// Read a supplied selected header through the actual author getter.
async fn header(driver: &mut impl Driver, raw: String) -> Result<Value, String> {
    ask(driver, json!({"action":"query","method":"getHeader","control":{"bind":{"replies":[{"method":"getHeader","args":[],"value":null,"raw":raw}]}}}), false).await
}
/// Native header decoding retains presence, literal fields and qualified tag handling.
async fn headers(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let base =
        json!({"type":"session","id":"id 雪","timestamp":"not a date","cwd":" ../literal/雪 "});
    header_presence(driver, &base).await?;
    header_tags(driver, &base).await?;
    header_failures(driver, &base).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
/// Missing, null and finite values remain distinct after the public read.
async fn header_presence(driver: &mut impl Driver, base: &Value) -> Result<(), String> {
    for version in [None, Some(Value::Null), Some(json!(-0.0)), Some(json!(3.5))] {
        for parent in [
            None,
            Some(Value::Null),
            Some(json!("")),
            Some(json!("/parent/雪")),
        ] {
            let mut input = base.clone();
            if let Some(version) = &version {
                input["version"] = version.clone();
            }
            if let Some(parent) = &parent {
                input["parentSession"] = parent.clone();
            }
            let actual = header(driver, input.to_string()).await?;
            assert!(actual.get("error").is_none());
            super::session_corpus::same(&actual["ok"], &input);
        }
    }
    Ok(())
}
/// A selected header's incoming tag is not another selection policy.
async fn header_tags(driver: &mut impl Driver, base: &Value) -> Result<(), String> {
    for tag in [
        None,
        Some(Value::Null),
        Some(json!({"not":"a discriminator"})),
    ] {
        let mut input = base.clone();
        input.as_object_mut().unwrap().shift_remove("type");
        if let Some(tag) = tag {
            input["type"] = tag;
        }
        input["unread"] = json!({"anything":[false]});
        assert_eq!(header(driver, input.to_string()).await?, json!({"ok":base}));
    }
    Ok(())
}
/// Required record domains and native syntax diagnostics.
async fn header_failures(driver: &mut impl Driver, base: &Value) -> Result<(), String> {
    for field in ["id", "timestamp", "cwd", "version", "parentSession"] {
        for wrong in [None, Some(Value::Null), Some(json!({"wrong":true}))] {
            if ["version", "parentSession"].contains(&field)
                && wrong.as_ref().is_none_or(Value::is_null)
            {
                continue;
            }
            let mut input = base.clone();
            input.as_object_mut().unwrap().shift_remove(field);
            if let Some(wrong) = wrong {
                input[field] = wrong;
            }
            let result = header(driver, input.to_string()).await?;
            let error = result["error"].as_str().ok_or("expected header error")?;
            assert!(error.contains("missing field") || error.contains("invalid type"));
            assert!(result.get("ok").is_none());
            assert_eq!(header(driver, base.to_string()).await?, json!({"ok":base}));
        }
    }
    for (raw, message) in [
        (r#"["id","timestamp","cwd"]"#.to_owned(), "invalid type"),
        (
            base.to_string()
                .replace("\"id\":", "\"id\":\"first\",\"id\":"),
            "duplicate field `id`",
        ),
        ("{".to_owned(), "EOF"),
    ] {
        let result = header(driver, raw).await?;
        assert!(result["error"].as_str().unwrap().contains(message));
        assert!(result.get("ok").is_none());
    }
    Ok(())
}
#[test]
fn maestro_reader_header_preserves_presence_and_native_errors() -> Result<(), String> {
    on_both_adapters!(headers)
}
