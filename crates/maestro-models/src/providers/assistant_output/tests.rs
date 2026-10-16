//! Recorded publication and conclusion cases at their owning boundaries.
use super::*;
use serde_json::{Value, json};
use std::future::Future;
use std::pin::pin;
use std::sync::{
    RwLock,
    atomic::{AtomicUsize, Ordering},
};
use std::task::{Context, Poll, Wake, Waker};

/// Recorded source cases, selected without discarding their input fields.
fn rows(kind: &str) -> Vec<Value> {
    let rows: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/assistant-output.json"
    ))
    .unwrap();
    rows.into_iter().filter(|row| row["kind"] == kind).collect()
}

/// A shared message whose identities and content distinguish it from an empty replacement.
fn output() -> SharedAssistantMessage {
    Arc::new(RwLock::new(serde_json::from_value(json!({
        "role":"assistant", "api":"custom", "provider":"fixture", "model":"model",
        "content":[{"type":"text","text":"partial"}], "timestamp":1,
        "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
        "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}, "stopReason":"stop"
    })).unwrap()))
}

/// A waker that observes that publication runs outside the message lock.
struct Unlocked {
    /// Same message as the terminal event.
    output: SharedAssistantMessage,
    /// Number of notifications delivered.
    wakes: AtomicUsize,
}
impl Wake for Unlocked {
    fn wake(self: Arc<Self>) {
        assert!(self.output.try_write().is_ok());
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn failure_publication_preserves_identity_and_unlocks() {
    let rows = rows("failure");
    assert_eq!(rows.len(), 16);
    for (index, row) in rows.iter().enumerate() {
        assert!(matches!(
            row["input"]["owner"].as_str(),
            Some(
                "chat"
                    | "messages"
                    | "responses"
                    | "sessions"
                    | "conversation"
                    | "content"
                    | "azure"
                    | "bedrock"
            )
        ));
        let output = output();
        let stream = AssistantMessageEventStream::new();
        let signal = Cancellation::new();
        if row["input"]["aborted"].as_bool().unwrap() {
            signal.abort();
        }
        let observer = Arc::new(Unlocked {
            output: Arc::clone(&output),
            wakes: AtomicUsize::new(0),
        });
        let waker = Waker::from(Arc::clone(&observer));
        let mut context = Context::from_waker(&waker);
        let mut next = pin!(stream.next());
        assert!(next.as_mut().poll(&mut context).is_pending());
        let text = row["input"]["message"].as_str().unwrap().to_owned();
        if index % 2 == 0 {
            publish_failure(&stream, &output, Some(&signal), text);
        } else {
            fail(&stream, &output, Some(&signal), RequestFailure::new(text));
        }
        assert!(observer.wakes.load(Ordering::SeqCst) > 0);
        let Poll::Ready(Some(AssistantMessageEvent::Error { reason, error })) =
            next.as_mut().poll(&mut context)
        else {
            panic!("missing terminal event")
        };
        assert_eq!(
            json!([{"type":"error", "reason":reason,"sameMessage":Arc::ptr_eq(&output, &error)}]),
            row["expected"]["events"]
        );
        assert_failure_output(&output, &row["expected"]["output"]);
        assert_eq!(
            json!(stream.next().now_or_never().unwrap().is_none()),
            row["expected"]["ended"]
        );
        assert!(Arc::ptr_eq(
            &stream.result().now_or_never().unwrap(),
            &output
        ));
    }
}

/// The partial content survives while the failure fields are replaced.
fn assert_failure_output(output: &SharedAssistantMessage, expected: &Value) {
    let actual = output.read().unwrap();
    assert_eq!(
        serde_json::to_value(&actual.content).unwrap(),
        expected["content"]
    );
    assert_eq!(
        serde_json::to_value(&actual.stop_reason).unwrap(),
        expected["stopReason"]
    );
    assert_eq!(json!(actual.error_message), expected["errorMessage"]);
    drop(actual);
}

use futures_util::FutureExt;

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
    assert_eq!(rows.len(), if chat { 9 } else { 58 });
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

#[test]
fn standard_conclusion_preserves_recorded_outcomes() {
    check_conclusions(false, conclude);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn producer_completion_outlives_observers() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut outputs = Vec::new();
            let mut releases = Vec::new();
            let mut completions = Vec::new();
            for _ in 0..2 {
                let output = output();
                let stream = AssistantMessageEventStream::new();
                let (entered, ready) = tokio::sync::oneshot::channel();
                let (release, proceed) = tokio::sync::oneshot::channel();
                let (complete, completed) = tokio::sync::oneshot::channel();
                let retained = Arc::clone(&output);
                let producer = stream.clone();
                start_producer_for::<crate::Threaded>(
                    &stream,
                    &output,
                    None,
                    Box::pin(async move {
                        entered.send(()).unwrap();
                        proceed.await.unwrap();
                        retained.write().unwrap().error_message = Some("completed".into());
                        producer.end(Some(Arc::clone(&retained)));
                        complete.send(retained).unwrap();
                    }),
                    None,
                );
                ready.await.unwrap();
                drop(stream);
                releases.push(release);
                completions.push(completed);
                outputs.push(output);
            }
            assert!(!Arc::ptr_eq(&outputs[0], &outputs[1]));
            for ((release, completed), output) in releases.into_iter().zip(completions).zip(outputs)
            {
                release.send(()).unwrap();
                let retained = completed.await.unwrap();
                assert!(Arc::ptr_eq(&retained, &output));
                assert_eq!(
                    output.read().unwrap().error_message.as_deref(),
                    Some("completed")
                );
            }
        });
}
