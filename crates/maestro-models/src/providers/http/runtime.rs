//! Timers, detached tasks and racing that behave alike on native and browser targets.

use std::future::{Future, pending, poll_fn};
use std::pin::pin;
use std::task::Poll;
use std::time::Duration;

use crate::Cancellation;

/// How racing work against a timer and a cancellation signal ended.
pub(crate) enum Raced<T> {
    /// The work finished first.
    Done(T),
    /// The timer elapsed first.
    TimedOut,
    /// The signal was aborted first.
    Cancelled,
}

/// Run `work` until it finishes, the optional `timeout` elapses or the `signal` aborts.
pub(crate) async fn race<T>(
    work: impl Future<Output = T>,
    timeout: Option<Duration>,
    signal: Option<&Cancellation>,
) -> Raced<T> {
    let mut work = pin!(work);
    let mut timer = pin!(async {
        match timeout {
            Some(duration) => sleep(duration).await,
            None => pending().await,
        }
    });
    let mut cancelled = pin!(async {
        match signal {
            Some(signal) => signal.cancelled().await,
            None => pending().await,
        }
    });
    poll_fn(|context| {
        if let Poll::Ready(output) = work.as_mut().poll(context) {
            return Poll::Ready(Raced::Done(output));
        }
        if cancelled.as_mut().poll(context).is_ready() {
            return Poll::Ready(Raced::Cancelled);
        }
        if timer.as_mut().poll(context).is_ready() {
            return Poll::Ready(Raced::TimedOut);
        }
        Poll::Pending
    })
    .await
}

/// Wait for `duration` on the Tokio clock.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

/// Wait for `duration` with browser timers, which accept at most `i32::MAX` milliseconds.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn sleep(duration: Duration) {
    const LONGEST: Duration = Duration::from_millis(2_147_483_647);
    let mut remaining = duration;
    while !remaining.is_zero() {
        let step = remaining.min(LONGEST);
        wait(step).await;
        remaining -= step;
    }
}

/// Resolve after one browser timer of at most `i32::MAX` milliseconds.
#[cfg(target_arch = "wasm32")]
async fn wait(duration: Duration) {
    use wasm_bindgen::{JsCast, JsValue};

    let millis = f64::from(i32::try_from(duration.as_millis()).unwrap_or(i32::MAX));
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let global = js_sys::global();
        let timer = js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout")).ok();
        if let Some(function) = timer.as_ref().and_then(|t| t.dyn_ref::<js_sys::Function>()) {
            function
                .call2(&global, &resolve, &JsValue::from_f64(millis))
                .ok();
        }
    });
    wasm_bindgen_futures::JsFuture::from(promise).await.ok();
}

/// Start request work that outlives the reader of its events; report whether it started.
///
/// Native targets need a Tokio runtime on the calling thread.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn spawn_detached(task: impl Future<Output = ()> + Send + 'static) -> bool {
    tokio::runtime::Handle::try_current()
        .map(|runtime| drop(runtime.spawn(task)))
        .is_ok()
}

/// Start request work that outlives the reader of its events; it always starts.
#[cfg(target_arch = "wasm32")]
pub(crate) fn spawn_detached(task: impl Future<Output = ()> + 'static) -> bool {
    wasm_bindgen_futures::spawn_local(task);
    true
}

/// Draw a number in `[0, 1)`.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn unit_random() -> f64 {
    use std::hash::{BuildHasher, RandomState};

    let high = u32::try_from(RandomState::new().hash_one(0_u8) >> 32).unwrap_or(0);
    f64::from(high) / 4_294_967_296.0
}

/// Draw a number in `[0, 1)`.
#[cfg(target_arch = "wasm32")]
pub(crate) fn unit_random() -> f64 {
    js_sys::Math::random()
}
