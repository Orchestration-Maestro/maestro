//! Independent local capability ownership through the single request pump.
use crate::read_imports::ReadEndpoint;
mod model_decode;
mod synchronous;
mod worker;
use super::Pump;
use crate::{
    context_reads::{Owner, Request},
    tests::runner,
};
use maestro_extensions::{
    ExtensionContextActions, ExtensionExecution, Local, types::ProviderActions,
};
use std::{rc::Rc, sync::mpsc};
#[path = "../../../maestro-extensions/tests/support/ui_host.rs"]
mod ui_host;

#[test]
fn retained_ui_outlives_context_without_retaining_it() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        for command in [false, true] {
            independent_ui(command).await;
        }
    });
}

#[test]
fn retired_pump_rejects_resource_registration_with_live_admission() {
    let runner = runner("/closed-pump");
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    drop(pump);
    assert!(
        registry
            .context(Owner::Base(runner.create_context()))
            .is_err(),
        "registration admitted a resource after its pump ended"
    );
    drop(admission);
}

/// A caller-owned UI whose destructor attempts one bounded re-entrant registration.
struct RegisterOnDrop {
    /// Weak local insertion service, not an admission.
    registry: super::Registry,
    /// Records the observed registration result without panicking in a destructor.
    refused: Rc<std::cell::Cell<Option<bool>>>,
}

impl maestro_extensions::types::ExtensionUIContextPort for RegisterOnDrop {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
}

impl Drop for RegisterOnDrop {
    fn drop(&mut self) {
        let (ui, _) = ui_host::Named::install("during-drop");
        self.refused.set(Some(self.registry.ui(ui).is_err()));
    }
}

#[test]
fn pump_revokes_admission_before_releasing_caller_capabilities() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    let refused = Rc::new(std::cell::Cell::new(None));
    let ui = maestro_extensions::ExtensionUIContext::new(Rc::new(RegisterOnDrop {
        registry: registry.clone(),
        refused: Rc::clone(&refused),
    }));
    let lease = registry.ui(ui).unwrap();
    drop(pump);
    assert_eq!(refused.get(), Some(true));
    drop(lease);
    drop(admission);
}

/// A captured domain operation distinguishes releasing a context from releasing its UI.
fn captured_runner() -> (
    maestro_extensions::ExtensionRunner<Local>,
    std::rc::Weak<std::cell::Cell<bool>>,
) {
    let runner = runner("/original");
    let context_capture = Rc::new(std::cell::Cell::new(true));
    let context_weak = Rc::downgrade(&context_capture);
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                is_idle: Rc::new(move || Ok(context_capture.get())),
                ..ExtensionContextActions::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
    (runner, context_weak)
}

/// Checks both kinds of guarded context against a separately retained raw UI.
async fn independent_ui(command: bool) {
    let (runner, context_weak) = captured_runner();
    let (ui, host) = ui_host::Named::install("original");
    let ui_weak = Rc::downgrade(&host);
    runner.set_ui_context(Some(ui));
    let owner = if command {
        Owner::Command(runner.create_command_context())
    } else {
        Owner::Base(runner.create_context())
    };
    let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
    let endpoint = pump.registry().context(owner).unwrap();
    drop(admission);
    let (release, released) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let copy = endpoint.clone();
        drop(endpoint);
        let ui = copy.read(Request::Ui).unwrap();
        drop(copy);
        released.recv().unwrap();
        drop(ui);
    });
    assert!(pump.answer_next().await);
    runner.set_ui_context(None);
    runner.invalidate(Some("retired".into()));
    drop(runner);
    drop(host);
    assert!(pump.answer_next().await);
    assert!(
        context_weak.upgrade().is_none(),
        "context was retained by its raw UI"
    );
    assert!(ui_weak.upgrade().is_some(), "raw UI lost its original host");
    let mut final_release = Box::pin(pump.answer_next());
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(std::future::Future::poll(final_release.as_mut(), &mut context).is_pending());
    release.send(()).unwrap();
    worker.join().unwrap();
    assert!(final_release.await);
    pump.serve().await;
    assert!(
        ui_weak.upgrade().is_none(),
        "final raw UI release retained its host"
    );
}
