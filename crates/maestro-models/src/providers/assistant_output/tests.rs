//! Recorded publication and conclusion cases at their owning boundaries.
use super::*;
use serde_json::{Value, json};
use std::future::Future;
use std::pin::pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};

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
    assert_eq!(rows.len(), 2);
    for (row, typed) in rows.iter().flat_map(|row| [(row, false), (row, true)]) {
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
        if typed {
            fail(&stream, &output, Some(&signal), RequestFailure::new(text));
        } else {
            publish_failure(&stream, &output, Some(&signal), text);
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

use crate::tests::conclusions::{check_conclusions, output, rows};
use futures_util::FutureExt;

#[test]
fn standard_conclusion_preserves_recorded_outcomes() {
    check_conclusions(false, conclude);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn producer_completion_outlives_observers() {
    crate::tests::runtime::run(async {
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
        for ((release, completed), output) in releases.into_iter().zip(completions).zip(outputs) {
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
