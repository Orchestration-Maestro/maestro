//! Tool callback observations through both component adapters.
use serde_json::{Value, json};

use super::scenario::{Driver, ToolRun};

/// Registration data with opaque schema and independently optional output.
pub(super) fn configuration() -> Value {
    json!({"metadata":{
        "name":"echo", "label":"Echo Ω", "description":"Carries values", "parameters":" {\"x\":0,\"x\":1} "
    }, "result":{"content":[],"details":"false","terminate":false}})
}

/// Registers and executes the same authored operation through an adapter.
async fn preparation_execution(driver: &mut impl Driver) -> Result<(), String> {
    let config = configuration();
    driver
        .run_plain_command("capture", &format!("tools {config}"))
        .await?;
    let args = driver.prepare("echo", r#"{"mark":"before"}"#).await?;
    assert_eq!(
        serde_json::from_str::<Value>(&args).unwrap(),
        json!({"mark":"after"})
    );
    let result = driver
        .tool(ToolRun {
            name: "echo",
            id: "call-Ω",
            params: &args,
            signal: None,
            update: true,
            held: false,
        })
        .await?;
    assert_eq!(
        serde_json::from_str::<Value>(&result).unwrap(),
        config["result"]
    );
    assert_eq!(driver.updates(), [result]);
    assert!(driver.transcript().iter().any(|line| line == r#"entry execution {"id":"call-Ω","args":{"mark":"after"},"cwd":"/work","signal":null,"update":true}"#));
    assert!(driver.release("prepare echo").await);
    assert!(driver.release("tool echo").await);
    callback_classes(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}

#[test]
fn maestro_tool_preparation_execution_and_progress_keep_values() -> Result<(), String> {
    on_both_adapters!(preparation_execution)
}

/// Handler edits are independent from its failing answer.
async fn mutation_error(driver: &mut impl Driver) -> Result<(), String> {
    use super::documents::ask;
    let events = [
        json!({"type":"input", "text":"before", "source":"interactive"}),
        json!({"type":"tool_call", "toolCallId":"c", "toolName":"bash", "input":{"command":"before"}}),
        json!({"type":"tool_call", "toolCallId":"c", "toolName":"custom", "input":" {\"mark\":\"before\"} "}),
    ];
    for (index, event) in events.into_iter().enumerate() {
        let mut edited = event.clone();
        match index {
            0 => edited["text"] = json!("changed"),
            1 => {
                edited["input"]["command"] = json!("changed");
                edited["input"]["mark"] = json!("changed");
            }
            _ => edited["input"] = json!("changed"),
        }
        let answer = ask(
            driver,
            &event,
            &json!({"mark":true, "ending":{"fails":"handler failed: Ω"}}),
        )
        .await?;
        assert_eq!(answer.event, Some(edited));
        assert_eq!(
            answer.ended,
            super::documents::Ended::Failed("handler failed: Ω".into())
        );
    }
    Ok(())
}

#[test]
fn maestro_handler_mutation_survives_returned_error() -> Result<(), String> {
    on_both_adapters!(mutation_error)
}

/// Metadata crosses registration unchanged; callbacks leave the table before capture drops.
async fn registration(driver: &mut impl Driver) -> Result<(), String> {
    accepted_registration(driver, false).await?;
    accepted_registration(driver, true).await?;
    rejected_registration(driver).await?;
    let released = driver
        .transcript()
        .iter()
        .filter(|line| line.starts_with("entry released \"tool "))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        &released[..3],
        [
            r#"entry released "tool execute""#,
            r#"entry released "tool prepare""#,
            r#"entry released "tool execute""#
        ]
    );
    assert_eq!(
        released
            .iter()
            .filter(|line| line.ends_with("prepare\""))
            .count(),
        2
    );
    assert_eq!(
        released
            .iter()
            .filter(|line| line.ends_with("execute\""))
            .count(),
        3
    );
    metadata_classes(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}

#[test]
fn maestro_tool_registration_retains_metadata_and_releases_callbacks() -> Result<(), String> {
    on_both_adapters!(registration)
}

/// A held invocation sees cancellation on the same retained resource after progress.
async fn retained_execution(driver: &mut impl Driver) -> Result<(), String> {
    driver.run_plain_command("capture", "").await?;
    let baseline = driver.live();
    let mut config = configuration();
    config["retain"] = json!(true);
    config["held"] = json!(true);
    driver
        .run_plain_command("capture", &format!("tools {config}"))
        .await?;
    let registered = driver.live();
    let signal = driver.lend_signal(false)?;
    let result = driver
        .tool(ToolRun {
            name: "echo",
            id: "pending",
            params: "{}",
            signal: Some(signal),
            update: true,
            held: true,
        })
        .await?;
    assert_eq!(driver.updates(), [result]);
    assert!(
        driver
            .transcript()
            .iter()
            .any(|line| line == "entry after-gate true")
    );
    retained_aliases(driver, registered).await?;
    assert!(registered > baseline);
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}

#[test]
fn maestro_tool_execution_suspends_and_retains_signal_and_progress() -> Result<(), String> {
    on_both_adapters!(retained_execution)
}

/// Every recorded partial/final output and each failure reaches the same invocation.
async fn callback_classes(driver: &mut impl Driver) -> Result<(), String> {
    let outputs: Vec<Value> = serde_json::from_str(include_str!("tool_outputs.json")).unwrap();
    assert_eq!(outputs.len(), 72);
    for output in outputs {
        let mut config = configuration();
        config["result"] = output.clone();
        driver
            .run_plain_command("capture", &format!("tools {config}"))
            .await?;
        let result = driver
            .tool(ToolRun {
                name: "echo",
                id: "output",
                params: "{}",
                signal: None,
                update: true,
                held: false,
            })
            .await?;
        assert_eq!(serde_json::from_str::<Value>(&result).unwrap(), output);
        assert_eq!(driver.updates().last(), Some(&result));
        assert!(driver.release("prepare echo").await);
        assert!(driver.release("tool echo").await);
    }
    callback_errors(driver).await?;
    execution_capabilities(driver).await
}

/// Preparation, execution and update failures retain their messages.
async fn callback_errors(driver: &mut impl Driver) -> Result<(), String> {
    for (phase, message) in [
        ("prepare", "failed prepare: Ω"),
        ("execute", "failed execute: Ω"),
        ("update", "failed update: Ω"),
    ] {
        let mut config = configuration();
        match phase {
            "prepare" => config["prepareError"] = json!(message),
            "execute" => config["executeError"] = json!(message),
            _ => config["result"]["details"] = json!("fail update"),
        }
        driver
            .run_plain_command("capture", &format!("tools {config}"))
            .await?;
        let failure = if phase == "prepare" {
            driver.prepare("echo", "{}").await
        } else {
            driver
                .tool(ToolRun {
                    name: "echo",
                    id: "failure",
                    params: "{}",
                    signal: None,
                    update: true,
                    held: false,
                })
                .await
        };
        assert_eq!(failure.unwrap_err(), message);
        assert!(driver.release("prepare echo").await);
        assert!(driver.release("tool echo").await);
    }
    Ok(())
}

/// Optional resource combinations reach the callback independently.
async fn execution_capabilities(driver: &mut impl Driver) -> Result<(), String> {
    for (signal_present, update_present) in
        [(false, false), (false, true), (true, false), (true, true)]
    {
        let config = configuration();
        driver
            .run_plain_command("capture", &format!("tools {config}"))
            .await?;
        let signal = if signal_present {
            Some(driver.lend_signal(false)?)
        } else {
            None
        };
        driver
            .tool(ToolRun {
                name: "echo",
                id: "capabilities",
                params: "{}",
                signal,
                update: update_present,
                held: false,
            })
            .await?;
        let expected = format!(
            "entry execution {}",
            json!({"id":"capabilities","args":{},"cwd":"/work","signal":if signal_present {json!(false)} else {Value::Null},"update":update_present})
        );
        assert!(driver.transcript().contains(&expected));
        assert!(driver.release("prepare echo").await);
        assert!(driver.release("tool echo").await);
    }
    Ok(())
}

/// Rejection drops both callbacks without publishing identities.
async fn rejected_registration(driver: &mut impl Driver) -> Result<(), String> {
    let mut config = configuration();
    config["metadata"]["name"] = json!("rejected");
    assert_eq!(
        driver
            .run_plain_command("capture", &format!("tools {config}"))
            .await
            .unwrap_err(),
        "tool rejected: Ω"
    );
    assert!(driver.identity("tool rejected").is_err());
    assert!(driver.identity("prepare rejected").is_err());
    Ok(())
}

/// Surviving aliases keep both resources; their last owners release them.
async fn retained_aliases(driver: &mut impl Driver, registered: usize) -> Result<(), String> {
    assert_eq!(driver.live(), registered + 2);
    assert_eq!(
        driver.prepare("echo", r#"{"control":"report"}"#).await?,
        "true"
    );
    driver.prepare("echo", r#"{"control":"drop-one"}"#).await?;
    assert_eq!(driver.live(), registered + 2);
    assert_eq!(
        driver.prepare("echo", r#"{"control":"report"}"#).await?,
        "true"
    );
    driver.prepare("echo", r#"{"control":"drop-all"}"#).await?;
    assert_eq!(driver.live(), registered);
    assert_eq!(driver.updates().len(), 3);
    Ok(())
}

/// Acceptance preserves metadata and exposes only the supplied callback kinds.
async fn accepted_registration(driver: &mut impl Driver, prepare: bool) -> Result<(), String> {
    let mut config = configuration();
    config["prepare"] = json!(prepare);
    config["metadata"]["name"] = json!(if prepare { "prepared" } else { "plain" });
    for (field, value) in [
        ("promptSnippet", json!("")),
        ("promptGuidelines", json!(["z", "a", "z"])),
        ("executionMode", json!("parallel")),
        ("renderShell", json!("self")),
    ] {
        config["metadata"][field] = value;
    }
    driver
        .run_plain_command("capture", &format!("tools {config}"))
        .await?;
    assert_eq!(
        serde_json::from_str::<Value>(driver.tools().last().unwrap()).unwrap(),
        config["metadata"]
    );
    let name = config["metadata"]["name"].as_str().unwrap();
    assert_eq!(driver.identity(&format!("prepare {name}")).is_ok(), prepare);
    let execute = driver.identity(&format!("tool {name}"))?;
    assert_eq!(
        driver
            .prepare_key(execute, "{}")
            .await
            .unwrap_err()
            .split(" identity ")
            .next()
            .unwrap(),
        "no callback registered for"
    );
    if prepare {
        assert!(driver.release(&format!("prepare {name}")).await);
        let dropped = *driver.dropped().last().unwrap();
        let handler = driver.reborrow(dropped)?;
        assert_eq!(
            driver.prepare_key(handler, "{}").await.unwrap_err(),
            format!("no callback registered for identity {dropped}")
        );
    }
    assert!(driver.release(&format!("tool {name}")).await);
    Ok(())
}

/// Optional metadata distinguishes null, empty and each authored enum choice.
async fn metadata_classes(driver: &mut impl Driver) -> Result<(), String> {
    for patch in [
        json!({"promptSnippet":null,"promptGuidelines":null,"executionMode":null,"renderShell":null}),
        json!({"promptSnippet":"snippet","promptGuidelines":[],"executionMode":"sequential","renderShell":"default"}),
        json!({"promptSnippet":"","promptGuidelines":["z","a","z"],"executionMode":"parallel","renderShell":"self"}),
    ] {
        let mut config = configuration();
        config["metadata"]
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        driver
            .run_plain_command("capture", &format!("tools {config}"))
            .await?;
        assert_eq!(
            serde_json::from_str::<Value>(driver.tools().last().unwrap()).unwrap(),
            config["metadata"]
        );
        assert!(driver.release("prepare echo").await);
        assert!(driver.release("tool echo").await);
    }
    Ok(())
}
