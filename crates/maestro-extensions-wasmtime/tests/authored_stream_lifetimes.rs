//! A pending compiled-guest producer outlives observers and releases its containing owner.
#![cfg(test)]
use maestro_extensions::{
    ExtensionActions, ExtensionCommandContextActions, ExtensionContextActions, ExtensionExecution,
    ExtensionRunner, Local, create_event_bus_for, types::ProviderActions,
};
use std::{cell::RefCell, rc::Rc};

/// Native destruction receipts, independent of guest-call settlement order.
#[derive(Default)]
struct Observations {
    /// Actual application-action effects in arrival order.
    labels: RefCell<Vec<String>>,
    /// Acknowledges actual effects instead of guessing when the Store is quiescent.
    changed: tokio::sync::Notify,
}

/// Invoke the original registration, then admit a separate Store turn.
async fn step(runner: &ExtensionRunner<Local>, action: &str) {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    for action in [action, "barrier"] {
        assert_eq!(
            command(
                format!("stream-lifetime {action}"),
                runner.create_command_context(),
            )
            .await
            .unwrap_err()
            .to_string(),
            action
        );
    }
}

/// Bind the actual application destruction observations and one acknowledged producer gate.
fn bind(
    runner: &ExtensionRunner<Local>,
    trace: &Rc<Observations>,
    signal: maestro_extensions::Cancellation,
) -> tokio::sync::mpsc::UnboundedReceiver<tokio::sync::oneshot::Sender<()>> {
    let observed = trace.clone();
    runner
        .bind_core(
            ExtensionActions {
                set_session_name: Rc::new(move |name| {
                    observed.labels.borrow_mut().push(name);
                    observed.changed.notify_one();
                    Ok(())
                }),
                ..Local::unbound_actions()
            },
            ExtensionContextActions {
                get_signal: Rc::new(move || Ok(Some(signal.clone()))),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
    let (entered, waits) = tokio::sync::mpsc::unbounded_channel();
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        maestro_extensions::CommandContextOperations {
            wait_for_idle: Rc::new(move || {
                let (resume, result) = tokio::sync::oneshot::channel();
                entered.send(resume).unwrap();
                Box::pin(async move {
                    result.await?;
                    Ok(())
                })
            }),
            ..super::context_actions::silent_command_operations()
        },
    )));
    waits
}

/// Count exact destruction effects without imposing ordering on independent releases.
async fn released(trace: &Rc<Observations>, expected: &[&str]) {
    while trace.labels.borrow().len() < expected.len() {
        trace.changed.notified().await;
    }
    let mut observed = trace.labels.borrow().clone();
    observed.sort();
    assert_eq!(observed, expected);
}

#[test]
fn dropped_result_observers_leave_the_producer_running() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        for case in ["one", "all", "cancel"] {
            independent_owners(case).await;
        }
    });
}

/// Exercise real pending work, all observer drops, final aliases and containing-owner release.
async fn independent_owners(case: &str) {
    let cancelled = case == "cancel";
    let errors = Rc::new(RefCell::new(Vec::new()));
    let (reporter, teardown) = super::lifetimes::observed_reporter(&errors);
    let bus = create_event_bus_for::<Local>(Rc::new(|_| panic!("no tails")), reporter);
    let (owner, runner, events) = super::retained_components::loaded_on(bus.for_extension()).await;
    let trace = Rc::new(Observations::default());
    let signal = maestro_extensions::Cancellation::new();
    let mut waits = bind(&runner, &trace, signal.clone());
    step(&runner, "create").await;
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let producer = command(
        "stream-lifetime produce".into(),
        runner.create_command_context(),
    );
    let control = async {
        let resume = waits.recv().await.unwrap();
        step(&runner, "drop-one").await;
        released(&trace, &[]).await;
        if case != "one" {
            step(&runner, "drop-all").await;
            released(&trace, &[]).await;
        }
        maestro_extensions::Cancellation::new().abort();
        released(&trace, &[]).await;
        if cancelled {
            signal.abort();
            step(&runner, "observe-aborted").await;
            released(&trace, &["producer-aborted"]).await;
        }
        resume.send(()).unwrap();
    };
    let (outcome, ()) = futures_util::future::join(producer, control).await;
    assert_eq!(outcome.unwrap_err().to_string(), "produce");
    let (before, after) = completed(&runner, &trace, cancelled).await;
    release_aliases(&runner, &trace, before, after).await;
    drop(command);
    bus.clear();
    drop((owner, runner, events, bus));
    teardown.await.unwrap();
    assert!(errors.borrow().is_empty());
    released(&trace, after).await;
}

/// Check exact completion effects and return the literal final-alias expectations.
async fn completed(
    runner: &ExtensionRunner<Local>,
    trace: &Rc<Observations>,
    cancelled: bool,
) -> (&'static [&'static str], &'static [&'static str]) {
    if cancelled {
        released(trace, &["producer-aborted", "producer-complete", "queued"]).await;
        (
            &[
                "classifier",
                "extractor",
                "producer-aborted",
                "producer-complete",
                "queued",
            ],
            &[
                "aborted-result",
                "classifier",
                "extractor",
                "producer-aborted",
                "producer-complete",
                "queued",
            ],
        )
    } else {
        step(runner, "observe").await;
        released(trace, &["producer-complete", "queued", "terminal"]).await;
        (
            &[
                "classifier",
                "extractor",
                "producer-complete",
                "queued",
                "terminal",
            ],
            &[
                "classifier",
                "extractor",
                "producer-complete",
                "queued",
                "result",
                "terminal",
            ],
        )
    }
}

/// The result remains usable after its source and after another result alias are dropped.
async fn release_aliases(
    runner: &ExtensionRunner<Local>,
    trace: &Rc<Observations>,
    before: &[&str],
    after: &[&str],
) {
    step(runner, "release-stream").await;
    released(trace, before).await;
    step(runner, "release-result").await;
    released(trace, before).await;
    step(runner, "release-result").await;
    released(trace, after).await;
}
