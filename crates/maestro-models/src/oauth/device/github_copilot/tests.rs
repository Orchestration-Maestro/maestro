//! Device authorization with a controlled wall clock and native timer ownership.
#![cfg(test)]
use super::*;
use crate::{Cancellation, HttpResponse, OAuthLoginCallbacks};
use futures_util::FutureExt as _;
use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

/// Interaction witness for publication and cancellation.
#[derive(Default)]
struct Interaction {
    /// Published authorizations.
    auth: AtomicUsize,
    /// Caller signal.
    signal: Cancellation,
    /// Abort at authorization publication.
    abort_auth: bool,
}
impl OAuthLoginCallbacks for Interaction {
    fn on_prompt(&self, _: OAuthPrompt) -> BoxFuture<Result<String, OAuthError>> {
        Box::pin(async { Ok(String::new()) })
    }
    fn on_auth(&self, _: OAuthAuthInfo) -> Result<(), OAuthError> {
        self.auth.fetch_add(1, Ordering::SeqCst);
        if self.abort_auth {
            self.signal.abort();
        }
        Ok(())
    }
    fn signal(&self) -> Option<&Cancellation> {
        Some(&self.signal)
    }
}

/// Run a controlled native timer scenario.
fn run(work: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            tokio::time::pause();
            work.await;
        });
}

/// Device response with supplied numeric spellings.
fn device(interval: &str, lifetime: &str) -> String {
    format!(
        r#"{{"device_code":"dev","user_code":"code","verification_uri":"url","interval":{interval},"expires_in":{lifetime}}}"#
    )
}

/// Read a complete controlled response.
fn response(text: String) -> HttpResponse {
    HttpResponse {
        status: 200,
        status_text: String::new(),
        headers: std::collections::BTreeMap::new(),
        body: Box::pin(futures_util::stream::iter([Ok(text.into_bytes())])),
    }
}

/// Execute the actual login policy with a finite sequence of wall-clock observations.
async fn exercise(
    device: String,
    polls: Vec<&str>,
    reads: Vec<f64>,
    waits: &[f64],
) -> Result<OAuthCredentials, OAuthError> {
    let interaction = Arc::new(Interaction::default());
    exercise_with(interaction, device, polls, reads, waits).await
}

/// Drive exact requested waits at the owned login seam, allowing one native timer tick.
async fn exercise_with(
    interaction: Arc<Interaction>,
    device: String,
    polls: Vec<&str>,
    reads: Vec<f64>,
    waits: &[f64],
) -> Result<OAuthCredentials, OAuthError> {
    let bodies = Mutex::new(VecDeque::from(
        polls.into_iter().map(str::to_owned).collect::<Vec<_>>(),
    ));
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let fetch: Fetch = Arc::new(move |request| {
        let text = if request.url.ends_with("/device/code") {
            device.clone()
        } else if request.url.ends_with("/access_token") {
            observed.fetch_add(1, Ordering::SeqCst);
            bodies.lock().unwrap().pop_front().expect("unexpected poll")
        } else if request.url.ends_with("/token") {
            r#"{"token":"service","expires_at":1}"#.into()
        } else {
            String::new()
        };
        Box::pin(async move { Ok(response(text)) })
    });
    let samples = Mutex::new(VecDeque::from(reads));
    let clock = || {
        samples
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected wall-clock read")
    };
    let mut login = Box::pin(login_with_clock(interaction, fetch, clock));
    let mut output = login.as_mut().now_or_never();
    for (index, &wait) in waits.iter().enumerate() {
        assert!(output.is_none(), "completed before requested wait {index}");
        let duration = Duration::try_from_secs_f64(wait / 1000.0).unwrap();
        if let Some(before) = duration.checked_sub(Duration::from_nanos(1)) {
            tokio::time::advance(before).await;
            assert!(login.as_mut().now_or_never().is_none());
            assert_eq!(count.load(Ordering::SeqCst), index);
            tokio::time::advance(Duration::from_nanos(1) + Duration::from_millis(1)).await;
        } else {
            tokio::time::advance(Duration::from_millis(1)).await;
        }
        output = login.as_mut().now_or_never();
        assert_eq!(count.load(Ordering::SeqCst), index + 1);
    }
    assert!(samples.lock().unwrap().is_empty(), "unused clock sample");
    output.expect("controlled operation settled")
}

#[test]
fn maestro_device_login_uses_remaining_lifetime() {
    run(maestro_device_login_uses_remaining_lifetime_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn maestro_device_login_uses_remaining_lifetime_scenario() {
    let error = exercise(
        device("5", "25"),
        vec![
            r#"{"error":"slow_down","interval":10}"#,
            r#"{"error":"slow_down","interval":15}"#,
            r#"{"error":"authorization_pending"}"#,
        ],
        vec![0.0, 0.0, 0.0, 6000.0, 6000.0, 20000.0, 20000.0, 25000.0],
        &[6000.0, 14000.0, 5000.0],
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Device flow timed out after one or more slow_down responses. This is often caused by clock drift in WSL or VM environments. Please sync or restart the VM clock and try again."
    );
    assert_eq!(
        exercise(
            device("5", "1"),
            vec![r#"{"access_token":"final"}"#],
            vec![0.0, 0.0, 0.0],
            &[1000.0]
        )
        .await
        .unwrap()
        .refresh,
        "final"
    );
}

#[test]
fn device_polling_rounds_clamps_and_resamples_clock() {
    run(device_polling_rounds_clamps_and_resamples_clock_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_polling_rounds_clamps_and_resamples_clock_scenario() {
    for (interval, wait) in [
        ("-2", 1200.0),
        ("0", 1200.0),
        ("0.9999", 1200.0),
        ("1", 1200.0),
        ("1.0009", 1200.0),
        ("1.001", 1200.0),
        ("5.0001", 6000.0),
    ] {
        assert_eq!(
            exercise(
                device(interval, "900"),
                vec![r#"{"access_token":"ok"}"#],
                vec![0.0, 0.0, 0.0],
                &[wait]
            )
            .await
            .unwrap()
            .refresh,
            "ok"
        );
    }
    let success = exercise(
        device("5", "900"),
        vec![
            r#"{"error":"authorization_pending"}"#,
            r#"{"error":"slow_down","interval":10}"#,
            r#"{"access_token":"ok"}"#,
        ],
        vec![0.0, 0.0, 0.0, 6000.0, 6000.0, 12000.0, 12000.0],
        &[6000.0, 6000.0, 14000.0],
    )
    .await
    .unwrap();
    assert_eq!(success.refresh, "ok");
    clock_adjustments().await;
}

#[test]
fn device_slow_down_accepts_positive_interval_or_adds_delay() {
    run(device_slow_down_accepts_positive_interval_or_adds_delay_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_slow_down_accepts_positive_interval_or_adds_delay_scenario() {
    for (interval, wait) in [
        (None, 14000.0),
        (Some("null"), 14000.0),
        (Some("0"), 14000.0),
        (Some("-1"), 14000.0),
        (Some(r#""10""#), 14000.0),
        (Some("0.0005"), 1.0),
        (Some("0.5"), 700.0),
        (Some("10.1234"), 14173.0),
    ] {
        let slow = format!(
            r#"{{"error":"slow_down"{},"error_description":"\ud800"}}"#,
            interval.map_or_else(String::new, |raw| format!(",\"interval\":{raw}"))
        );
        let result = exercise(
            device("5", "900"),
            vec![
                &slow,
                r#"{"error":"authorization_pending"}"#,
                r#"{"access_token":"ok"}"#,
            ],
            vec![0.0, 0.0, 0.0, 6000.0, 6000.0, 6000.0 + wait, 6000.0 + wait],
            &[6000.0, wait, wait],
        )
        .await
        .unwrap();
        assert_eq!(result.refresh, "ok");
    }
    for (first, second_wait, third_wait) in [
        (r#"{"error":"slow_down"}"#, 14000.0, 21000.0),
        (r#"{"error":"slow_down","interval":0.5}"#, 700.0, 7700.0),
    ] {
        assert_eq!(
            exercise(
                device("5", "900"),
                vec![
                    first,
                    r#"{"error":"slow_down"}"#,
                    r#"{"access_token":"ok"}"#
                ],
                vec![
                    0.0,
                    0.0,
                    0.0,
                    6000.0,
                    6000.0,
                    6000.0 + second_wait,
                    6000.0 + second_wait
                ],
                &[6000.0, second_wait, third_wait]
            )
            .await
            .unwrap()
            .refresh,
            "ok"
        );
    }
}

#[test]
fn device_timeouts_depend_on_observed_slow_down() {
    run(device_timeouts_depend_on_observed_slow_down_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_timeouts_depend_on_observed_slow_down_scenario() {
    for lifetime in ["-1", "0"] {
        let interaction = Arc::new(Interaction {
            abort_auth: true,
            ..Default::default()
        });
        let result = exercise_with(
            interaction.clone(),
            device("5", lifetime),
            vec![],
            vec![0.0, 0.0],
            &[],
        )
        .await
        .unwrap_err();
        assert_eq!(result.to_string(), "Device flow timed out");
        assert_eq!(interaction.auth.load(Ordering::SeqCst), 1);
    }
    for (lifetime, wait) in [("0.0005", 0.5), ("1", 1000.0), ("1.2", 1200.0)] {
        assert_eq!(
            exercise(
                device("5", lifetime),
                vec![r#"{"error":"authorization_pending"}"#],
                vec![0.0, 0.0, 0.0, wait],
                &[wait]
            )
            .await
            .unwrap_err()
            .to_string(),
            "Device flow timed out"
        );
    }
    assert!(
        exercise(
            device("5", "1"),
            vec![r#"{"error":"slow_down"}"#],
            vec![0.0, 0.0, 0.0, 1000.0],
            &[1000.0]
        )
        .await
        .unwrap_err()
        .to_string()
        .starts_with("Device flow timed out after one or more slow_down responses.")
    );
    assert_eq!(
        exercise(
            device("5", "1"),
            vec![r#"{"error":"expired_token","error_description":"gone"}"#],
            vec![0.0, 0.0, 0.0],
            &[1000.0]
        )
        .await
        .unwrap_err()
        .to_string(),
        "Device flow failed: expired_token: gone"
    );
}

#[test]
fn device_long_and_fractional_waits_avoid_runtime_clamping() {
    run(device_long_and_fractional_waits_avoid_runtime_clamping_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_long_and_fractional_waits_avoid_runtime_clamping_scenario() {
    for (interval, lifetime, wait) in [
        ("3e6", "4e6", 3_600_000_000.0),
        ("5", "0.0005", 0.5),
        ("1e100", "1", 1000.0),
    ] {
        assert_eq!(
            exercise(
                device(interval, lifetime),
                vec![r#"{"access_token":"ok"}"#],
                vec![0.0, 0.0, 0.0],
                &[wait]
            )
            .await
            .unwrap()
            .refresh,
            "ok"
        );
    }
}

#[test]
fn device_invalid_durations_fail_at_the_consuming_branch() {
    run(device_invalid_durations_fail_at_the_consuming_branch_scenario());
}

/// Caller Waker with destruction and notification witnesses.
struct Observer {
    /// Number of destroyed observers.
    drops: Arc<AtomicUsize>,
    /// Number of delivered wake calls.
    wakes: AtomicUsize,
}
impl std::task::Wake for Observer {
    fn wake(self: Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }
}
impl Drop for Observer {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

/// Poll with a distinct observer identity, releasing the caller's temporary Waker.
fn observe<F: std::future::Future>(
    future: std::pin::Pin<&mut F>,
    witness: &Arc<Observer>,
) -> std::task::Poll<F::Output> {
    let waker = std::task::Waker::from(witness.clone());
    future.poll(&mut std::task::Context::from_waker(&waker))
}

/// Controlled scenario for the named authorization behavior.
async fn device_invalid_durations_fail_at_the_consuming_branch_scenario() {
    for (interval, lifetime, reads, auth) in [
        ("1e400", "900", vec![], 0),
        ("5", "1e400", vec![], 0),
        ("1e308", "900", vec![0.0], 1),
        ("5", "1e308", vec![0.0], 1),
        ("1e100", "1e100", vec![0.0, 0.0, 0.0], 1),
        ("1.7e305", "900", vec![0.0, 0.0, 0.0], 1),
    ] {
        let interaction = Arc::new(Interaction::default());
        let result = exercise_with(
            interaction.clone(),
            device(interval, lifetime),
            vec![],
            reads,
            &[],
        )
        .await
        .unwrap_err();
        assert_eq!(result.to_string(), "Invalid device code response fields");
        assert_eq!(interaction.auth.load(Ordering::SeqCst), auth);
    }
    for raw in ["1e400", "1e308"] {
        let slow = format!(r#"{{"error":"slow_down","interval":{raw}}}"#);
        assert_eq!(
            exercise(
                device("5", "900"),
                vec![&slow],
                vec![0.0, 0.0, 0.0],
                &[6000.0]
            )
            .await
            .unwrap_err()
            .to_string(),
            "Invalid device code response fields"
        );
        let text = format!(r#"{{"token":"ok","expires_at":{raw}}}"#);
        let fetch: Fetch = Arc::new(move |_| {
            let text = text.clone();
            Box::pin(async move { Ok(response(text)) })
        });
        assert_eq!(
            refresh_github_copilot_token("r".into(), None, Some(fetch))
                .await
                .unwrap_err()
                .to_string(),
            "Invalid Copilot token response fields"
        );
    }
}

#[test]
fn device_waits_release_cancellation_observers() {
    run(device_waits_release_cancellation_observers_scenario());
}

/// Controlled scenario for the named authorization behavior.
async fn device_waits_release_cancellation_observers_scenario() {
    let interaction = Arc::new(Interaction::default());
    let fetch = cleanup_fetch();
    let reads = Mutex::new(VecDeque::from([
        0.0, 0.0, 0.0, 6000.0, 6000.0, 12000.0, 12000.0,
    ]));
    let mut login = Box::pin(login_with_clock(interaction.clone(), fetch, || {
        reads.lock().unwrap().pop_front().unwrap()
    }));
    let drops = Arc::new(AtomicUsize::new(0));
    let witness = || {
        Arc::new(Observer {
            drops: drops.clone(),
            wakes: AtomicUsize::new(0),
        })
    };
    let control = witness();
    let mut independent = Box::pin(interaction.signal.cancelled());
    assert!(observe(independent.as_mut(), &control).is_pending());
    let mut previous = witness();
    assert!(observe(login.as_mut(), &previous).is_pending());
    for (index, wait) in [6000, 6000, 14000].into_iter().enumerate() {
        assert!(Arc::strong_count(&previous) > 1);
        tokio::time::advance(Duration::from_millis(wait + 1)).await;
        let next = witness();
        let result = observe(login.as_mut(), &next);
        assert_eq!(
            Arc::strong_count(&previous),
            1,
            "completed wait released caller Waker"
        );
        drop(previous);
        assert_eq!(drops.load(Ordering::SeqCst), index + 1);
        assert!(
            Arc::strong_count(&control) > 1,
            "independent observer remains registered"
        );
        if index == 2 {
            let std::task::Poll::Ready(result) = result else {
                panic!("login has not settled")
            };
            assert_eq!(result.unwrap().refresh, "ok");
        } else {
            assert!(result.is_pending());
        }
        previous = next;
    }
    let before = previous.wakes.load(Ordering::SeqCst);
    interaction.signal.abort();
    assert_eq!(previous.wakes.load(Ordering::SeqCst), before);
    assert!(control.wakes.load(Ordering::SeqCst) > 0);
    assert!(observe(independent.as_mut(), &control).is_ready());
    later_wait_cancels(&previous);
}

/// Forward, backward and straddled wall-clock observations.
async fn clock_adjustments() {
    assert_eq!(
        exercise(
            device("5", "9"),
            vec![r#"{"error":"authorization_pending"}"#],
            vec![0.0, 0.0, 0.0, 11000.0],
            &[6000.0]
        )
        .await
        .unwrap_err()
        .to_string(),
        "Device flow timed out"
    );
    assert_eq!(
        exercise(
            device("5", "9"),
            vec![
                r#"{"error":"authorization_pending"}"#,
                r#"{"access_token":"ok"}"#
            ],
            vec![0.0, 0.0, 0.0, 4000.0, 4000.0],
            &[6000.0, 5000.0]
        )
        .await
        .unwrap()
        .refresh,
        "ok"
    );
    for (body, expected) in [
        (r#"{"access_token":"ok"}"#, Ok("ok")),
        (
            r#"{"error":"expired_token"}"#,
            Err("Device flow failed: expired_token"),
        ),
    ] {
        let result = exercise(
            device("5", "0.001"),
            vec![body],
            vec![0.0, 0.0, 2.0],
            &[0.0],
        )
        .await;
        assert_eq!(
            result
                .map(|credentials| credentials.refresh)
                .map_err(|error| error.to_string()),
            expected.map(str::to_owned).map_err(str::to_owned)
        );
    }
}

/// A fresh wait still observes abort after earlier observer cleanup.
fn later_wait_cancels(previous: &Arc<Observer>) {
    let later = Arc::new(Interaction::default());
    let fetch: Fetch = Arc::new(|_| Box::pin(async { Ok(response(device("5", "900"))) }));
    let mut login = Box::pin(login_with_clock(later.clone(), fetch, || 0.0));
    assert!(observe(login.as_mut(), previous).is_pending());
    later.signal.abort();
    let std::task::Poll::Ready(result) = observe(login.as_mut(), previous) else {
        panic!("cancellation has not settled")
    };
    assert_eq!(result.unwrap_err().to_string(), "Login cancelled");
    assert_eq!(Arc::strong_count(previous), 1);
}

/// Multi-cycle responses for the observer cleanup witness.
fn cleanup_fetch() -> Fetch {
    let bodies = Mutex::new(VecDeque::from([
        device("5", "900"),
        r#"{"error":"authorization_pending"}"#.into(),
        r#"{"error":"slow_down","interval":10}"#.into(),
        r#"{"access_token":"ok"}"#.into(),
        r#"{"token":"service","expires_at":1}"#.into(),
    ]));
    Arc::new(move |request| {
        let text = if request.url.ends_with("/policy") {
            String::new()
        } else {
            bodies.lock().unwrap().pop_front().unwrap()
        };
        Box::pin(async move { Ok(response(text)) })
    })
}
