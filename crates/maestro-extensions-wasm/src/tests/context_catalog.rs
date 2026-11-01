//! Live ordinary context capabilities on both adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::scenario::{Decision, Driver, transformed};
use serde_json::{Value, json};
/// Exact default stale-context diagnostic supplied by the host.
const STALE: &str = "This extension ctx is stale after session replacement or reload. Do not use a captured maestro or command ctx after ctx.newSession(), ctx.fork(), ctx.switchSession(), or ctx.reload(). For newSession, fork, and switchSession, move post-replacement work into withSession and use the ctx passed to withSession. For reload, do not use the old ctx after await ctx.reload().";
/// Install the author probes without changing the other scenarios.
async fn install(driver: &mut impl Driver) -> Result<(), String> {
    driver.run_plain_command("capture", "catalog").await
}
/// Execute a public operation and recover its returned answer.
async fn ask(driver: &mut impl Driver, request: Value, command: bool) -> Result<Value, String> {
    if command {
        driver
            .run_plain_command("catalog", &request.to_string())
            .await?;
        let text = driver
            .transcript()
            .into_iter()
            .rev()
            .find_map(|line| {
                line.strip_prefix("entry catalog-answer ")
                    .map(str::to_owned)
            })
            .ok_or("missing command answer")?;
        return serde_json::from_str(&text).map_err(|e| e.to_string());
    }
    let delivery = driver
        .deliver_input("event catalog", &request.to_string())
        .await?;
    transformed(delivery.decision)
}
/// Apply an acknowledged state mutation before creating the next context.
async fn control(driver: &mut impl Driver, value: Value) -> Result<(), String> {
    assert_eq!(
        ask(driver, json!({"action":"control","control":value}), false).await?,
        json!({"ok":"ack"})
    );
    Ok(())
}
/// Assert a single public getter result on the selected context view.
async fn getter(
    driver: &mut impl Driver,
    method: &str,
    command: bool,
    expected: Value,
) -> Result<(), String> {
    assert_eq!(
        ask(driver, json!({"action":"context","method":method}), command).await?,
        json!({"ok":expected}),
        "context route {method}"
    );
    Ok(())
}
/// A distinguishable full shared descriptor, not a guest projection.
fn model(id: &str) -> Value {
    json!({"id":id,"name":format!("model {id}"),"provider":"provider 雪","api":"custom-protocol","baseUrl":"not a URL","reasoning":true,"input":["image","text","image"],"cost":{"input":0.125,"output":2.5,"cacheRead":3.25,"cacheWrite":4.75},"contextWindow":8192.5,"maxTokens":1024.25,"compat":{"unknown":{"nested":[1,"雪",false]},"supportsStore":false}})
}
/// Controlled catalog replies preserve literal operands and selected raw records.
fn reply(method: &str, args: Value, value: Value) -> Value {
    Value::Object(
        [
            ("method".into(), json!(method)),
            ("args".into(), args),
            ("value".into(), value),
        ]
        .into_iter()
        .collect(),
    )
}
/// All eleven routes on ordinary, command and inherited replacement views.
async fn default_getters(driver: &mut impl Driver) -> Result<(), String> {
    for command in [false, true] {
        for (method, expected) in [
            ("ui", json!(true)),
            ("hasUI", json!(false)),
            ("modelRegistry", json!(true)),
            ("model", Value::Null),
            ("isIdle", json!(true)),
            ("signal", Value::Null),
            ("abort", Value::Null),
            ("hasPendingMessages", json!(false)),
            ("shutdown", Value::Null),
            ("getContextUsage", Value::Null),
            ("getSystemPrompt", json!("")),
        ] {
            getter(driver, method, command, expected.clone()).await?;
            if command {
                assert_eq!(
                    ask(
                        driver,
                        json!({"action":"replacement","method":method}),
                        true
                    )
                    .await?,
                    json!({"ok":expected})
                );
            }
        }
    }
    assert_eq!(
        driver
            .transcript()
            .iter()
            .filter(|l| l.as_str() == "context abort")
            .count(),
        3
    );
    assert_eq!(
        driver
            .transcript()
            .iter()
            .filter(|l| l.as_str() == "context shutdown")
            .count(),
        3
    );
    Ok(())
}
/// Run one public operation on the ordinary, command or replacement view.
async fn view(driver: &mut impl Driver, request: Value, kind: usize) -> Result<Value, String> {
    if kind == 2 {
        let mut request = request;
        request["action"] = json!("replacement");
        return ask(driver, request, true).await;
    }
    ask(driver, request, kind == 1).await
}
/// Present model, signal and catalog results differ per view and are not defaults.
async fn present_views(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    for (kind, name) in ["ordinary", "command", "replacement"]
        .into_iter()
        .enumerate()
    {
        let id = format!("{name} model");
        let flag = kind == 1;
        let tokens = f64::from(u8::try_from(kind).map_err(|e| e.to_string())?) + 0.5;
        control(
            driver,
            json!({"state":{"signal":flag,"prompt":name,"usage":{"tokens":tokens,"contextWindow":8192.0,"percent":null}},"model":{"replies":[reply("model",json!([]),model(&id))]}}),
        )
        .await?;
        for (method, expected) in [
            ("model", model(&id)),
            ("signal", json!(flag)),
            ("getSystemPrompt", json!(name)),
            (
                "getContextUsage",
                json!({"tokens":tokens,"contextWindow":8192.0,"percent":null,"signs":[false,false,null]}),
            ),
        ] {
            assert_eq!(
                view(driver, json!({"action":"context","method":method}), kind).await?,
                json!({"ok":expected}),
                "{name} {method}"
            );
        }
        view_catalog(driver, kind, name).await?;
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
/// Each view acquires its own catalog; duplicates survive in every list.
async fn view_catalog(driver: &mut impl Driver, kind: usize, name: &str) -> Result<(), String> {
    let (a, b) = (model(&format!("{name} a")), model(&format!("{name} b")));
    let found = model(&format!("{name} found"));
    let args = json!(["provider 雪", "id"]);
    for (method, call, value) in [
        ("getAll", json!([]), json!([a, b, a, a])),
        ("getAvailable", json!([]), json!([b, a, b, b])),
        ("find", args.clone(), found),
    ] {
        control(
            driver,
            json!({"registry":{"replies":[reply(method,call.clone(),value.clone())]}}),
        )
        .await?;
        assert_eq!(
            view(
                driver,
                json!({"action":"catalog","method":method,"args":call}),
                kind
            )
            .await?,
            json!({"ok":value}),
            "{name} {method}"
        );
    }
    Ok(())
}
#[test]
fn maestro_present_values_reach_every_context_view() -> Result<(), String> {
    on_both_adapters!(present_views)
}
/// Repeated getters read current state, with independently supplied booleans.
async fn live_getters(driver: &mut impl Driver) -> Result<(), String> {
    for command in [false, true] {
        assert_eq!(
            ask(driver, json!({"action":"keep-context"}), command).await?,
            json!({"ok":"kept"})
        );
        live_flags(driver, command).await?;
        for prompt in [
            "",
            "base\nfirst",
            "base\nfirst\nsecond",
            "\u{feff}prompt\u{85}",
        ] {
            control(driver, json!({"state":{"prompt":prompt}})).await?;
            getter(driver, "getSystemPrompt", command, json!(prompt)).await?;
            assert_eq!(
                ask(
                    driver,
                    json!({"action":"old-context","method":"getSystemPrompt"}),
                    command
                )
                .await?,
                json!({"ok":prompt})
            );
        }
        control(
            driver,
            json!({"state":{"usage":{"tokens":null,"contextWindow":8192.0,"percent":null}}}),
        )
        .await?;
        getter(
            driver,
            "getContextUsage",
            command,
            json!({"tokens":null,"contextWindow":8192.0,"percent":null,"signs":[null,false,null]}),
        )
        .await?;
    }
    Ok(())
}
/// Idle and queued flags are independent on the selected context view.
async fn live_flags(driver: &mut impl Driver, command: bool) -> Result<(), String> {
    for idle in [false, true] {
        for queued in [false, true] {
            control(
                driver,
                json!({"state":{"idle":idle,"queued":queued,"hasUI":true}}),
            )
            .await?;
            getter(driver, "isIdle", command, json!(idle)).await?;
            getter(driver, "hasPendingMessages", command, json!(queued)).await?;
            getter(driver, "hasUI", command, json!(true)).await?;
        }
    }
    Ok(())
}
/// A retained context captures the supplier, not its returned model.
async fn captured_model(driver: &mut impl Driver) -> Result<(), String> {
    control(
        driver,
        json!({"model":{"replies":[reply("model",json!([]),model("old"))]}}),
    )
    .await?;
    assert_eq!(
        ask(driver, json!({"action":"keep-context"}), false).await?,
        json!({"ok":"kept"})
    );
    for id in ["old", "changed"] {
        control(
            driver,
            json!({"model_update":{"replies":[reply("model",json!([]),model(id))]}}),
        )
        .await?;
        assert_eq!(
            ask(
                driver,
                json!({"action":"old-context","method":"model"}),
                false
            )
            .await?,
            json!({"ok":model(id)})
        );
    }
    control(
        driver,
        json!({"model":{"replies":[reply("model",json!([]),model("replacement-function"))]}}),
    )
    .await?;
    assert_eq!(
        ask(
            driver,
            json!({"action":"old-context","method":"model"}),
            false
        )
        .await?,
        json!({"ok":model("changed")})
    );
    getter(driver, "model", false, model("replacement-function")).await?;
    Ok(())
}
/// Import failures are caught, retain their first message and do not trap the instance.
async fn failed_getters(driver: &mut impl Driver) -> Result<(), String> {
    let methods = [
        "ui",
        "hasUI",
        "modelRegistry",
        "model",
        "isIdle",
        "signal",
        "abort",
        "hasPendingMessages",
        "shutdown",
        "getContextUsage",
        "getSystemPrompt",
    ];
    for message in [STALE, "stale 雪", "host failure 雪"] {
        control(driver, json!({"reset":true,"stale":message})).await?;
        control(driver, json!({"stale":"must not replace first error"})).await?;
        for command in [false, true] {
            for method in methods {
                assert_eq!(
                    ask(driver, json!({"action":"context","method":method}), command).await?,
                    json!({"error":message})
                );
            }
        }
        control(driver, json!({"reset":true,"state":{}})).await?;
        getter(driver, "isIdle", false, json!(true)).await?;
    }
    Ok(())
}
/// Observe defaults, live state, captured supplier and catchable errors.
async fn getters(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    default_getters(driver).await?;
    live_getters(driver).await?;
    captured_model(driver).await?;
    failed_getters(driver).await?;
    assert_eq!(
        ask(driver, json!({"action":"drop"}), false).await?,
        json!({"ok":"dropped"})
    );
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_context_getters_remain_live_and_errors_are_catchable() -> Result<(), String> {
    on_both_adapters!(getters)
}
/// Cancellation remains independently retained after callback completion and replacement.
async fn signals(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let baseline = driver.live();
    control(driver, json!({"state":{"signal":false}})).await?;
    assert_eq!(
        ask(driver, json!({"action":"signal","method":"retain"}), false).await?,
        json!({"ok":"done"})
    );
    assert_eq!(driver.live(), baseline + 1);
    assert_eq!(
        ask(driver, json!({"action":"signal","method":"alias"}), false).await?,
        json!({"ok":"done"})
    );
    assert_eq!(driver.live(), baseline + 1);
    changed_signals(driver, baseline).await
}
/// Observe the old flag and release its aliases independently of the new flag.
async fn changed_signals(driver: &mut impl Driver, baseline: usize) -> Result<(), String> {
    control(driver, json!({"state":{"signal":false}})).await?;
    getter(driver, "signal", false, json!(false)).await?;
    assert_eq!(
        ask(driver, json!({"action":"signal","method":"read"}), false).await?,
        json!({"ok":false})
    );
    control(driver, json!({"cancel_old":true,"stale":"stale 雪"})).await?;
    assert_eq!(
        ask(driver, json!({"action":"signal","method":"read"}), false).await?,
        json!({"ok":true})
    );
    assert_eq!(
        ask(driver, json!({"action":"signal","method":"retain"}), false).await?,
        json!({"error":"stale 雪"})
    );
    release_signals(driver, baseline).await
}
/// Dropping aliases leaves the new host flag untouched.
async fn release_signals(driver: &mut impl Driver, baseline: usize) -> Result<(), String> {
    for expected in [baseline + 1, baseline] {
        assert_eq!(
            ask(driver, json!({"action":"signal","method":"pop"}), false).await?,
            json!({"ok":"done"})
        );
        assert_eq!(driver.live(), expected);
    }
    control(driver, json!({"reset":true})).await?;
    getter(driver, "signal", false, json!(false)).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_signal_outlives_callback_return() -> Result<(), String> {
    on_both_adapters!(signals)
}
/// Preserve ordered catalog lists and literal lookup operands.
async fn catalog_lists(driver: &mut impl Driver) -> Result<(), String> {
    let mut second = model("z");
    second["name"] = json!("second match");
    let ordered = json!([model("z"), model("a"), second, model("z")]);
    for (method, value) in [
        ("getAll", ordered.clone()),
        ("getAvailable", json!([model("a"), model("z")])),
        ("getAll", json!([])),
        ("getAvailable", json!([])),
    ] {
        control(
            driver,
            json!({"registry":{"replies":[reply(method,json!([]),value.clone())]}}),
        )
        .await?;
        assert_eq!(
            ask(driver, json!({"action":"catalog","method":method}), false).await?,
            json!({"ok":value})
        );
    }
    for (provider, id, found) in [
        ("provider 雪", "z", true),
        ("", "", false),
        ("PROVIDER 雪", "z", false),
        ("provider 雪", "Z", false),
        (" provider 雪", "z", false),
        ("provider 雪", " z", false),
        ("\u{feff}provider 雪", "z", false),
        ("provider 雪", "z\u{85}", false),
    ] {
        let value = if found { model("z") } else { Value::Null };
        let args = json!([provider, id]);
        control(
            driver,
            json!({"registry":{"replies":[reply("find",args.clone(),value.clone())]}}),
        )
        .await?;
        assert_eq!(
            ask(
                driver,
                json!({"action":"catalog","method":"find","args":args}),
                false
            )
            .await?,
            json!({"ok":value})
        );
    }
    Ok(())
}
/// Authentication preserves optional keys, literal ordered headers and structured denials.
async fn auth_outcomes(driver: &mut impl Driver) -> Result<(), String> {
    let input = model("not in catalog");
    let headers = json!([
        ["2", "two"],
        ["1", "one"],
        ["x-Key", "lower"],
        ["X-Key", "upper"],
        ["X-Empty", ""]
    ]);
    let missing = "No API key found for \"provider 雪\"";
    let shell =
        "Failed to resolve provider \"provider 雪\" header \"X-A\" from shell command: exit 1";
    for value in [
        json!({}),
        json!({"apiKey":""}),
        json!({"apiKey":"sample key 雪"}),
        json!({"headers":headers}),
        json!({"headers":[]}),
        json!({"error":missing}),
        json!({"error":"auth failed 雪"}),
        json!({"error":""}),
        json!({"error":"auth failed 雪\nretry"}),
        json!({"error":shell}),
    ] {
        control(
            driver,
            json!({"registry":{"replies":[reply("auth",json!([input]),value.clone())]}}),
        )
        .await?;
        let actual = ask(
            driver,
            json!({"action":"catalog","method":"auth","args":[input]}),
            false,
        )
        .await?;
        let expected = if value.get("error").is_some() {
            json!({"ok":value})
        } else {
            json!({"ok":{"apiKey":value["apiKey"],"headers":value["headers"]}})
        };
        assert!(actual == expected, "authentication outcome is preserved");
    }
    import_failures(driver, &input).await
}
/// Every catalog route recovers on the same retained handle after an import failure.
async fn import_failures(driver: &mut impl Driver, input: &Value) -> Result<(), String> {
    for method in ["getAll", "getAvailable", "find", "auth"] {
        control(driver, json!({"registry":{"error":"outer host error"}})).await?;
        retain_catalog(driver).await?;
        let args = match method {
            "auth" => json!([input]),
            "find" => json!(["provider 雪", "z"]),
            _ => json!([]),
        };
        let request = json!({"action":"catalog","method":method,"args":args,"retained":true});
        assert!(
            ask(driver, request.clone(), false).await? == json!({"error":"outer host error"}),
            "host failure remains recoverable"
        );
        let value = match method {
            "auth" => json!({}),
            "find" => Value::Null,
            _ => json!([]),
        };
        control(
            driver,
            json!({"registry_update":{"replies":[reply(method,args,value.clone())]}}),
        )
        .await?;
        let expected = if method == "auth" {
            json!({"ok":{"apiKey":null,"headers":null}})
        } else {
            json!({"ok":value})
        };
        assert!(
            ask(driver, request, false).await? == expected,
            "same catalog route recovers"
        );
        pop_catalog(driver).await?;
    }
    Ok(())
}
/// Keep the acquired catalog before a fallible query.
async fn retain_catalog(driver: &mut impl Driver) -> Result<(), String> {
    assert_eq!(
        ask(driver, json!({"action":"catalog","method":"retain"}), false).await?,
        json!({"ok":"done"})
    );
    Ok(())
}
/// Release the final alias after the same-handle recovery witness.
async fn pop_catalog(driver: &mut impl Driver) -> Result<(), String> {
    assert_eq!(
        ask(driver, json!({"action":"catalog","method":"pop"}), false).await?,
        json!({"ok":"done"})
    );
    Ok(())
}
/// Four catalog routes transport live host results without filtering or normalization.
async fn catalog_queries(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    catalog_lists(driver).await?;
    auth_outcomes(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_catalog_queries_preserve_auth_outcomes() -> Result<(), String> {
    on_both_adapters!(catalog_queries)
}
/// Acquired capabilities retain their own owner when bindings change or context acquisition fails.
async fn retained_capabilities(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let baseline = driver.live();
    for method in ["retain", "alias"] {
        assert_eq!(
            ask(driver, json!({"action":"ui","method":method}), false).await?,
            json!({"ok":"done"})
        );
        assert_eq!(driver.live(), baseline + 1);
    }
    control(
        driver,
        json!({"registry":{"replies":[reply("getAll",json!([]),json!([model("first")]))]}}),
    )
    .await?;
    for (method, retained) in [("retain", false), ("alias", true)] {
        assert_eq!(
            ask(
                driver,
                json!({"action":"catalog","method":method,"retained":retained}),
                false
            )
            .await?,
            json!({"ok":"done"})
        );
        assert_eq!(driver.live(), baseline + 2);
    }
    rebound_capabilities(driver, baseline).await
}
/// A retained catalog remains live without following a replacement binding.
async fn rebound_capabilities(driver: &mut impl Driver, baseline: usize) -> Result<(), String> {
    control(driver,json!({"registry_update":{"replies":[reply("getAll",json!([]),json!([model("changed")]))]}})).await?;
    assert_eq!(
        ask(
            driver,
            json!({"action":"catalog","method":"getAll","retained":true}),
            false
        )
        .await?,
        json!({"ok":[model("changed")]})
    );
    control(
        driver,
        json!({"registry":{"replies":[reply("getAll",json!([]),json!([model("rebound")]))]}}),
    )
    .await?;
    assert_eq!(
        ask(driver, json!({"action":"catalog","method":"getAll"}), false).await?,
        json!({"ok":[model("rebound")]})
    );
    assert_eq!(
        ask(driver, json!({"action":"ui","method":"retain"}), false).await?,
        json!({"ok":"done"})
    );
    assert_eq!(driver.live(), baseline + 3);
    stale_capabilities(driver, baseline).await
}
/// Staleness rejects acquisition, not already retained resources.
async fn stale_capabilities(driver: &mut impl Driver, baseline: usize) -> Result<(), String> {
    control(driver, json!({"stale":"stale 雪"})).await?;
    for action in ["ui", "catalog"] {
        assert_eq!(
            ask(driver, json!({"action":action,"method":"retain"}), false).await?,
            json!({"error":"stale 雪"})
        );
    }
    assert_eq!(
        ask(
            driver,
            json!({"action":"catalog","method":"getAll","retained":true}),
            false
        )
        .await?,
        json!({"ok":[model("changed")]})
    );
    release_capabilities(driver, baseline).await
}
/// Each final alias releases only its own resource.
async fn release_capabilities(driver: &mut impl Driver, baseline: usize) -> Result<(), String> {
    for (action, expected) in [
        ("ui", baseline + 2),
        ("ui", baseline + 2),
        ("ui", baseline + 1),
        ("catalog", baseline + 1),
        ("catalog", baseline),
    ] {
        assert_eq!(
            ask(driver, json!({"action":action,"method":"pop"}), false).await?,
            json!({"ok":"done"})
        );
        assert_eq!(driver.live(), expected);
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_retained_context_capabilities_release_independently() -> Result<(), String> {
    on_both_adapters!(retained_capabilities)
}
/// A named completion barrier witnesses pending ownership and all three authentication outcomes.
async fn pending_auth(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let baseline = driver.live();
    let input = model("noncatalog pending");
    for (value, outer) in [
        (json!({"apiKey":"","headers":[["X-Empty",""]]}), false),
        (json!({"error":"pending auth failure 雪"}), false),
        (Value::Null, true),
    ] {
        let data = if outer {
            json!({"error":"pending outer failure 雪"})
        } else {
            json!({"replies":[reply("auth",json!([input]),value.clone())]})
        };
        control(driver, json!({"registry":data})).await?;
        let handler = driver.identity("event catalog")?;
        let request = json!({"action":"catalog","method":"auth","args":[input]});
        let event = json!({"type":"input","text":request.to_string(),"source":"rpc"});
        let delivery = driver
            .deliver_held(handler, &event.to_string(), "/work")
            .await?;
        let actual = transformed(delivery.decision)?;
        let expected = if outer {
            json!({"error":"pending outer failure 雪"})
        } else {
            json!({"ok":value})
        };
        assert!(actual == expected, "pending authentication outcome");
        let transcript = driver.transcript();
        let started = transcript
            .iter()
            .rposition(|line| line == "auth started")
            .ok_or("auth never started")?;
        assert_eq!(
            transcript[started + 1],
            "command suspended after auth started"
        );
        assert_eq!(
            transcript[started + 2],
            format!("auth pending resources {}", baseline + 2)
        );
        assert_eq!(transcript[started + 3], "auth completed");
        assert_eq!(driver.live(), baseline);
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_catalog_auth_waits_for_completion_and_releases() -> Result<(), String> {
    on_both_adapters!(pending_auth)
}
/// The registered compaction callback uses preparation, branch entries and live reader/catalog results.
async fn compaction(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    control(
        driver,
        json!({"registry":{"replies":[reply("getAll",json!([]),json!([model("z"),model("a")]))]}}),
    )
    .await?;
    assert_eq!(
        ask(driver, json!({"action":"control","control":{}}), false).await?,
        json!({"ok":"ack"})
    );
    driver.run_plain_command("capture", "reader").await?;
    let reader = json!({"action":"retain","control":{"bind":{"replies":[reply("getLeafId",json!([]),json!("live leaf 雪"))]}}});
    let delivery = driver
        .deliver_input("event reader", &reader.to_string())
        .await?;
    assert!(matches!(delivery.decision, Decision::Returned(Ok(Some(_)))));
    let mut event = super::documents::document("session_before_compact");
    event["preparation"]["firstKeptEntryId"] = json!("keep from preparation");
    event["preparation"]["tokensBefore"] = json!(4321.5);
    event["preparation"]["messagesToSummarize"] = json!([
        {"role":"user","content":"first request 雪","timestamp":1.0},
        {"role":"user","content":[{"type":"text","text":"complex request"}],"timestamp":2.0}
    ]);
    let branch = json!({"type":"session_info","id":"branch-id","parentId":null,"timestamp":"literal timestamp","name":"branch name"});
    event["branchEntries"] = json!([branch]);
    let handler = driver.identity("event catalog-compaction")?;
    let signal = driver.lend_signal(false)?;
    let delivered = driver
        .deliver(handler, &event.to_string(), "/work", Some(signal))
        .await?;
    assert_eq!(
        serde_json::from_str::<Value>(delivered.event?.ok_or("missing event")?.as_str())
            .map_err(|e| e.to_string())?,
        event
    );
    let Decision::Returned(Ok(Some(result))) = delivered.decision else {
        return Err("compaction callback did not return content".into());
    };
    let result: Value = serde_json::from_str(&result).map_err(|e| e.to_string())?;
    assert_eq!(
        result,
        json!({"compaction":{"summary":"User requests:\n- first request 雪\n- [complex]","firstKeptEntryId":"keep from preparation","tokensBefore":4321.5,"details":{"leaf":"live leaf 雪","models":["model z","model a"],"branchEntries":[branch]}}})
    );
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_compaction_handler_uses_preparation_and_returns_content() -> Result<(), String> {
    on_both_adapters!(compaction)
}
/// One pinned decoder boundary observation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DecoderCase {
    /// Case label without record contents.
    name: String,
    /// Original selected wire text, preserving duplicate members and unread depth.
    raw: String,
    /// Independently measured native result.
    native: NativeResult,
}
/// Selected native result; each optional field is read in its owning branch.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeResult {
    /// Accepted value or decoding error.
    kind: String,
    /// Accepted full shared model.
    value: Option<Value>,
    /// Original diagnostic for the selected model.
    error: Option<String>,
    /// Input-cost zero sign, independent of JSON equality.
    input_cost_negative_zero: Option<bool>,
}
/// Exercise raw selected records through the public getter and catalog routes.
async fn decoder_cases(driver: &mut impl Driver) -> Result<(), String> {
    let cases: Vec<DecoderCase> =
        serde_json::from_str(include_str!("catalog_decoder.json")).map_err(|e| e.to_string())?;
    assert_eq!(cases.len(), 70);
    let mut accepted = 0;
    for case in cases {
        accepted += usize::from(case.native.kind == "value");
        selected_case(driver, &case).await?;
        for method in ["getAll", "getAvailable", "find"] {
            catalog_case(driver, &case, method).await?;
        }
    }
    assert_eq!(accepted, 17);
    Ok(())
}
/// Original JSON reaches the selected-record decoder, preserving its exact diagnostic.
async fn selected_case(driver: &mut impl Driver, case: &DecoderCase) -> Result<(), String> {
    let selected = json!({"method":"model","args":[],"value":null,"raw":case.raw});
    control(driver, json!({"model":{"replies":[selected]}})).await?;
    assert_eq!(
        ask(driver, json!({"action":"keep-context"}), false).await?,
        json!({"ok":"kept"})
    );
    let actual = ask(
        driver,
        json!({"action":"old-context","method":"model"}),
        false,
    )
    .await?;
    if case.native.kind == "value" {
        assert!(case.native.error.is_none());
        assert_eq!(
            actual,
            json!({"ok":case.native.value}),
            "decoder case {}",
            case.name
        );
        assert_eq!(
            actual["ok"]["cost"]["input"]
                .as_f64()
                .ok_or("missing input cost")?
                .is_sign_negative(),
            case.native
                .input_cost_negative_zero
                .ok_or("missing zero sign")?,
            "zero sign case {}",
            case.name
        );
    } else {
        assert_eq!(case.native.kind, "error");
        assert!(case.native.value.is_none() && case.native.input_cost_negative_zero.is_none());
        assert_eq!(
            actual,
            json!({"error":case.native.error}),
            "decoder case {}",
            case.name
        );
        control(driver,json!({"model_update":{"replies":[reply("model",json!([]),model("healthy after decode error"))]}})).await?;
        assert_eq!(
            ask(
                driver,
                json!({"action":"old-context","method":"model"}),
                false
            )
            .await?,
            json!({"ok":model("healthy after decode error")})
        );
    }
    Ok(())
}
/// List failures never become partial results or poison a later healthy call.
async fn catalog_case(
    driver: &mut impl Driver,
    case: &DecoderCase,
    method: &str,
) -> Result<(), String> {
    let args = if method == "find" {
        json!(["literal provider", "literal id"])
    } else {
        json!([])
    };
    let raw = if method == "find" {
        case.raw.clone()
    } else {
        format!("[{},{}]", model("valid first"), case.raw)
    };
    control(
        driver,
        json!({"registry":{"replies":[{"method":method,"args":args,"value":null,"raw":raw}]}}),
    )
    .await?;
    retain_catalog(driver).await?;
    let actual = ask(
        driver,
        json!({"action":"catalog","method":method,"args":args,"retained":true}),
        false,
    )
    .await?;
    if case.native.kind == "value" {
        let expected = if method == "find" {
            json!({"ok":case.native.value})
        } else {
            json!({"ok":[model("valid first"),case.native.value]})
        };
        assert_eq!(
            actual, expected,
            "catalog decoder case {} route {method}",
            case.name
        );
    } else {
        assert!(
            actual["error"].as_str().is_some_and(|e| !e.is_empty()),
            "catalog decoder case {} route {method}",
            case.name
        );
        recover_selected_catalog(driver, method, &args).await?;
    }
    pop_catalog(driver).await
}
/// The same retained catalog decodes a healthy replacement after an earlier selected-record failure.
async fn recover_selected_catalog(
    driver: &mut impl Driver,
    method: &str,
    args: &Value,
) -> Result<(), String> {
    let value = if method == "find" {
        Value::Null
    } else {
        json!([])
    };
    control(
        driver,
        json!({"registry_update":{"replies":[reply(method,args.clone(),value.clone())]}}),
    )
    .await?;
    assert_eq!(
        ask(
            driver,
            json!({"action":"catalog","method":method,"args":args,"retained":true}),
            false
        )
        .await?,
        json!({"ok":value})
    );
    Ok(())
}
/// Usage numbers preserve fractional values and zero signs without recomputing the percentage.
async fn usage_numbers(driver: &mut impl Driver) -> Result<(), String> {
    for (tokens, window, percent) in [
        (1.25_f64, 256.5_f64, 0.487_329_434_697_855_7_f64),
        (0.0, 0.0, 0.0),
        (-0.0, -0.0, -0.0),
    ] {
        control(
            driver,
            json!({"state":{"usage":{"tokens":tokens,"contextWindow":window,"percent":percent}}}),
        )
        .await?;
        getter(driver,"getContextUsage",false,json!({"tokens":tokens,"contextWindow":window,"percent":percent,"signs":[tokens.is_sign_negative(),window.is_sign_negative(),percent.is_sign_negative()]})).await?;
    }
    Ok(())
}
/// Auth uses the supplied full shared descriptor and rejects authored nonfinite data before import.
async fn authored_records(driver: &mut impl Driver) -> Result<(), String> {
    let descriptor = super::selection_events::model();
    control(
        driver,
        json!({"registry":{"replies":[reply("auth",json!([descriptor]),json!({"apiKey":""}))]}}),
    )
    .await?;
    assert!(
        ask(
            driver,
            json!({"action":"catalog","method":"auth","args":[descriptor]}),
            false
        )
        .await?
            == json!({"ok":{"apiKey":"","headers":null}}),
        "shared auth input preserved"
    );
    let before = driver
        .transcript()
        .iter()
        .filter(|l| l.as_str() == "auth started")
        .count();
    assert!(
        ask(
            driver,
            json!({"action":"catalog","method":"auth","args":[descriptor],"nonfinite":true}),
            false
        )
        .await?
            == json!({"error":"extension wrote a non-finite number (Infinity or NaN)"}),
        "nonfinite authored model rejected"
    );
    assert_eq!(
        driver
            .transcript()
            .iter()
            .filter(|l| l.as_str() == "auth started")
            .count(),
        before
    );
    control(driver,json!({"model":{"error":"host failure before decode","replies":[{"method":"model","args":[],"value":null,"raw":"{"}]}})).await?;
    assert_eq!(
        ask(driver, json!({"action":"context","method":"model"}), false).await?,
        json!({"error":"host failure before decode"})
    );
    Ok(())
}
/// New routes reuse the shared model decoder and the supplied usage representation.
async fn shared_records(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    decoder_cases(driver).await?;
    usage_numbers(driver).await?;
    complete_descriptor(driver).await?;
    authored_records(driver).await?;
    compatibility_corpus(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_context_and_catalog_records_keep_shared_semantics() -> Result<(), String> {
    on_both_adapters!(shared_records)
}

/// Shared fixture fields used by their owning decoder and these new transport consumers.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CompatibilityCase {
    /// Supplied shared descriptor.
    input: Value,
    /// Independently observed protocol and open compatibility fields.
    expected: Value,
}
/// Reuse the owning shared-model corpus rather than copying descriptors.
async fn compatibility_corpus(driver: &mut impl Driver) -> Result<(), String> {
    let cases: Vec<CompatibilityCase> = serde_json::from_str(include_str!(
        "../../../maestro-request/tests/fixtures/model_compatibility.json"
    ))
    .map_err(|e| e.to_string())?;
    let mut seen = std::collections::HashSet::new();
    for case in cases {
        if !seen.insert(case.input.to_string()) {
            continue;
        }
        transport_compatibility(driver, &case).await?;
    }
    assert_eq!(seen.len(), 222);
    Ok(())
}
/// Each new selected-record route carries the same open compatibility fields.
async fn transport_compatibility(
    driver: &mut impl Driver,
    case: &CompatibilityCase,
) -> Result<(), String> {
    control(driver,json!({"model":{"replies":[reply("model",json!([]),case.input.clone())]},"registry":{"replies":[reply("getAll",json!([]),json!([case.input])),reply("getAvailable",json!([]),json!([case.input])),reply("find",json!(["literal provider","literal id"]),case.input.clone())]}})).await?;
    let ctx = ask(driver, json!({"action":"context","method":"model"}), false).await?;
    compare_compatibility(&ctx["ok"], &case.expected)?;
    for method in ["getAll", "getAvailable", "find"] {
        let returned = ask(
            driver,
            json!({"action":"catalog","method":method,"args":["literal provider","literal id"]}),
            false,
        )
        .await?;
        let actual = if method == "find" {
            &returned["ok"]
        } else {
            &returned["ok"][0]
        };
        compare_compatibility(actual, &case.expected)?;
    }
    Ok(())
}
/// Read every expected field and prove nested key order using the owning assertion.
fn compare_compatibility(actual: &Value, expected: &Value) -> Result<(), String> {
    for (key, value) in expected
        .as_object()
        .ok_or("expected shared compatibility object")?
    {
        super::selection_events::assert_order(&actual[key], value);
        super::session_corpus::same(&actual[key], value);
    }
    Ok(())
}

/// The complete descriptor retains all shared fields, header order and numeric bits on new getters.
async fn complete_descriptor(driver: &mut impl Driver) -> Result<(), String> {
    let descriptor = super::selection_events::model();
    control(driver,json!({"model":{"replies":[reply("model",json!([]),descriptor.clone())]},"registry":{"replies":[reply("getAll",json!([]),json!([descriptor])),reply("getAvailable",json!([]),json!([descriptor])),reply("find",json!(["provider","id"]),descriptor.clone())]}})).await?;
    let returned = ask(driver, json!({"action":"context","method":"model"}), false).await?;
    compare_descriptor(&returned["ok"], &descriptor);
    for method in ["getAll", "getAvailable", "find"] {
        let returned = ask(
            driver,
            json!({"action":"catalog","method":method,"args":["provider","id"]}),
            false,
        )
        .await?;
        let returned = if method == "find" {
            &returned["ok"]
        } else {
            &returned["ok"][0]
        };
        compare_descriptor(returned, &descriptor);
    }
    Ok(())
}
/// Compare shared numeric bits, every field and the observable header key sequence.
fn compare_descriptor(actual: &Value, expected: &Value) {
    super::session_corpus::same(actual, expected);
    let keys = |v: &Value| {
        v["headers"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(actual), ["z", "a"]);
    super::selection_events::assert_order(&actual["compat"], &expected["compat"]);
}
