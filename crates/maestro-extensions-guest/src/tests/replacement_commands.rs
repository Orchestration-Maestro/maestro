//! Replacement operands and ownership observed through both adapters.
use super::scenario::Driver;
use serde::Deserialize;
use serde_json::{Value, json};
/// Source-derived input with an independently recorded expected forwarding result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Operation selecting the public method.
    group: String,
    /// Authored inputs.
    input: Value,
    /// Independent source observation.
    expected: Value,
}
/// Strict corpus root.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Unique operation/input pairs.
    records: Vec<Case>,
}
/// Invoke one operation and isolate its own transcript.
async fn ask(driver: &mut impl Driver, input: Value, held: bool) -> Result<Vec<String>, String> {
    let start = driver.transcript().len();
    if held {
        driver
            .command_held("replacement", &input.to_string())
            .await?;
    } else {
        driver
            .run_plain_command("replacement", &input.to_string())
            .await?;
    }
    Ok(driver.transcript()[start..].to_vec())
}
/// Read exactly one tagged observation, rejecting absence and repetition.
fn one(lines: &[String], prefix: &str) -> Value {
    let values: Vec<Value> = lines
        .iter()
        .filter_map(|l| l.strip_prefix(prefix))
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(values.len(), 1, "observation {prefix}");
    values.into_iter().next().unwrap()
}
/// Absent and present-empty options must remain distinguishable at the host import.
async fn option_presence(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", "replacement-register")
        .await?;
    driver
        .run_plain_command("capture", "setup-register")
        .await?;
    driver
        .run_plain_command(
            "setup",
            &json!({"drop":true,"control":{"record_options":true}}).to_string(),
        )
        .await?;
    let mut observations = Vec::new();
    for option in [json!({"present":false}), json!({"present":true,"value":{}})] {
        let lines = ask(
            driver,
            json!({"group":"newSession","input":{"args":[option]}}),
            false,
        )
        .await?;
        observations.push(one(&lines, "setup-options "));
    }
    assert_ne!(
        observations[0], observations[1],
        "new-session import erased absent versus present-empty options"
    );
    observations.clear();
    for option in [json!({"present":false}), json!({"present":true,"value":{}})] {
        let lines = ask(driver, json!({"group":"switchSession","input":{"args":[{"present":true,"value":"/session"},option]}}), false).await?;
        observations.push(one(&lines, "replacement-call "));
    }
    assert_ne!(
        observations[0], observations[1],
        "switch-session import erased absent versus present-empty options"
    );
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}

#[test]
fn command_session_options_preserve_presence() -> Result<(), String> {
    on_both_adapters!(option_presence)
}

/// Source corpus owns command operands; explicit outcome cases own errors and pending results.
async fn forwarding(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", "replacement-register")
        .await?;
    driver
        .run_plain_command("capture", "setup-register")
        .await?;
    driver
        .run_plain_command(
            "setup",
            &json!({"drop":true,"control":{"record_options":true,"value":"written"}}).to_string(),
        )
        .await?;
    let corpus: Corpus = serde_json::from_str(include_str!("replacement_corpus.json")).unwrap();
    for case in corpus.records.into_iter().filter(|c| {
        matches!(
            c.group.as_str(),
            "fork" | "navigateTree" | "switchSession" | "reload" | "newSession"
        )
    }) {
        let lines = ask(
            driver,
            json!({"group":case.group,"input":case.input}),
            false,
        )
        .await?;
        if case.group == "newSession" {
            assert_new(&lines, &case.expected);
        } else {
            assert_command(&lines, &case.group, &case.expected);
        }
        let result = one(&lines, "entry replacement-result ");
        assert_eq!(
            result,
            json!({"ok":expected_value(&case.expected["result"]).unwrap_or(&Value::Null)})
        );
    }
    oracle_commands(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
/// Compare selected operation data, including absent versus empty fork/tree documents.
fn assert_command(lines: &[String], group: &str, expected: &Value) {
    let args = &expected["args"];
    let mut data = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| json!({"present":false}));
    let callback = expected_value(&data).is_some_and(|v| v.get("withSession").is_some());
    if let Some(object) = data.get_mut("value").and_then(Value::as_object_mut) {
        object.shift_remove("withSession");
    }
    assert_eq!(
        one(lines, "replacement-call "),
        json!({"group":group,"id":args.get(0).and_then(expected_value).and_then(Value::as_str).unwrap_or(""),"data":data,"callback":callback})
    );
    if callback {
        assert_eq!(
            one(lines, "entry replacement-target ")["cwd"],
            args[0]["value"]
        );
    }
}
/// New-session transport selects the three fields its source consumers read.
fn assert_new(lines: &[String], expected: &Value) {
    let options = expected_value(&expected["args"][0]).unwrap_or(&Value::Null);
    let record = one(lines, "setup-options ");
    assert_eq!(
        record,
        json!({"present":expected["args"][0]["present"],"parent":options["parentSession"],"setup":options.get("setup").is_some(),"continuation":options.get("withSession").is_some()})
    );
}
/// Recorded awaited results and stale host rejection are observed through the command facade.
async fn oracle_commands(driver: &mut impl Driver) -> Result<(), String> {
    let corpus: Corpus = serde_json::from_str(include_str!("replacement_corpus.json")).unwrap();
    for case in corpus
        .records
        .iter()
        .filter(|c| matches!(c.group.as_str(), "commandOutcome" | "stale"))
    {
        let method = case.input["method"].as_str().unwrap();
        let stale = case.group == "stale";
        let error = if stale {
            if case.input["message"]["present"] == true {
                assert_eq!(
                    case.expected["result"]["error"],
                    case.input["message"]["value"]
                );
            }
            case.expected["result"]["error"].clone()
        } else {
            case.input["outcome"]
                .as_str()
                .map_or(Value::Null, |e| json!(e))
        };
        let control = if method == "newSession" {
            json!({"setupControl":{"hold_return":!stale,"cancelled":case.input["outcome"] == true,"session_error":error}})
        } else {
            json!({"control":{"cancelled":case.input["outcome"] == true,"error":error,"stale":stale}})
        };
        let mut input = json!({"group":method,"input":{"args":[{"present":true,"value":if method=="newSession" {json!({"withSession":"callback"})} else {json!("target")}},{"present":true,"value":{"withSession":"callback"}}]}});
        input
            .as_object_mut()
            .unwrap()
            .extend(control.as_object().unwrap().clone());
        let lines = ask(driver, input, !stale).await?;
        let expected = &case.expected["result"];
        assert_eq!(
            one(&lines, "entry replacement-result "),
            if expected.as_object().unwrap().is_empty() {
                json!({"ok":null})
            } else {
                expected.clone()
            }
        );
        if stale {
            assert_eq!(case.expected["calls"], json!(0));
            assert!(!lines.iter().any(|l| l.starts_with("replacement-call ")
                || l.starts_with("new-session start ")
                || l.starts_with("entry replacement-target ")));
        } else {
            assert_eq!(case.expected["pendingBeforeRelease"], json!(true));
            assert!(
                lines
                    .iter()
                    .any(|l| l.starts_with("command suspended after "))
            );
        }
    }
    Ok(())
}
#[test]
fn maestro_command_options_and_errors_reach_the_bound_host() -> Result<(), String> {
    on_both_adapters!(forwarding)
}
/// Compare payloads, delivery choices and same-resource state through the public reader.
async fn sends(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", "replacement-register")
        .await?;
    let corpus: Corpus = serde_json::from_str(include_str!("replacement_corpus.json")).unwrap();
    for case in corpus
        .records
        .into_iter()
        .filter(|c| matches!(c.group.as_str(), "sendMessage" | "sendUserMessage"))
    {
        let lines = ask(
            driver,
            json!({"group":case.group,"input":case.input,"control":{}}),
            true,
        )
        .await?;
        let observed = one(&lines, "replacement-send ");
        let mut expected = case.expected["payload"].clone();
        if case.group == "sendMessage"
            && let Some(details) = expected.get_mut("details")
            && !details.is_null()
        {
            *details = json!(details.to_string());
        }
        assert_eq!(
            observed,
            json!({"group":case.group,"target":"A","payload":expected,"options":case.expected["options"]})
        );
        assert_eq!(
            one(&lines, "entry replacement-send-result "),
            json!({"ok":null})
        );
        let state = one(&lines, "entry replacement-state ");
        let state: Value = serde_json::from_str(state.as_str().unwrap()).unwrap();
        assert_eq!(state, json!([{"group":case.group,"payload":expected}]));
    }
    oracle_sends(driver).await?;
    retained(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
/// The captured target survives slot removal and cannot be redirected by publishing B.
async fn retained(driver: &mut impl Driver) -> Result<(), String> {
    ask(driver, json!({"drop":true,"control":{}}), false).await?;
    let baseline = driver.live();
    acquire(driver, "A", false).await?;
    let original = driver.live();
    ask(driver, json!({"clone":true}), false).await?;
    ask(driver, json!({"dropOriginal":true}), false).await?;
    assert_eq!(driver.live(), original);
    let corpus: Corpus = serde_json::from_str(include_str!("replacement_corpus.json")).unwrap();
    let case = corpus
        .records
        .iter()
        .find(|c| c.group == "boundSends")
        .unwrap();
    let active = case.input["active"].as_str().unwrap();
    acquire(driver, active, true).await?;
    bound_deliveries(driver).await?;
    bound_state(driver, active, &case.expected).await?;
    driver.replacement_sends_pair().await?;
    constructors(driver).await?;
    retained_operations(driver).await?;
    let owners = driver.live();
    ask(driver, json!({"drop":true,"control":{}}), false).await?;
    assert_eq!(driver.live(), baseline);
    assert_eq!(owners - baseline, 4);
    Ok(())
}
/// Obtain a replacement resource through its public continuation.
async fn acquire(driver: &mut impl Driver, target: &str, other: bool) -> Result<(), String> {
    ask(driver,json!({"group":"fork","input":{"args":[{"value":target},{"present":true,"value":{}}]},"retain":true,"target":if other {"B"} else {"A"},"control":{}}),false).await?;
    Ok(())
}
/// Deref routes retain their independent command projection.
async fn retained_command_views(driver: &mut impl Driver) -> Result<(), String> {
    for group in ["fork", "navigateTree", "switchSession", "reload"] {
        let lines=ask(driver,json!({"group":group,"retained":true,"input":{"args":[{"value":"literal Ω\n"},{"present":true,"value":{}}]},"control":{}}),false).await?;
        assert_eq!(one(&lines, "replacement-call ")["group"], json!(group));
        assert_eq!(one(&lines, "replacement-binding "), json!("A"));
        assert_eq!(
            one(&lines, "entry replacement-result "),
            json!({"ok":if group=="reload" {Value::Null} else {json!({"cancelled":false})}})
        );
    }
    Ok(())
}
/// An after-effect failure does not roll back the message already published on this target.
async fn after_effect(driver: &mut impl Driver) -> Result<(), String> {
    opaque_spellings(driver).await?;
    for group in ["sendMessage", "sendUserMessage"] {
        let payload = if group == "sendMessage" {
            json!({"customType":"opaque","content":"after","display":true,"details":{"z":1,"a":"Ω"}})
        } else {
            json!("after-user")
        };
        let lines=ask(driver,json!({"group":group,"retained":true,"opaque":"{ \"z\":1, \"a\":\"\\u03a9\" }","input":{"payload":payload,"options":{"present":false}},"control":{"after_error":"after effect Ω\n"}}),true).await?;
        assert_eq!(
            one(&lines, "entry replacement-send-result "),
            json!({"error":"after effect Ω\n"})
        );
        let sent = one(&lines, "replacement-send ");
        if group == "sendMessage" {
            assert_eq!(
                sent["payload"]["details"],
                json!("{ \"z\":1, \"a\":\"\\u03a9\" }")
            );
        }
        let state = one(&lines, "entry replacement-state ");
        let messages: Vec<Value> = serde_json::from_str(state.as_str().unwrap()).unwrap();
        assert_eq!(messages.last().unwrap()["payload"], sent["payload"]);
        let lines=ask(driver,json!({"group":group,"retained":true,"input":{"payload":payload,"options":{"present":false}},"control":{}}),true).await?;
        assert_eq!(
            one(&lines, "entry replacement-send-result "),
            json!({"ok":null})
        );
    }
    Ok(())
}
#[test]
fn maestro_replacement_sends_await_the_bound_target() -> Result<(), String> {
    on_both_adapters!(sends)
}
/// Distinct closures are delivered to the host-selected target; failures never re-invoke them.
async fn continuations(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", "replacement-register")
        .await?;
    driver
        .run_plain_command("capture", "setup-register")
        .await?;
    driver
        .run_plain_command(
            "setup",
            &json!({"drop":true,"control":{"value":"written","hold_return":true}}).to_string(),
        )
        .await?;
    driver.replacement_triple().await?;
    for group in ["fork", "switchSession"] {
        for (cancelled, unused, error) in [
            (false, false, None),
            (true, false, None),
            (false, true, None),
            (false, false, Some("author Ω\nnext")),
        ] {
            continuation_case(driver, group, (cancelled, unused, error)).await?;
        }
    }
    failure_after_setup(driver).await?;
    continuation_outcomes(driver).await?;
    lifecycle(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_replacement_continuations_keep_their_target_and_closure() -> Result<(), String> {
    on_both_adapters!(continuations)
}
/// Tool and shortcut callback declarations require ordinary contexts and use their supplied readers.
async fn ordinary(driver: &mut impl Driver) -> Result<(), String> {
    driver
        .run_plain_command("capture", "replacement-register")
        .await?;
    ask(driver, json!({"drop":true,"readerControl":{"bind":{"replies":[{"method":"getSessionName","args":[],"value":"shortcut-reader"}]},"update":null,"stale":null}}), false).await?;
    let start = driver.transcript().len();
    let handler = driver.identity("shortcut replacement-ordinary")?;
    driver.shortcut(handler, "/ordinary Ω").await?;
    assert_eq!(
        one(&driver.transcript()[start..], "entry ordinary-shortcut "),
        json!({"cwd":"/ordinary Ω","name":"shortcut-reader"})
    );
    let start = driver.transcript().len();
    ask(driver, json!({"drop":true,"readerControl":{"bind":{"replies":[{"method":"getSessionName","args":[],"value":"tool-reader"}]},"update":null,"stale":null}}), false).await?;
    let result = driver
        .tool(super::scenario::ToolRun {
            name: "replacement-ordinary",
            id: "ordinary",
            params: "{}",
            signal: None,
            update: false,
            held: false,
        })
        .await?;
    assert_eq!(
        one(&driver.transcript()[start..], "entry ordinary-tool "),
        json!({"cwd":"/work","name":"tool-reader"})
    );
    assert_eq!(
        serde_json::from_str::<Value>(&result).unwrap()["content"],
        json!([{"type":"text","text":"ordinary answer"}])
    );
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_command_capabilities_are_not_available_to_tools() -> Result<(), String> {
    on_both_adapters!(ordinary)
}
/// Distinct source operations with distinct closure values and setup state.
pub(super) fn triple_input(group: &str) -> String {
    let args = if group == "newSession" {
        json!([{"present":true,"value":{"parentSession":"A","setup":"setup","withSession":"callback"}}])
    } else {
        json!([{"value":if group=="fork" {"B"} else {"C"}},{"present":true,"value":{"withSession":"callback"}}])
    };
    json!({"group":group,"input":{"args":args},"closure":group,"control":{}}).to_string()
}
/// Reuse the existing completion driver to finish C, B, then A without timer observations.
pub(super) async fn triple<F: std::future::Future>(
    calls: [F; 3],
    barriers: [(
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ); 3],
) -> [F::Output; 3] {
    let [a, b, c] = calls;
    let [(entered_a, open_a), gate_b, gate_c] = barriers;
    let (done, finished) = tokio::sync::oneshot::channel();
    let main = async { (a.await, finished.await.unwrap()) };
    let helper = async {
        entered_a.await.unwrap();
        let outcomes =
            super::join::reversed(b, c, [gate_b, gate_c], |_| std::future::ready(())).await;
        open_a.send(()).unwrap();
        assert!(done.send(outcomes).is_ok());
    };
    let (a, (b, c)) = super::join::alongside(main, helper).await;
    [a, b, c]
}
/// Reverse completion preserves captured closures and the setup-mutated operation reader.
pub(super) fn triple_completed(lines: &[String]) {
    let targets: Vec<Value> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("entry replacement-target "))
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let readers: Vec<Value> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("entry replacement-reader "))
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(readers, vec![json!("C"), json!("B"), json!("setup-marker")]);
    assert_eq!(
        &targets,
        json!([
            {"cwd":"C","closure":"switchSession"},
            {"cwd":"B","closure":"fork"},
            {"cwd":"/replacement","closure":"newSession"}
        ])
        .as_array()
        .unwrap()
    );
}

/// Both retained send methods await their own result, preserve errors and recover.
async fn oracle_sends(driver: &mut impl Driver) -> Result<(), String> {
    let corpus: Corpus = serde_json::from_str(include_str!("replacement_corpus.json")).unwrap();
    for case in corpus.records.iter().filter(|c| c.group == "sendOutcome") {
        let method = case.input["method"].as_str().unwrap();
        let error = if case.input["failure"]["present"] == true {
            case.input["failure"]["value"].clone()
        } else {
            Value::Null
        };
        let payload = if method == "sendMessage" {
            json!({"customType":"outcome","content":"distinct custom","display":true})
        } else {
            json!("distinct user")
        };
        ask(driver, json!({"group":"fork","retain":true,"input":{"args":[{"value":"A"},{"present":true,"value":{}}]},"control":{}}), false).await?;
        let lines = ask(driver, json!({"group":method,"retained":true,"input":{"payload":payload,"options":{"present":false}},"control":{"error":error}}), true).await?;
        let result = &case.expected["result"];
        assert_eq!(
            one(&lines, "entry replacement-send-result "),
            if result.as_object().unwrap().is_empty() {
                json!({"ok":null})
            } else {
                result.clone()
            }
        );
        assert_eq!(case.expected["pendingBeforeRelease"], json!(true));
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("command suspended after replacement-send-start "))
        );
        let state = one(&lines, "entry replacement-state ");
        if error.is_null() {
            let state: Value = serde_json::from_str(state.as_str().unwrap()).unwrap();
            assert_eq!(state, json!([{"group":method,"payload":payload}]));
        } else {
            assert_eq!(state, json!("A"));
        }
    }
    Ok(())
}

/// Source continuation outcomes are compared after a closed command result.
async fn continuation_outcomes(driver: &mut impl Driver) -> Result<(), String> {
    let corpus: Corpus = serde_json::from_str(include_str!("replacement_corpus.json")).unwrap();
    for case in corpus
        .records
        .iter()
        .filter(|c| c.group == "continuationOutcome")
    {
        let callback = case.input["callback"].as_bool().unwrap();
        let options = if callback {
            json!({"withSession":"callback"})
        } else {
            json!({})
        };
        let error = if case.input["failure"]["present"] == true {
            case.input["failure"]["value"].clone()
        } else {
            Value::Null
        };
        let lines=ask(driver,json!({"group":"fork","input":{"args":[{"value":"A"},{"present":true,"value":options}]},"closure":"outcome","authorError":error,"control":{}}),false).await?;
        let outcome = one(&lines, "entry replacement-result ");
        assert_eq!(
            outcome,
            if case.expected["result"].as_object().unwrap().is_empty() {
                json!({"ok":{"cancelled":false}})
            } else {
                case.expected["result"].clone()
            }
        );
        let trace: Vec<Value> = lines
            .iter()
            .filter_map(|line| {
                if line == "replacement-rebound" {
                    Some(json!("rebound"))
                } else if let Some(target) = line.strip_prefix("entry replacement-target ") {
                    let target: Value = serde_json::from_str(target).unwrap();
                    Some(json!(format!(
                        "continuation {}",
                        target["cwd"].as_str().unwrap()
                    )))
                } else if line == "entry replacement-continued none" {
                    Some(json!("continued"))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(json!(trace), case.expected["trace"]);
    }
    Ok(())
}
/// Callback captures, callable slots and host owners are independently qualified.
async fn lifecycle(driver: &mut impl Driver) -> Result<(), String> {
    for group in ["fork", "switchSession"] {
        for mode in [
            "normal",
            "cancelled",
            "unused",
            "host-error",
            "before",
            "callback",
            "repeat",
            "host",
        ] {
            lifecycle_case(driver, group, mode).await?;
        }
    }
    invalid_continuations(driver).await?;
    let lines=ask(driver,json!({"group":"fork","input":{"args":[{"value":"reentry"},{"present":true,"value":{"withSession":"callback"}}]},"reenter":true,"control":{"unused":true}}),false).await?;
    assert!(lines.contains(&"register event replacement-reentrant".to_owned()));
    assert!(matches!(
        driver
            .deliver_input("event replacement-reentrant", "reentry")
            .await?
            .decision,
        super::scenario::Decision::Returned(Ok(None))
    ));
    assert!(driver.release("event replacement-reentrant").await);
    Ok(())
}
/// Missing, wrong-kind and released identities reject without a second author invocation.
async fn invalid_continuations(driver: &mut impl Driver) -> Result<(), String> {
    let released = driver.reborrow(*driver.dropped().last().unwrap())?;
    let wrong = driver.identity("command replacement")?;
    let missing = driver.unregistered()?;
    for id in [released, wrong, missing] {
        let logical = driver
            .callback_error(id)
            .rsplit(' ')
            .next()
            .unwrap()
            .to_owned();
        let start = driver.transcript().len();
        let live = driver.live();
        assert_eq!(
            driver.notify_continuation(id).await,
            Err(format!("no pending continuation for identity {logical}"))
        );
        assert_eq!(driver.transcript().len(), start);
        assert_eq!(driver.live(), live);
    }
    let dropped = driver.dropped().len();
    driver.repeat_release(released).await?;
    driver.repeat_release(released).await?;
    assert_eq!(driver.dropped().len(), dropped);
    Ok(())
}

/// Constructor-selected sends and command projections remain independent in both directions.
async fn constructors(driver: &mut impl Driver) -> Result<(), String> {
    for (target, sibling) in [("A", "B"), ("B", "A")] {
        for group in ["sendMessage", "sendUserMessage"] {
            let payload = if group == "sendMessage" {
                json!({"customType":"constructor","content":"constructor-custom","display":true})
            } else {
                json!("constructor-user")
            };
            let lines=ask(driver,json!({"group":group,"retained":true,"constructor":true,"target":target,"input":{"payload":payload,"options":{"present":false}},"control":{}}),true).await?;
            assert_eq!(one(&lines, "replacement-send ")["target"], json!(target));
            assert_eq!(one(&lines, "entry constructor-command "), json!(sibling));
            assert_eq!(one(&lines, "entry constructor-drop "), json!("owner"));
            let at = |prefix: &str| lines.iter().position(|l| l.starts_with(prefix)).unwrap();
            assert!(
                at("command suspended after replacement-send-start ") < at("replacement-send ")
            );
            assert!(at("replacement-send ") < at("entry replacement-send-result "));
        }
    }
    Ok(())
}

/// One closed continuation result owns its callback admission and exact error observations.
async fn continuation_case(
    driver: &mut impl Driver,
    group: &str,
    outcome: (bool, bool, Option<&str>),
) -> Result<(), String> {
    let (cancelled, unused, error) = outcome;
    let lines=ask(driver,json!({"group":group,"input":{"args":[{"value":"target Ω"},{"present":true,"value":{"withSession":"callback"}}]},"closure":group,"authorError":error,"control":{"cancelled":cancelled,"unused":unused}}),true).await?;
    if !cancelled && !unused {
        assert_eq!(
            one(&lines, "entry replacement-target "),
            json!({"cwd":"target Ω","closure":group})
        );
    } else {
        assert!(
            !lines
                .iter()
                .any(|l| l.starts_with("entry replacement-target "))
        );
    }
    let expected = error.filter(|_| !cancelled && !unused).map_or_else(
        || json!({"ok":{"cancelled":cancelled}}),
        |error| json!({"error":error}),
    );
    assert_eq!(one(&lines, "entry replacement-result "), expected);
    Ok(())
}

/// Read both retained resources to distinguish captured delivery from active-state routing.
async fn bound_state(
    driver: &mut impl Driver,
    active: &str,
    expected: &Value,
) -> Result<(), String> {
    for target in ["A", active] {
        let lines = ask(
            driver,
            json!({"group":"read","retained":true,"target":target,"control":{}}),
            false,
        )
        .await?;
        let state = one(&lines, "entry replacement-state ");
        if target == active {
            assert_eq!(state, json!(active));
            assert_eq!(expected[target], json!([]));
            continue;
        }
        let messages: Vec<Value> = serde_json::from_str(state.as_str().unwrap()).unwrap();
        let content: Vec<Value> = messages
            .iter()
            .map(|m| {
                if m["group"] == "sendMessage" {
                    m["payload"]["content"].clone()
                } else {
                    m["payload"].clone()
                }
            })
            .collect();
        assert_eq!(json!(content), expected[target]);
    }
    Ok(())
}

/// Drive each drop boundary through its named completion witnesses.
async fn lifecycle_case(driver: &mut impl Driver, group: &str, mode: &str) -> Result<(), String> {
    let live = driver.live();
    let dropped = driver.dropped().len();
    let input = json!({"group":group,"input":{"args":[{"value":"lifetime"},{"present":true,"value":{"withSession":"callback"}}]},"dropPhase":if matches!(mode,"before"|"callback"|"host") {json!(mode)} else {Value::Null},"control":{"repeat":mode=="repeat","hold_callback":mode=="callback","cancelled":mode=="cancelled","unused":matches!(mode,"unused"|"host"),"error":if mode=="host-error" {json!("rejected before continuation")} else {Value::Null}}});
    let start = driver.transcript().len();
    if mode == "host" {
        driver.host_drop(&input.to_string()).await?;
    } else if mode == "callback" {
        driver
            .drop_command("replacement", &input.to_string())
            .await?;
    } else {
        ask(driver, input, false).await?;
    }
    let lines = driver.transcript()[start..].to_vec();
    lifetime_observations(&lines, mode);
    assert_eq!(driver.live(), live);
    assert_eq!(
        driver.dropped().len() - dropped,
        usize::from(mode != "before")
    );
    Ok(())
}
/// Teardown distinguishes closure capture, admitted future and non-admitted callback.
fn lifetime_observations(lines: &[String], mode: &str) {
    let captures: Vec<Value> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("entry replacement-drop "))
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(captures, vec![json!("closure"), json!("future")]);
    if mode == "callback" {
        let at = |prefix: &str| lines.iter().position(|l| l.starts_with(prefix)).unwrap();
        assert!(at("entry replacement-drop \"closure\"") < at("entry replacement-target "));
        assert!(at("entry replacement-target ") < at("entry replacement-drop \"future\""));
    }
    if mode == "repeat" {
        let repeated = lines
            .iter()
            .find_map(|l| l.strip_prefix("replacement-repeat "))
            .unwrap();
        assert!(repeated.starts_with("no pending continuation for identity "));
        assert_eq!(
            lines
                .iter()
                .filter(|l| l.starts_with("entry replacement-target "))
                .count(),
            1
        );
    }
    if matches!(
        mode,
        "cancelled" | "unused" | "host-error" | "before" | "host"
    ) {
        assert!(
            !lines
                .iter()
                .any(|l| l.starts_with("entry replacement-target "))
        );
    }
}

/// Two distinguishable messages use the surviving alias after B is published.
async fn bound_deliveries(driver: &mut impl Driver) -> Result<(), String> {
    for (method, payload) in [
        (
            "sendMessage",
            json!({"customType":"bound","content":"A-custom","display":false}),
        ),
        ("sendUserMessage", json!("A-user")),
    ] {
        let lines=ask(driver,json!({"group":method,"retained":true,"input":{"payload":payload,"options":{"present":false}},"control":{}}),true).await?;
        assert_eq!(one(&lines, "replacement-send ")["target"], json!("A"));
    }
    Ok(())
}

/// A pending continuation error preserves the setup write on its retained operation reader.
async fn failure_after_setup(driver: &mut impl Driver) -> Result<(), String> {
    let lines=ask(driver,json!({"group":"newSession","input":{"args":[{"present":true,"value":{"parentSession":"before-marker","setup":"setup","withSession":"callback"}}]},"closure":"written-target","retain":true,"holdCallback":true,"authorError":"after await Ω\n","control":{"hold_callback":true},"setupControl":{"value":"setup-entry"}}),true).await?;
    assert_eq!(
        one(&lines, "entry replacement-reader "),
        json!("setup-marker")
    );
    assert_eq!(
        one(&lines, "entry replacement-result "),
        json!({"error":"after await Ω\n"})
    );
    let at = |prefix: &str| lines.iter().position(|l| l.starts_with(prefix)).unwrap();
    assert!(at("entry replacement-reader ") < at("command suspended after wait-for-idle"));
    assert!(at("command suspended after wait-for-idle") < at("entry replacement-result "));
    let lines = ask(
        driver,
        json!({"group":"read","retained":true,"control":{}}),
        false,
    )
    .await?;
    assert_eq!(
        one(&lines, "entry replacement-state "),
        json!("setup-marker")
    );
    ask(driver, json!({"drop":true}), false).await?;
    Ok(())
}

/// Empty opaque text and JSON null text remain present, without product-side parsing.
async fn opaque_spellings(driver: &mut impl Driver) -> Result<(), String> {
    for text in ["", "null"] {
        let lines=ask(driver,json!({"group":"sendMessage","retained":true,"opaque":text,"input":{"payload":{"customType":"opaque","content":"spelling","display":false,"details":{}},"options":{"present":false}},"control":{}}),true).await?;
        assert_eq!(
            one(&lines, "replacement-send ")["payload"]["details"],
            json!(text)
        );
    }
    Ok(())
}

/// Select a recorded present value without turning an absent result into present-empty data.
fn expected_value(record: &Value) -> Option<&Value> {
    record["present"]
        .as_bool()
        .unwrap()
        .then(|| &record["value"])
}

/// Additional retained-capability operations after the independent-target send witness.
async fn retained_operations(driver: &mut impl Driver) -> Result<(), String> {
    retained_command_views(driver).await?;
    after_effect(driver).await
}
/// Distinguishable custom and user payloads sent through separately retained targets.
pub(super) fn send_pair_input(target: &str) -> String {
    let (group, payload) = if target == "A" {
        (
            "sendMessage",
            json!({"customType":"paired","content":"paired A","display":false}),
        )
    } else {
        ("sendUserMessage", json!("paired B"))
    };
    json!({"group":group,"retained":true,"target":target,"input":{"payload":payload,"options":{"present":false}},"control":{}}).to_string()
}
/// Both admissions are closed; B's completion cannot release A's pending send.
pub(super) fn send_pair_phase(lines: &[String], second_completed: bool) {
    let sent: Vec<Value> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("replacement-send "))
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let starts: Vec<Value> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("replacement-send-start "))
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(
        starts
            .iter()
            .map(|v| v["target"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["A", "B"]
    );
    let results = lines
        .iter()
        .filter(|l| l.starts_with("entry replacement-send-result "))
        .count();
    if second_completed {
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["target"], json!("B"));
        assert_eq!(sent[0]["payload"], json!("paired B"));
        assert_eq!(results, 1);
    } else {
        assert!(sent.is_empty());
        assert_eq!(results, 0);
    }
}
/// Final observations come from the same two completed operations in reverse order.
pub(super) fn send_pair_completed(lines: &[String]) {
    let sent: Vec<Value> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("replacement-send "))
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(
        sent.iter()
            .map(|v| v["target"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["B", "A"]
    );
    assert_eq!(sent[0]["payload"], json!("paired B"));
    assert_eq!(sent[1]["payload"]["content"], json!("paired A"));
    assert_eq!(
        lines
            .iter()
            .filter(|l| l.starts_with("entry replacement-send-result "))
            .count(),
        2
    );
}
