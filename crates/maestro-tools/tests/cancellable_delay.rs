//! Cancellable delay observed through its public operation.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::process_support;

use futures_util::FutureExt;
use maestro_models::Cancellation;
use maestro_tools::{SleepError, sleep};
use process_support::{poll_once, polled_to_completion};
use std::task::Poll;
use std::time::Duration;

/// A single-threaded runtime whose clock starts paused.
fn paused_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
}

#[test]
fn delay_obeys_its_selected_duration() {
    paused_runtime().block_on(async {
        let live = Cancellation::new();
        for signal in [None, Some(&live)] {
            let wait = sleep(Duration::from_millis(25), signal);
            tokio::pin!(wait);
            assert!(poll_once(&mut wait).await.is_pending());
            tokio::time::advance(Duration::from_millis(24)).await;
            assert!(poll_once(&mut wait).await.is_pending());
            tokio::time::advance(Duration::from_millis(1)).await;
            assert_eq!(poll_once(&mut wait).await, Poll::Ready(Ok(())));
            assert_eq!(sleep(Duration::ZERO, signal).await, Ok(()));
        }
    });
}

#[test]
fn preaborted_delay_does_not_arm_a_timer() {
    let aborted = Cancellation::new();
    aborted.abort();
    for duration in [Duration::ZERO, Duration::from_millis(25)] {
        assert_eq!(
            sleep(duration, Some(&aborted)).now_or_never(),
            Some(Err(SleepError))
        );
    }
    assert_eq!(SleepError.to_string(), "Aborted");
}

#[test]
fn active_abort_releases_delay_with_exact_error() {
    paused_runtime().block_on(async {
        let signal = Cancellation::new();
        let wait = sleep(Duration::from_millis(25), Some(&signal));
        tokio::pin!(wait);
        assert!(poll_once(&mut wait).await.is_pending());
        signal.abort();
        assert_eq!(
            poll_once(&mut wait).await,
            Poll::Ready(Err(SleepError)),
            "settled by the abort, not by the elapsed time"
        );
    });
}

#[test]
fn completed_delay_releases_cancellation_observer() {
    paused_runtime().block_on(async {
        let signal = Cancellation::new();
        let (result, forward) =
            polled_to_completion(sleep(Duration::from_millis(25), Some(&signal))).await;
        assert_eq!(result, Ok(()));
        assert_eq!(std::sync::Arc::strong_count(&forward), 1);
        signal.abort();
        assert_eq!(std::sync::Arc::strong_count(&forward), 1);
    });
}
