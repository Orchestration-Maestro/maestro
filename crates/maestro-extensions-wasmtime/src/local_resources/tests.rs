//! Independent local capability ownership through the single request pump.
use crate::read_imports::ReadEndpoint;
mod builtin;
mod component;
mod keys;
mod lent;
mod local_bash;
mod lock;
mod oauth;
mod overlay;
mod pipes;
mod provider;
mod replaced;
mod response;
mod signal;
mod stream;
mod synchronous;
mod watch;
mod worker;
use super::{Capability, Lease, Pump, Registry};
use crate::{
    context_reads::{Owner, Request},
    tests::{ended_errors, runner},
};
use maestro_extensions::{
    ExtensionContextActions, ExtensionExecution, Local, types::ProviderActions,
};
use std::{
    rc::Rc,
    sync::{Arc, mpsc},
};
mod ui_host {
    pub use maestro_extensions::test_support::Named;
}

/// Lends `capability` to the endpoint `endpoint` builds, releases it, and returns the failures
/// `operations` settles with on the ended endpoint.
pub(super) fn ended_endpoint<E: Send + 'static, const N: usize>(
    capability: Capability,
    endpoint: fn(Arc<Lease>) -> E,
    operations: impl FnOnce(E) -> [String; N] + Send + 'static,
) -> [String; N] {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let lease = pump.registry().insert(capability).unwrap();
    let endpoint = endpoint(lease.clone());
    ended_errors(lease, pump, admission, move || operations(endpoint))
}

/// Callback handles that need no Store: their releases go to the returned receiver.
pub(super) fn handles<const N: usize>(
    registry: &Registry,
) -> (
    [crate::guest_calls::Handle; N],
    tokio::sync::mpsc::UnboundedReceiver<crate::guest_calls::Call>,
    tokio::sync::mpsc::UnboundedSender<crate::guest_calls::Call>,
) {
    let (calls, releases) = tokio::sync::mpsc::unbounded_channel();
    let callbacks = crate::guest_calls::Callbacks::new(
        calls.downgrade(),
        crate::lifecycle::replies::Replies::default(),
    );
    let handles = std::array::from_fn(|id| {
        let observer = callbacks.observer(u32::try_from(id).unwrap());
        observer.handle(registry.admission().unwrap()).unwrap()
    });
    (handles, releases, calls)
}

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
