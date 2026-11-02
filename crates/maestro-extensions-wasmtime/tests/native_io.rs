//! Native waits cross the compiled guest and production request pump.
#![cfg(test)]
use super::example_process::run;
use maestro_extensions::{
    Cancellation, ExtensionRunner, Local, create_event_bus_for, create_extension_runtime,
    load_extension_from_factory,
};
use std::{rc::Rc, sync::RwLock, time::Duration};

/// Loads the actual authored component through the shipped artifact factory.
async fn loaded() -> ExtensionRunner<Local> {
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no errors")),
    );
    let runtime = create_extension_runtime::<Local>();
    let path = std::env::var("MAESTRO_NATIVE_IO_COMPONENT").unwrap();
    let owner = load_extension_from_factory(
        maestro_extensions_wasmtime::artifact_factory(
            path.clone(),
            super::support::widgets(),
            maestro_extensions_wasmtime::Installation {
                version: "native-io-host".into(),
                agent_dir: "/controlled-agent".into(),
            },
        ),
        "/".into(),
        bus.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session("/").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    super::support::bind_prompt(&runner, "native wait");
    runner
}

/// Invokes the same compiled author operation for each selected native signal.
async fn wait(runner: &ExtensionRunner<Local>, millis: u64) -> Result<(), String> {
    (runner.get_command("native-wait").unwrap().command.handler)(
        millis.to_string(),
        runner.create_command_context(),
    )
    .await
    .map_err(|e| e.to_string())
}

#[test]
fn native_wait_rejects_preabort_before_arming() {
    run(async {
        tokio::time::pause();
        let runner = loaded().await;
        let signal = Cancellation::new();
        signal.abort();
        super::abort_observers::bind(&runner, &signal);
        let before = tokio::time::Instant::now();
        assert_eq!(wait(&runner, 500).await, Err("Aborted".into()));
        assert_eq!(tokio::time::Instant::now(), before);
    });
}

#[test]
fn native_wait_uses_the_selected_duration() {
    run(async {
        tokio::time::pause();
        let runner = loaded().await;
        for signal in [None, Some(Cancellation::new())] {
            if let Some(signal) = signal {
                super::abort_observers::bind(&runner, &signal);
            }
            for millis in [0, 125] {
                let before = tokio::time::Instant::now();
                wait(&runner, millis).await.unwrap();
                let elapsed = tokio::time::Instant::now() - before;
                let selected = Duration::from_millis(millis);
                assert!(elapsed >= selected);
                assert!(elapsed <= selected + Duration::from_millis(1));
            }
        }
    });
}

/// Runs the same retained guest future across independent handler lifetimes.
async fn retained(runner: &ExtensionRunner<Local>, action: &str) -> Result<(), String> {
    (runner
        .get_command("native-retained-wait")
        .unwrap()
        .command
        .handler)(action.into(), runner.create_command_context())
    .await
    .map_err(|e| e.to_string())
}

#[test]
fn native_wait_releases_its_signal_observer() {
    run(async {
        let runner = loaded().await;
        let signal = Cancellation::new();
        super::abort_observers::bind(&runner, &signal);
        retained(&runner, "start").await.unwrap();
        super::abort_observers::bind(&runner, &Cancellation::new());
        signal.abort();
        assert_eq!(retained(&runner, "finish").await, Err("Aborted".into()));

        let signal = Cancellation::new();
        let released = super::abort_observers::signal_release(&signal);
        super::abort_observers::bind(&runner, &signal);
        retained(&runner, "start").await.unwrap();
        retained(&runner, "drop").await.unwrap();
        assert!(!signal.is_aborted());
        super::support::bind_prompt(&runner, "released wait");
        drop(signal);
        released.await.unwrap();
    });
}
