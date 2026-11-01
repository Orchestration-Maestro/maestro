//! Canonical registrations invoke original author callbacks through native consumers.
#![cfg(test)]
use maestro_extensions::{ExtensionRunner, Local};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Invokes completions and shortcuts after their originating registration call ended.
pub(super) async fn observe(runner: Rc<ExtensionRunner<Local>>) {
    let command = runner.get_command("canonical-completion").unwrap().command;
    assert_eq!(command.description.as_deref(), Some(" literal command "));
    assert_eq!(
        (command.handler)(" literal args ".into(), runner.create_command_context())
            .await
            .unwrap_err()
            .to_string(),
        " literal args |/canonical-reads"
    );
    let completions = (command.get_argument_completions.unwrap())(" literal prefix ".into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].value, " literal prefix  literal value ");
    assert_eq!(completions[0].label, " literal label ");
    assert_eq!(
        completions[0].description.as_deref(),
        Some(" literal description ")
    );
    let shortcut = runner.get_shortcuts(&Vec::default())["ctrl+alt+k"].clone();
    let handler = {
        let shortcut = shortcut.read().unwrap();
        assert_eq!(shortcut.description.as_deref(), Some(" literal shortcut "));
        shortcut.handler.clone()
    };
    assert_eq!(
        handler(runner.create_context())
            .await
            .unwrap_err()
            .to_string(),
        "shortcut:/canonical-reads"
    );
    tool(runner.clone()).await;
    shell(&runner).await;
}

/// Uses the production wrapper's fresh context and actual caller-owned update sink.
async fn tool(runner: Rc<ExtensionRunner<Local>>) {
    let registered = runner.get_all_registered_tools();
    assert_eq!(registered.len(), 1);
    let definition = &registered[0];
    let metadata = definition.definition.metadata();
    assert_eq!(metadata.definition.name, "canonical-tool");
    assert_eq!(
        metadata.definition.parameters.as_json(),
        &json!({"type":"object"})
    );
    assert_eq!(
        metadata.definition.description,
        " Literal tool description "
    );
    let wrapped = maestro_extensions::wrap_registered_tool(definition, runner);
    let mut input = json!({"n":2.5});
    let prepared = wrapped.prepare_arguments.as_ref().unwrap()(&mut input).unwrap();
    assert_eq!(prepared, json!({"prepared":{"n":2.5}}));
    let updates = Rc::new(RefCell::new(Vec::new()));
    let received = updates.clone();
    let signal = maestro_extensions::Cancellation::new();
    signal.abort();
    let result = (wrapped.execute)(
        " literal tool id ".into(),
        prepared,
        Some(signal),
        Some(Rc::new(move |update| received.borrow_mut().push(update))),
    )
    .await
    .unwrap();
    let details = json!({"id":" literal tool id ","params":{"prepared":{"n":2.5}},"cwd":"/canonical-reads","aborted":true});
    assert_eq!(result.details, Some(details.clone()));
    assert_eq!(result.terminate, Some(false));
    assert_eq!(
        serde_json::to_value(result.content).unwrap(),
        json!([{"type":"text","text":" literal result "}])
    );
    let updates: Vec<Value> = updates
        .borrow()
        .iter()
        .map(|update| serde_json::to_value(update).unwrap())
        .collect();
    assert_eq!(updates, [json!({"content":[],"details":details})]);
}

/// Calls a returned real guest executor with uninterpreted bytes and no native process.
async fn shell(runner: &ExtensionRunner<Local>) {
    let mut event = maestro_extensions::UserBashEvent {
        command: " literal event ".into(),
        cwd: "/event".into(),
        exclude_from_context: true,
    };
    let result = runner.emit_user_bash::<Rc<dyn maestro_extensions::BashOperations<Local>>, maestro_extensions::BashResult>(&mut event).await.unwrap().unwrap();
    assert_eq!(event.command, " literal event :edited");
    assert!(result.result.is_missing());
    let mut chunks = Vec::new();
    let mut sink = |bytes: &[u8]| chunks.push(bytes.to_vec());
    let signal = maestro_extensions::Cancellation::new();
    signal.abort();
    let result = result
        .operations
        .unwrap()
        .exec(
            " literal shell ",
            "./literal/../cwd",
            maestro_extensions::BashExecOptions {
                on_data: &mut sink,
                signal: Some(&signal),
                timeout: Some(2.5),
                env: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(result.exit_code, Some(7.5));
    assert_eq!(chunks, [vec![0, 255, 10], vec![]]);
}
