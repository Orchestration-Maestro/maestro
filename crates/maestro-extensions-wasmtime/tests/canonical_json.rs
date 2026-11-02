//! Factory-level semantic JSON witnesses for every arbitrary payload family.
#![cfg(test)]
use maestro_extensions::{ExtensionExecution, ExtensionRunner, Local, types::ExtensionHandle};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Keeps concrete event and result owners visible beside each canonical fixture.
macro_rules! row {
    ($owner:expr, $runner:expr, $expected:expr; $event:ty, $result:ty, $name:literal, $data:expr, $answer:expr) => {
        $expected.push(roundtrip::<$event, $result>($owner, $runner, $name, $data, $answer).await);
    };
}

/// Checks event edits, result payloads and the guest's independently emitted observations.
pub(super) async fn observe(
    owner: &ExtensionHandle<Local>,
    runner: &ExtensionRunner<Local>,
    events: &maestro_extensions::EventBus<Local>,
) {
    let received = Rc::new(RefCell::new(Vec::new()));
    let observed = received.clone();
    let _subscription = events.on(
        "canonical-json",
        Rc::new(move |data, _| {
            observed.borrow_mut().push((*data).clone());
            Ok(None)
        }),
    );
    let mut expected = Vec::new();
    message_events(owner, runner, &mut expected).await;
    tool_events(owner, runner, &mut expected).await;
    entry_events(owner, runner, &mut expected).await;
    resource_events(owner, runner, &mut expected).await;
    assert_eq!(*received.borrow(), expected);
    assert_eq!(expected.len(), 20);
}

/// Checks arbitrary provider payloads and custom conversation message details.
async fn message_events(
    owner: &ExtensionHandle<Local>,
    runner: &ExtensionRunner<Local>,
    expected: &mut Vec<Value>,
) {
    let payload = json!({"z":[false,null],"a":2.5});
    let custom = json!({"role":"custom","customType":"boundary","content":"literal","display":false,"timestamp":2.5,"details":payload});
    row!(owner, runner, expected;
        maestro_extensions::BeforeProviderRequestEvent,
        maestro_extensions::BeforeProviderRequestEventResult,
        "before_provider_request",
        json!({"payload":payload}),
        Some(payload.clone())
    );
    row!(owner, runner, expected;
        maestro_extensions::BeforeAgentStartEvent,
        maestro_extensions::BeforeAgentStartEventResult,
        "before_agent_start",
        json!({"prompt":"literal","systemPrompt":"literal","systemPromptOptions":{"cwd":"/literal"}}),
        Some(
            json!({"message":{"customType":"boundary","content":"literal","display":false,"details":{"n":2.5}}})
        )
    );
    row!(owner, runner, expected;
        maestro_extensions::ContextEvent,
        maestro_extensions::ContextEventResult,
        "context",
        json!({"messages":[custom.clone()]}),
        Some(json!({"messages":[custom.clone()]}))
    );
    row!(owner, runner, expected;
        maestro_extensions::MessageEndEvent,
        maestro_extensions::MessageEndEventResult,
        "message_end",
        json!({"message":custom.clone()}),
        Some(json!({"message":custom}))
    );
}

/// Checks execution payloads and each custom or built-in tool result alias.
async fn tool_events(
    owner: &ExtensionHandle<Local>,
    runner: &ExtensionRunner<Local>,
    expected: &mut Vec<Value>,
) {
    let payload = json!({"z":[false,null],"a":2.5});
    row!(owner, runner, expected;
        maestro_extensions::ToolCallEvent,
        maestro_extensions::ToolCallEventResult,
        "tool_call",
        json!({"toolCallId":"literal","toolName":"boundary","input":payload}),
        None
    );
    row!(owner, runner, expected;
        maestro_extensions::ToolExecutionStartEvent,
        (),
        "tool_execution_start",
        json!({"toolCallId":"literal","toolName":"boundary","args":payload}),
        None
    );
    row!(owner, runner, expected;
        maestro_extensions::ToolExecutionUpdateEvent,
        (),
        "tool_execution_update",
        json!({"toolCallId":"literal","toolName":"boundary","args":payload,"partialResult":{"p":2.5}}),
        None
    );
    row!(owner, runner, expected;
        maestro_extensions::ToolExecutionEndEvent,
        (),
        "tool_execution_end",
        json!({"toolCallId":"literal","toolName":"boundary","result":payload,"isError":false}),
        None
    );
    for name in [
        "bash", "read", "edit", "write", "grep", "find", "ls", "boundary",
    ] {
        let mut data = json!({"toolCallId":"literal","toolName":name,"input":payload,"content":[],"isError":false});
        if name == "boundary" {
            data["details"] = payload.clone();
        }
        expected.push(
            roundtrip::<
                maestro_extensions::ToolResultEvent,
                maestro_extensions::ToolResultEventResult,
            >(
                owner,
                runner,
                "tool_result",
                data,
                Some(json!({"details":{"n":2.5}})),
            )
            .await,
        );
    }
}

/// Checks concrete persisted compaction and branch summary payloads.
async fn entry_events(
    owner: &ExtensionHandle<Local>,
    runner: &ExtensionRunner<Local>,
    expected: &mut Vec<Value>,
) {
    let payload = json!({"z":[false,null],"a":2.5});
    let compaction = json!({"type":"compaction","id":"compact","parentId":null,"timestamp":"literal","summary":"literal","firstKeptEntryId":"kept","tokensBefore":2.5,"details":payload});
    let branch = json!({"type":"branch_summary","id":"branch","parentId":null,"timestamp":"literal","fromId":"from","summary":"literal","details":payload});
    row!(owner, runner, expected;
        maestro_extensions::SessionCompactEvent,
        (),
        "session_compact",
        json!({"compactionEntry":compaction,"fromExtension":true}),
        None
    );
    row!(owner, runner, expected;
        maestro_extensions::SessionTreeEvent,
        (),
        "session_tree",
        json!({"newLeafId":"new","oldLeafId":"old","summaryEntry":branch}),
        None
    );
}

/// Uses the concrete native registration and context, never an adapter outcome surrogate.
async fn invoke<E: 'static, R: Serialize + 'static>(
    owner: &ExtensionHandle<Local>,
    runner: &ExtensionRunner<Local>,
    name: &str,
    event: &mut E,
    expected: Option<Value>,
) {
    let callback = {
        let owner = owner.read().unwrap();
        Local::recover_handler::<E, R>(owner.handlers[name].last().unwrap()).unwrap()
    };
    let result = callback(event, runner.create_context()).await.unwrap();
    assert_eq!(
        result.map(|result| serde_json::to_value(result).unwrap()),
        expected,
        "{name}"
    );
}

/// Rejects any payload change between native encoding and the returned guest edit.
async fn roundtrip<E: DeserializeOwned + Serialize + 'static, R: Serialize + 'static>(
    owner: &ExtensionHandle<Local>,
    runner: &ExtensionRunner<Local>,
    name: &str,
    data: Value,
    result: Option<Value>,
) -> Value {
    let mut event: E = serde_json::from_value(data).unwrap();
    let before = serde_json::to_value(&event).unwrap();
    invoke::<E, R>(owner, runner, name, &mut event, result).await;
    assert_eq!(serde_json::to_value(event).unwrap(), before, "{name}");
    before
}

/// Resource-bearing events preserve original cancellation outside the JSON envelope.
async fn resource_events(
    owner: &ExtensionHandle<Local>,
    runner: &ExtensionRunner<Local>,
    expected: &mut Vec<Value>,
) {
    let mut tree = maestro_extensions::SessionBeforeTreeEvent {
        preparation: serde_json::from_value(json!({"targetId":"target","oldLeafId":null,"commonAncestorId":null,"entriesToSummarize":[],"userWantsSummary":true})).unwrap(),
        signal: maestro_extensions::Cancellation::new(),
    };
    let before = json!({"preparation":tree.preparation});
    invoke::<_, maestro_extensions::SessionBeforeTreeResult>(
        owner,
        runner,
        "session_before_tree",
        &mut tree,
        Some(json!({"summary":{"summary":"literal","details":{"n":2.5}}})),
    )
    .await;
    assert_eq!(json!({"preparation":tree.preparation}), before);
    expected.push(before);
    let mut compact = maestro_extensions::SessionBeforeCompactEvent {
        data: serde_json::from_value(json!({"branchEntries":[],"preparation":{"firstKeptEntryId":"kept","isSplitTurn":false,"tokensBefore":2.5,"messagesToSummarize":[],"turnPrefixMessages":[],"fileOps":{"read":[],"written":[],"edited":[]},"settings":{"enabled":true,"reserveTokens":2.5,"keepRecentTokens":2.5}}})).unwrap(),
        signal: maestro_extensions::Cancellation::new(),
    };
    let before = json!(compact.data);
    invoke::<_,maestro_extensions::SessionBeforeCompactResult>(owner, runner, "session_before_compact", &mut compact, Some(json!({"compaction":{"summary":"literal","firstKeptEntryId":"kept","tokensBefore":2.5,"details":{"n":2.5}}}))).await;
    assert_eq!(json!(compact.data), before);
    expected.push(before);
}
