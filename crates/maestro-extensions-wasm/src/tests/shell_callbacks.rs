//! Returned shell capabilities through the compiled and controlled public seams.
use super::{
    scenario::{Decision, Delivery, Driver},
    shell_host::{
        Case, EmptyExpected, EmptyInput, ExecExpected, ExecInput, Operands, OutcomeExpected,
        OutcomeInput, Run,
    },
};
use serde_json::{Value, json};
/// Registers one configured authored operation.
async fn register(driver: &mut impl Driver, config: &Value) -> Result<(), String> {
    driver
        .run_plain_command("capture", &format!("shell-register {config}"))
        .await
}
/// Delivers a user shell event and keeps its returned resource separate from documents.
async fn answer(driver: &mut impl Driver) -> Result<(Delivery, Option<usize>), String> {
    let handler = driver.identity("event user_bash")?;
    let event =
        r#"{"type":"user_bash","command":"before","excludeFromContext":false,"cwd":"../work"}"#;
    let delivery = driver
        .deliver(handler, event, "shell-context", None)
        .await?;
    Ok((delivery, driver.returned_bash()))
}
/// Changes retained author state through a public callback.
async fn control(driver: &mut impl Driver, value: Value) -> Result<Delivery, String> {
    let handler = driver.identity("event shell-control")?;
    let event = json!({"type":"input","text":value.to_string(),"source":"rpc"});
    driver
        .deliver(handler, &event.to_string(), "shell-context", None)
        .await
}
/// A run whose operands differ from defaults.
fn run(operation: usize, output: usize) -> Run {
    Run {
        operation,
        output,
        command: "  echo 雪\0\n ".into(),
        cwd: "../x/../ y ".into(),
        timeout: Some(-0.0),
        env: Some(vec![("MISSING".into(), None)]),
        signal: None,
        release_pending: None,
    }
}
/// Closes the author-held alias and every registration after resource observations.
async fn close(driver: &mut impl Driver) -> Result<(), String> {
    control(driver, json!({"action":"clear-operation"})).await?;
    control(driver, json!({"action":"clear-resources"})).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
/// Replays the ordinary forwarding corpus through both actual adapters.
async fn invocation(driver: &mut impl Driver) -> Result<(), String> {
    let corpus: Vec<Case> =
        serde_json::from_str(include_str!("../../tests/fixtures/shell-operations.json"))
            .map_err(|e| e.to_string())?;
    for case in corpus {
        match case.group.as_str() {
            "outcome" => outcome_case(driver, case).await?,
            "empty-answer" => empty_case(driver, case).await?,
            "exec" => exec_case(driver, case).await?,
            "failure" => {}
            other => return Err(format!("unknown corpus group {other}")),
        }
    }
    mutable_capture(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
/// Removes each author-held alias before replacing its control registration.
async fn cleanup_shell(driver: &mut impl Driver, operation: Option<usize>) -> Result<(), String> {
    if let Some(operation) = operation {
        driver.drop_bash(operation).await?;
    }
    control(driver, json!({"action":"clear-operation"})).await?;
    control(driver, json!({"action":"clear-resources"})).await?;
    assert!(driver.release("event user_bash").await);
    assert!(driver.release("event shell-control").await);
    Ok(())
}
/// Compares every document member without collapsing absent and explicit null.
async fn outcome_case(driver: &mut impl Driver, case: Case) -> Result<(), String> {
    let input: OutcomeInput =
        serde_json::from_value(case.input.clone()).map_err(|e| e.to_string())?;
    let expected: OutcomeExpected =
        serde_json::from_value(case.expected).map_err(|e| e.to_string())?;
    register(
        driver,
        &json!({"operations":input.operations,"result":input.result,"chunks":[[122]],"exitCode":7}),
    )
    .await?;
    let (delivery, operation) = answer(driver).await?;
    assert_eq!(operation.is_some(), expected.operations);
    let Decision::Returned(encoded) = delivery.decision else {
        return Err("unexpected handler failure".into());
    };
    let encoded = encoded?;
    assert_eq!(encoded.is_some(), expected.returned);
    let document: Value = serde_json::from_str(encoded.as_deref().ok_or("missing answer")?)
        .map_err(|e| e.to_string())?;
    if expected.result.presence == "missing" {
        assert_eq!(document, json!({}));
    } else {
        assert_eq!(expected.result.presence, "present");
        assert_document(&document, &json!({"result":expected.result.value}));
    }
    if let Some(operation) = operation {
        let output = driver.shell_output(None, None)?;
        assert_eq!(
            driver.shell_exec(run(operation, output)).await?,
            expected.exit_code
        );
        assert_eq!(driver.shell_chunks(output), [vec![122]]);
    } else {
        assert!(expected.exit_code.is_none());
    }
    cleanup_shell(driver, operation).await
}
/// Compares numeric value rather than the JSON decoder's integer/float storage variant.
fn assert_document(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(e)) => {
            assert_eq!(a.as_f64().unwrap().to_bits(), e.as_f64().unwrap().to_bits());
        }
        (Value::Object(a), Value::Object(e)) => {
            assert_eq!(a.keys().collect::<Vec<_>>(), e.keys().collect::<Vec<_>>());
            for (key, value) in e {
                assert_document(&a[key], value);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}
/// Both source no-answer forms use the one native absence representation.
async fn empty_case(driver: &mut impl Driver, case: Case) -> Result<(), String> {
    let input: EmptyInput = serde_json::from_value(case.input).map_err(|e| e.to_string())?;
    let expected: EmptyExpected =
        serde_json::from_value(case.expected).map_err(|e| e.to_string())?;
    assert!(["undefined", "null"].contains(&input.value.as_str()));
    register(driver, &json!({"empty":input.value})).await?;
    let (delivery, operation) = answer(driver).await?;
    assert!(operation.is_none());
    assert!(!expected.returned);
    assert_eq!(delivery.decision, Decision::Returned(Ok(None)));
    cleanup_shell(driver, operation).await
}
/// Parses the corpus's numeric spelling, preserving sign and exact finite bits.
fn timeout_bits(timeout: Option<&str>) -> Result<Option<u64>, String> {
    timeout
        .map(|s| {
            s.parse::<f64>()
                .map(f64::to_bits)
                .map_err(|e| e.to_string())
        })
        .transpose()
}
/// Reads only observations emitted by this invocation; earlier calls cannot satisfy it.
fn operands(driver: &impl Driver, before: usize) -> Result<(Operands, Option<bool>), String> {
    let lines = driver.transcript();
    let emitted = &lines[before..];
    let entry = emitted
        .iter()
        .find_map(|line| line.strip_prefix("entry shell-operands "))
        .ok_or("missing operands observation")?;
    let final_state = emitted
        .iter()
        .find_map(|line| line.strip_prefix("entry shell-final "))
        .ok_or("missing final signal observation")?;
    Ok((
        serde_json::from_str(entry).map_err(|e| e.to_string())?,
        serde_json::from_str(final_state).map_err(|e| e.to_string())?,
    ))
}
/// Runs unequal timeout/environment policies and all independently selected signal states.
async fn exec_case(driver: &mut impl Driver, case: Case) -> Result<(), String> {
    let input: ExecInput = serde_json::from_value(case.input.clone()).map_err(|e| e.to_string())?;
    let expected: ExecExpected =
        serde_json::from_value(case.expected).map_err(|e| e.to_string())?;
    register(
        driver,
        &json!({"chunks":input.chunks,"exitCode":input.exit_code}),
    )
    .await?;
    let (_, operation) = answer(driver).await?;
    let operation = operation.ok_or("missing operation")?;
    let signal = match input.signal.as_str() {
        "absent" => None,
        "live" | "later-cancelled" => Some(driver.lend_signal(false)?),
        "already-cancelled" => Some(driver.lend_signal(true)?),
        other => return Err(format!("unknown signal {other}")),
    };
    let output = driver.shell_output(
        None,
        (input.signal == "later-cancelled")
            .then_some(signal)
            .flatten(),
    )?;
    let before = driver.transcript().len();
    let timeout = timeout_bits(input.timeout.as_deref())?.map(f64::from_bits);
    let actual = driver
        .shell_exec(Run {
            operation,
            output,
            command: input.command,
            cwd: input.cwd,
            timeout,
            env: input.env,
            signal,
            release_pending: None,
        })
        .await?;
    assert_eq!(
        actual.map(f64::to_bits),
        expected.exit_code.map(f64::to_bits)
    );
    assert_eq!(driver.shell_chunks(output), expected.chunks);
    let (operands, final_state) = operands(driver, before)?;
    assert_eq!(operands.command, expected.command);
    assert_eq!(operands.cwd, expected.cwd);
    assert_eq!(
        operands.timeout.map(|s| s.parse::<u64>().unwrap()),
        timeout_bits(expected.timeout.as_deref())?
    );
    assert_eq!(operands.env, expected.env);
    assert_eq!(operands.initial_signal, expected.initial_signal);
    assert_eq!(final_state, expected.final_signal);
    cleanup_shell(driver, Some(operation)).await
}
/// A surviving callback reads mutated captures rather than a registration-time copy.
async fn mutable_capture(driver: &mut impl Driver) -> Result<(), String> {
    register(driver, &json!({"chunks":[[9]],"exitCode":23})).await?;
    let (_, operation) = answer(driver).await?;
    let operation = operation.ok_or("missing operation")?;
    for (chunks, exit) in [(vec![vec![9]], 23.0), (vec![vec![44], vec![44]], -3.5)] {
        control(driver, json!({"chunks":chunks,"exitCode":exit})).await?;
        let output = driver.shell_output(None, None)?;
        assert_eq!(driver.shell_exec(run(operation, output)).await?, Some(exit));
        assert_eq!(driver.shell_chunks(output), chunks);
    }
    let decoded: crate::UserBashEventResult =
        serde_json::from_value(json!({"operations":{"exec":"fabricated"}}))
            .map_err(|e| e.to_string())?;
    assert!(decoded.operations.is_none());
    assert_eq!(
        serde_json::to_value(decoded).map_err(|e| e.to_string())?,
        json!({})
    );
    cleanup_shell(driver, Some(operation)).await
}
#[test]
fn maestro_user_shell_operations_stream_and_retain_execution() -> Result<(), String> {
    on_both_adapters!(invocation)
}
/// Registration release cannot revoke a pending call or an independently returned alias.
async fn ownership(driver: &mut impl Driver) -> Result<(), String> {
    register(
        driver,
        &json!({"chunks":[[122],[9]],"exitCode":17,"retain":true,"wait":true,"reenter":true}),
    )
    .await?;
    let before = driver.transcript().len();
    let (_, first) = answer(driver).await?;
    let first = first.ok_or("first alias absent")?;
    let (_, second) = answer(driver).await?;
    let second = second.ok_or("second alias absent")?;
    let handler = driver.identity("event user_bash")?;
    let flag = driver.lend_signal(false)?;
    let output = driver.shell_output(None, None)?;
    let mut pending = run(first, output);
    pending.signal = Some(flag);
    pending.release_pending = Some(handler);
    assert_eq!(driver.shell_exec(pending).await?, Some(17.0));
    assert_eq!(driver.shell_chunks(output), [vec![122], vec![9]]);
    assert_eq!(operands(driver, before)?.1, Some(true));
    assert_reentered(driver).await?;
    driver.drop_bash(first).await?;
    surviving_aliases(driver, (second, output, flag), before).await?;
    close(driver).await
}
/// Separately removes facade aliases and the author slot, then checks final owners.
async fn surviving_aliases(
    driver: &mut impl Driver,
    prior: (usize, usize, usize),
    before: usize,
) -> Result<(), String> {
    let (second, first_output, first_flag) = prior;
    control(
        driver,
        json!({"chunks":[[45]],"exitCode":-3.5,"retain":true}),
    )
    .await?;
    let flag = driver.lend_signal(false)?;
    let output = driver.shell_output(None, None)?;
    let mut call = run(second, output);
    call.signal = Some(flag);
    assert_eq!(driver.shell_exec(call).await?, Some(-3.5));
    assert_eq!(driver.shell_chunks(output), [vec![45]]);
    assert_eq!(driver.shell_drops(first_output), 1);
    assert_eq!(driver.shell_signal_drops(first_flag), 1);
    control(driver, json!({"action":"alias"})).await?;
    let third = driver.returned_bash().ok_or("author alias absent")?;
    driver.drop_bash(second).await?;
    control(driver, json!({"action":"clear-operation"})).await?;
    driver.abort_signal(flag)?;
    last_alias(driver, (third, output, flag), before).await
}
/// Independently retained resources survive final operation teardown.
async fn last_alias(
    driver: &mut impl Driver,
    state: (usize, usize, usize),
    before: usize,
) -> Result<(), String> {
    let (third, output, flag) = state;
    let third_output = healthy(driver, third, 91, 29.0).await?;
    assert_eq!(releases(driver, before, "shell-operation"), 0);
    driver.drop_bash(third).await?;
    assert_eq!(releases(driver, before, "shell-operation"), 1);
    assert_eq!(releases(driver, before, "shell-handler"), 1);
    assert_reentered(driver).await?;
    retained_resources(driver, (output, flag)).await?;
    assert_eq!(driver.shell_drops(third_output), 1);
    Ok(())
}
/// Uses retained output and signal after the operation's last owner has been destroyed.
async fn retained_resources(
    driver: &mut impl Driver,
    resources: (usize, usize),
) -> Result<(), String> {
    let (output, flag) = resources;
    control(driver, json!({"action":"output"})).await?;
    assert_eq!(driver.shell_chunks(output), [vec![45], vec![42, 0, 255]]);
    let signal = control(driver, json!({"action":"signal"})).await?;
    let event: Value =
        serde_json::from_str(signal.event?.as_deref().ok_or("edited control absent")?)
            .map_err(|e| e.to_string())?;
    assert_eq!(event["text"], "true");
    control(driver, json!({"action":"clear-resources"})).await?;
    assert_eq!(driver.shell_drops(output), 1);
    assert_eq!(driver.shell_signal_drops(flag), 1);
    Ok(())
}
/// Runs the callback created by a destructor, proving teardown permitted registration.
async fn assert_reentered(driver: &mut impl Driver) -> Result<(), String> {
    let handler = driver.identity("event shell-reentered")?;
    let result = driver
        .deliver(
            handler,
            r#"{"type":"input","text":"before","source":"rpc"}"#,
            "/work",
            None,
        )
        .await?;
    let event: Value = serde_json::from_str(result.event?.as_deref().ok_or("event absent")?)
        .map_err(|e| e.to_string())?;
    assert_eq!(event["text"], "reentered");
    Ok(())
}
/// Counts destruction only after the relevant final producer has been removed.
fn releases(driver: &impl Driver, before: usize, label: &str) -> usize {
    driver.transcript()[before..]
        .iter()
        .filter(|line| **line == format!("entry released {}", json!(label)))
        .count()
}
#[test]
fn maestro_user_shell_owners_survive_registration_release() -> Result<(), String> {
    on_both_adapters!(ownership)
}
/// Keeps recoverable callback failures separate from event/document encoding errors.
async fn failures(driver: &mut impl Driver) -> Result<(), String> {
    let corpus: Vec<Case> =
        serde_json::from_str(include_str!("../../tests/fixtures/shell-operations.json"))
            .map_err(|e| e.to_string())?;
    for case in corpus.into_iter().filter(|case| case.group == "failure") {
        failure_case(driver, case).await?;
    }
    independent_encodings(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
/// Exercises healthy/error/healthy calls against the same returned callback.
async fn failure_case(driver: &mut impl Driver, case: Case) -> Result<(), String> {
    use super::shell_host::{FailureExpected, FailureInput};
    let input: FailureInput = serde_json::from_value(case.input).map_err(|e| e.to_string())?;
    let expected: FailureExpected =
        serde_json::from_value(case.expected).map_err(|e| e.to_string())?;
    register(driver, &json!({"chunks":[[64]],"exitCode":8})).await?;
    let (_, operation) = answer(driver).await?;
    let operation = operation.ok_or("returned operation absent")?;
    healthy(driver, operation, 64, 8.0).await?;
    let (config, output_error) = match input.kind.as_str() {
        "author-empty" | "author-text" => (json!({"chunks":[[122]],"error":input.message}), None),
        "output-error" => (json!({"chunks":[[122],[255]]}), Some(input.message)),
        "output-caught" => (
            json!({"chunks":[[122],[255],[42]],"catch":true,"exitCode":19}),
            Some(input.message),
        ),
        other => return Err(format!("unknown failure kind {other}")),
    };
    control(driver, config).await?;
    let output = driver.shell_output(output_error, None)?;
    let result = driver.shell_exec(run(operation, output)).await;
    match expected.error {
        Some(error) => assert_eq!(result, Err(error)),
        None => assert_eq!(result, Ok(expected.exit_code)),
    }
    assert_eq!(driver.shell_chunks(output), expected.chunks);
    healthy(driver, operation, 81, -2.0).await?;
    cleanup_shell(driver, Some(operation)).await
}
/// A distinguishable successful call proves that a prior error did not poison the resource.
async fn healthy(
    driver: &mut impl Driver,
    operation: usize,
    byte: u8,
    exit: f64,
) -> Result<usize, String> {
    control(driver, json!({"chunks":[[byte]],"exitCode":exit})).await?;
    let output = driver.shell_output(None, None)?;
    assert_eq!(driver.shell_exec(run(operation, output)).await?, Some(exit));
    assert_eq!(driver.shell_chunks(output), [vec![byte]]);
    Ok(output)
}
/// The result encoder cannot erase the resource that accompanied the rejected document.
async fn independent_encodings(driver: &mut impl Driver) -> Result<(), String> {
    for number in ["infinity", "negative-infinity", "nan"] {
        register(driver,&json!({"operations":true,"result":"nonfinite","number":number,"chunks":[[213]],"exitCode":31})).await?;
        let (delivery, operation) = answer(driver).await?;
        let event: Value = serde_json::from_str(delivery.event?.as_deref().ok_or("event absent")?)
            .map_err(|e| e.to_string())?;
        assert_eq!(event["command"], "edited\0雪");
        assert_eq!(
            delivery.decision,
            Decision::Returned(Err(
                "extension wrote a non-finite number (Infinity or NaN)".into()
            ))
        );
        let operation = operation.ok_or("encoding failure erased operation")?;
        let output = driver.shell_output(None, None)?;
        assert_eq!(driver.shell_exec(run(operation, output)).await?, Some(31.0));
        assert_eq!(driver.shell_chunks(output), [vec![213]]);
        cleanup_shell(driver, Some(operation)).await?;
    }
    handler_failure(driver).await
}
/// An authored handler error keeps its earlier event edit but returns no operation.
async fn handler_failure(driver: &mut impl Driver) -> Result<(), String> {
    register(driver, &json!({"handlerError":"handler failed\0雪"})).await?;
    let (delivery, operation) = answer(driver).await?;
    assert!(operation.is_none());
    let event: Value = serde_json::from_str(delivery.event?.as_deref().ok_or("event absent")?)
        .map_err(|e| e.to_string())?;
    assert_eq!(event["command"], "edited\0雪");
    assert_eq!(
        delivery.decision,
        Decision::Failed("handler failed\0雪".into())
    );
    cleanup_shell(driver, None).await
}
#[test]
fn maestro_user_shell_failures_preserve_independent_outcomes() -> Result<(), String> {
    on_both_adapters!(failures)
}
