//! Value-chaining groups: context, payloads, agent starts, message ends and input transforms.
use super::Harness;
use super::cases::{
    answer, assistant, edit, encoded, fail, images, labels, message_view, observe, options,
    outcome, text_views, typed, user,
};
use maestro_extensions::{
    ExtensionRunner, Local, runner::BeforeAgentStartCombinedResult as Combined,
};
use maestro_extensions::{MessageEndEvent, NativeAgentMessage};
use serde_json::{Value, json};

/// Dispatches the value-chaining groups.
pub(super) async fn replay(
    harness: &Harness,
    index: usize,
    name: &str,
    input: &Value,
    recorded: Value,
) -> (Value, Value) {
    match name {
        "context" => context(harness, input, recorded).await,
        "context-rebinding" => context_assignment(harness).await,
        "payload" => payloads(harness, recorded).await,
        "payload-rebinding" => payload_assignment(harness).await,
        "payload-zero" | "payload-order" => payload_numbers(harness, name, recorded).await,
        "before-agent" => agent_start(harness, recorded).await,
        "concurrent-prompts" => concurrent_prompts(harness, recorded).await,
        "before-agent-branches" => agent_branches(harness, input, recorded).await,
        "message-end" => message_end(harness, input, recorded).await,
        "input" => super::input::input(harness, index, input, recorded).await,
        other => panic!("unrecorded group {other}"),
    }
}

/// Edits before a failure survive, a later replacement wins and the caller's list is copied.
async fn context(harness: &Harness, input: &Value, mut recorded: Value) -> (Value, Value) {
    let empty = input["kind"] == "empty replacement";
    let handlers = if empty {
        vec![json!([answer(json!({ "messages": [] }))])]
    } else {
        let edited = json!({ "role": "user", "content": [{ "type": "text", "text": "edited" }], "timestamp": 1 });
        vec![
            json!([
                edit("messages", json!([edited])),
                edit("append", user("tail")),
                fail("after mutation")
            ]),
            json!([edit("append", assistant("replaced")), "echo"]),
        ]
    };
    let run = harness.prepare("context", &[handlers]).await;
    let original = if empty {
        user("old")
    } else {
        json!({ "role": "user", "content": [{ "type": "text", "text": "original" }], "timestamp": 1 })
    };
    let messages: Vec<NativeAgentMessage> = typed(json!([original]));
    let result = encoded(&run.runner.emit_context(&messages).await.unwrap());
    if empty {
        return (json!({ "messages": result }), recorded);
    }
    let observed =
        json!({ "original": text_views(&encoded(&messages)), "messages": text_views(&result) });
    let output = &mut recorded["output"];
    output["original"] = text_views(&output["original"]);
    output["messages"] = text_views(&output["messages"]);
    (outcome(observed, &run), recorded)
}

/// Assigning the context list in place is a Rust assignment: the next handler and the result
/// see the assigned list, where the recording kept its originally selected list.
async fn context_assignment(harness: &Harness) -> (Value, Value) {
    let handlers = vec![
        json!([edit("messages", json!([user("not selected")]))]),
        json!([observe("firstContent")]),
    ];
    let run = harness.prepare("context", &[handlers]).await;
    let messages: Vec<NativeAgentMessage> = typed(json!([user("original")]));
    let result = encoded(&run.runner.emit_context(&messages).await.unwrap());
    let observed = json!({ "messages": text_views(&result), "seen": labels(harness) });
    let expected = json!({ "messages": [{ "role": "user", "text": "not selected" }], "seen": ["not selected"] });
    (
        outcome(observed, &run),
        json!({ "output": expected, "errors": [] }),
    )
}

/// Every present payload replaces the previous one; only absence keeps it.
async fn payloads(harness: &Harness, recorded: Value) -> (Value, Value) {
    let mut handlers: Vec<_> = [
        json!(null),
        json!(false),
        json!(0),
        json!(""),
        json!("nonempty"),
        json!([]),
        json!({}),
    ]
    .into_iter()
    .map(|value| json!([observe("payload"), answer(value)]))
    .collect();
    handlers.push(json!([observe("payload")]));
    let run = harness
        .prepare("before_provider_request", &[handlers])
        .await;
    let payload = run
        .runner
        .emit_before_provider_request(json!({ "initial": true }))
        .await
        .unwrap();
    let observed = json!({ "payload": payload, "seen": labels(harness) });
    (outcome(observed, &run), recorded)
}

/// A nested edit before a failure is seen; a later assignment survives its own failure and
/// is the final payload. The recording's selected-object identity has no Rust counterpart.
async fn payload_assignment(harness: &Harness) -> (Value, Value) {
    let handlers = vec![
        json!([
            edit("payload", json!({ "nested": { "flag": "mutated" } })),
            fail("after edit")
        ]),
        json!([
            observe("payload"),
            edit("payload", json!(["assigned"])),
            fail("after assignment")
        ]),
        json!([observe("payload")]),
    ];
    let run = harness
        .prepare("before_provider_request", &[handlers])
        .await;
    let payload = run
        .runner
        .emit_before_provider_request(json!({ "nested": { "flag": "original" } }))
        .await
        .unwrap();
    let observed = json!({ "payload": payload, "seen": labels(harness) });
    let failure = |error: &str| json!({ "extensionPath": "<extension:0>", "event": "before_provider_request", "error": error });
    let expected = json!({
        "output": { "payload": ["assigned"], "seen": [{ "nested": { "flag": "mutated" } }, ["assigned"]] },
        "errors": [failure("after edit"), failure("after assignment")],
    });
    (outcome(observed, &run), expected)
}

/// A negative zero and the object key order survive the component boundary.
async fn payload_numbers(harness: &Harness, name: &str, recorded: Value) -> (Value, Value) {
    let order = name == "payload-order";
    let steps = if order {
        json!(["echo"])
    } else {
        json!([answer(json!(-0.0))])
    };
    let run = harness
        .prepare("before_provider_request", &[vec![steps]])
        .await;
    let supplied = if order {
        serde_json::from_str(r#"{"z":-0.0,"a":[null,false,0,"",{},[]]}"#).unwrap()
    } else {
        Value::Null
    };
    let payload = run
        .runner
        .emit_before_provider_request(supplied)
        .await
        .unwrap();
    let negative = |value: &Value| {
        value
            .as_f64()
            .is_some_and(|number| number == 0.0 && number.is_sign_negative())
    };
    if !order {
        return (json!({ "negativeZero": negative(&payload) }), recorded);
    }
    let keys: Vec<_> = payload.as_object().unwrap().keys().cloned().collect();
    let zero = negative(&payload["z"]);
    // The recording's JSON writes a negative zero as 0; its sign is the separate flag.
    let mut written = payload;
    written["z"] = json!(0);
    (
        json!({ "keys": keys, "negativeZero": zero, "payload": written }),
        recorded,
    )
}

/// The combined agent-start result as recorded.
fn combined(result: Option<Combined>) -> Value {
    let Some(result) = result else {
        return Value::Null;
    };
    let mut value = json!({});
    if let Some(messages) = result.messages {
        value["messages"] = encoded(&messages);
    }
    if let Some(prompt) = result.system_prompt {
        value["systemPrompt"] = json!(prompt);
    }
    value
}

/// Starts an agent with the recorded operands.
async fn start(runner: &ExtensionRunner<Local>, prompt: &str, system: &str) -> Value {
    let mut images = images();
    let result = runner
        .emit_before_agent_start(prompt.into(), &mut images, system.into(), options())
        .await
        .unwrap();
    combined(result)
}

/// Messages accumulate and each handler reads the prompt the previous one left, even empty.
async fn agent_start(harness: &Harness, recorded: Value) -> (Value, Value) {
    let message = |custom: &str, content: &str, display: bool| json!({ "customType": custom, "content": content, "display": display });
    let handlers = vec![
        json!([
            observe("prompts"),
            answer(json!({ "systemPrompt": "", "message": message("first", "one", true) }))
        ]),
        json!([
            observe("prompts"),
            answer(json!({ "systemPrompt": "final", "message": message("second", "two", false) }))
        ]),
    ];
    let run = harness.prepare("before_agent_start", &[handlers]).await;
    let result = start(&run.runner, "user", "initial").await;
    let observed = json!({ "result": result, "seen": labels(harness) });
    (outcome(observed, &run), recorded)
}

/// Overlapping agent starts keep their own prompt readers.
async fn concurrent_prompts(harness: &Harness, recorded: Value) -> (Value, Value) {
    let steps = json!([{ "skipUnless": { "prompt": "first", "steps": 2 } }, "wait", observe("contextPrompt"), { "suffix": "!" }]);
    let run = harness.prepare("before_agent_start", &[vec![steps]]).await;
    let runner = run.runner.clone();
    let first = tokio::task::spawn_local(async move { start(&runner, "first", "alpha").await });
    harness.gate.entered.notified().await;
    let second = start(&run.runner, "second", "beta").await;
    harness.gate.release.notify_one();
    let first = first.await.unwrap();
    (
        json!({ "first": first, "second": second, "seen": labels(harness) }),
        recorded,
    )
}

/// Each branch of an agent start sees the untrimmed prompt and the caller's images and options.
async fn agent_branches(harness: &Harness, input: &Value, recorded: Value) -> (Value, Value) {
    let notice = json!({ "customType": "notice", "content": "notice text", "display": true });
    let mut first = vec![observe("agent")];
    match input["variant"].as_str().unwrap() {
        "prompt" => first.push(answer(json!({ "systemPrompt": "" }))),
        "absent" => {}
        _ => first.push(answer(json!({ "message": notice }))),
    }
    let mut handlers = vec![Value::Array(first)];
    if input["variant"] == "error-after-message" {
        handlers.push(json!([fail("agent hook fault")]));
    }
    let run = harness.prepare("before_agent_start", &[handlers]).await;
    let prompt = "\u{feff} user \u{85}";
    let result = start(&run.runner, prompt, "system").await;
    let seen = labels(harness).remove(0);
    let observed = json!({
        "present": !result.is_null(),
        "result": result,
        "imagesIdentity": seen["images"] == encoded(&images()),
        "optionsIdentity": seen["options"] == encoded(&options()),
        "untrimmedPrompt": seen["prompt"] == prompt,
    });
    (outcome(observed, &run), recorded)
}

/// Same-role replacements chain and mark a change; a changed role is reported and skipped.
async fn message_end(harness: &Harness, input: &Value, mut recorded: Value) -> (Value, Value) {
    let steps = match input["mode"].as_str().unwrap() {
        "none" => json!([]),
        "same" => json!(["echo"]),
        "replace" => json!([answer(json!({ "message": assistant("new") }))]),
        _ => json!([answer(json!({ "message": user("wrong") }))]),
    };
    let run = harness.prepare("message_end", &[vec![steps]]).await;
    let mut event: MessageEndEvent = typed(json!({ "message": assistant("original") }));
    let result = run.runner.emit_message_end(&mut event).await.unwrap();
    let observed = json!({
        "message": message_view(&result.as_ref().map_or(Value::Null, encoded)),
        "modified": result.is_some(),
        "original": message_view(&encoded(&event.message)),
    });
    let output = &mut recorded["output"];
    output["message"] = message_view(&output["message"]);
    output["original"] = message_view(&output["original"]);
    (outcome(observed, &run), recorded)
}
