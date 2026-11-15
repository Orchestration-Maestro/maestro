//! Recorded assistant conclusion checks shared by the standard and chat adapters.
use futures_util::FutureExt;
use maestro_http::RequestFailure;
use maestro_models::{
    AssistantMessageEvent, AssistantMessageEventStream, Cancellation, SharedAssistantMessage,
};
use serde_json::{Value, json};
use std::sync::{Arc, RwLock};
/// Recorded source cases, selected without discarding their input fields.
pub(crate) fn rows(kind: &str) -> Vec<Value> {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/assistant-output.json")).unwrap();
    rows.into_iter().filter(|row| row["kind"] == kind).collect()
}

/// A shared message whose identities and content distinguish it from an empty replacement.
pub(crate) fn output() -> SharedAssistantMessage {
    Arc::new(RwLock::new(serde_json::from_value(json!({
        "role":"assistant", "api":"custom", "provider":"fixture", "model":"model",
        "content":[{"type":"text","text":"partial"}], "timestamp":1,
        "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
        "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}, "stopReason":"stop"
    })).unwrap()))
}

/// Exercise conclusion at the policy boundary before any failure is published.
pub(crate) fn check_conclusions(
    chat: bool,
    conclude: fn(
        &AssistantMessageEventStream,
        &SharedAssistantMessage,
        Option<&Cancellation>,
    ) -> Result<(), RequestFailure>,
) {
    let rows: Vec<_> = rows("conclusion")
        .into_iter()
        .filter(|row| (row["input"]["owner"] == "chat") == chat)
        .collect();
    assert_eq!(rows.len(), 9);
    for row in rows {
        let output = output();
        {
            let mut message = output.write().unwrap();
            message.stop_reason =
                serde_json::from_value(row["input"]["stopReason"].clone()).unwrap();
            message.error_message =
                serde_json::from_value(row["input"]["errorMessage"].clone()).unwrap();
        }
        let stream = AssistantMessageEventStream::new();
        let signal = Cancellation::new();
        if row["input"]["aborted"].as_bool().unwrap() {
            signal.abort();
        }
        let result = conclude(&stream, &output, Some(&signal));
        let expected = &row["expected"];
        if let Some(error) = expected.get("error") {
            assert_eq!(result.unwrap_err().message, error.as_str().unwrap());
            assert_eq!(expected["events"], json!([]));
            assert_eq!(expected["ended"], false);
            assert!(stream.next().now_or_never().is_none());
            assert!(stream.result().now_or_never().is_none());
        } else {
            result.unwrap();
            let Some(AssistantMessageEvent::Done { reason, message }) =
                stream.next().now_or_never().unwrap()
            else {
                panic!("missing done event")
            };
            assert_eq!(
                json!([{"type":"done", "reason":reason, "sameMessage":Arc::ptr_eq(&message, &output)}]),
                expected["events"]
            );
            assert_eq!(
                json!(stream.next().now_or_never().unwrap().is_none()),
                expected["ended"]
            );
            assert!(Arc::ptr_eq(
                &stream.result().now_or_never().unwrap(),
                &output
            ));
        }
    }
}
