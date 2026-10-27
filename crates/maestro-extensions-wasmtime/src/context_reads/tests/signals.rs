//! Retained signals keep their selected cancellation across rebinding and retirement.
use super::*;
use crate::read_imports::ReadEndpoint;
use maestro_extensions::{ExtensionContextActions, ExtensionExecution, types::ProviderActions};
use maestro_models::Cancellation;

#[test]
fn retained_signal_survives_rebind_and_retirement() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        for command in [false, true] {
            retained_signal(command).await;
        }
    });
}

/// Selects unequal signals before retiring the guarded context.
async fn retained_signal(command: bool) {
    let runner = runner("/signal");
    let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
    let context = if command {
        Owner::Command(runner.create_command_context())
    } else {
        Owner::Base(runner.create_context())
    };
    let endpoint = pump.registry().context(context).unwrap();
    let original = Cancellation::new();
    let captured = original.clone();
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_signal: Rc::new(move || Ok(Some(captured.clone()))),
                ..ExtensionContextActions::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
    let worker = std::thread::spawn(move || {
        let signal = endpoint.read(Request::Signal).unwrap().unwrap();
        assert!(!signal.aborted().unwrap());
        assert!(!signal.aborted().unwrap());
        assert!(signal.aborted().unwrap());
        assert_eq!(
            endpoint.read(Request::Cwd).unwrap_err(),
            "retired signal context"
        );
        drop(endpoint);
        assert!(signal.aborted().unwrap());
    });
    assert!(pump.answer_next().await);
    assert!(pump.answer_next().await);
    let replacement = Cancellation::new();
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_signal: Rc::new(move || Ok(Some(replacement.clone()))),
                ..ExtensionContextActions::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
    assert!(pump.answer_next().await);
    original.abort();
    runner.invalidate(Some("retired signal context".into()));
    drop(runner);
    drop(admission);
    pump.serve().await;
    worker.join().unwrap();
}
