//! Final domain consumers own Store admissions through healthy and faulted teardown.
#![cfg(test)]
use maestro_extensions::{ExtensionRunner, Local, create_event_bus_for};
use std::{cell::RefCell, rc::Rc};

/// Keeps a second component usable while the first releases aliases, UI and admitted tails.
pub(super) async fn release() {
    super::retained_contexts::retained(false).await;
    for fault in [false, true] {
        release_instance(fault).await;
    }
}

/// Disposes registrations explicitly; neither healthy nor faulted teardown sends a stop request.
async fn release_instance(fault: bool) {
    let tails = Rc::new(RefCell::new(Vec::new()));
    let queued = tails.clone();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let (reporter, mut teardown) = observed_reporter(&errors);
    let bus = create_event_bus_for::<Local>(
        Rc::new(move |tail| queued.borrow_mut().push(tail)),
        reporter,
    );
    let (owner, runner, events) = load(bus.for_extension(), "/lifetime-first").await;
    let other_errors = Rc::new(RefCell::new(Vec::new()));
    let (reporter, other_teardown) = observed_reporter(&other_errors);
    let other_bus = create_event_bus_for::<Local>(Rc::new(|_| panic!("no tails")), reporter);
    let (other_owner, other, other_events) =
        load(other_bus.for_extension(), "/lifetime-second").await;
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let copy = command.clone();
    let context = runner.create_command_context();
    let component = super::retained_components::view(&runner, &events, "listener-tail")
        .unwrap()
        .unwrap();
    let ui_alias = component.clone();
    assert_eq!(tails.borrow().len(), 2);
    let expected = expected(&runner, &component, fault).await;
    drop(command);
    drop((owner, runner, events));
    bus.clear();
    drop(bus);
    assert_eq!(
        tails.borrow().len(),
        2,
        "unsubscribe cancelled admitted work"
    );
    assert_eq!(
        copy("copied".into(), context.clone())
            .await
            .unwrap_err()
            .to_string(),
        expected
    );
    pending(&mut teardown);
    drop((copy, context, component));
    pending(&mut teardown);
    drop(ui_alias);
    pending(&mut teardown);
    settle_tails(&tails).await;
    teardown.await.unwrap();
    diagnostics(&errors.borrow(), fault, &expected);
    assert_second(&other).await;
    other_bus.clear();
    drop(other_bus);
    drop((other_owner, other, other_events));
    other_teardown.await.unwrap();
    assert!(other_errors.borrow().is_empty());
}

/// Fault and final release in another Store cannot exchange this instance's context identity.
async fn assert_second(runner: &ExtensionRunner<Local>) {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    assert_eq!(
        command("independent".into(), runner.create_command_context())
            .await
            .unwrap_err()
            .to_string(),
        "command:independent|/lifetime-second|renderer barrier"
    );
}

/// Creates a real loader instance with an independently observable final admission.
async fn load(
    events: maestro_extensions::EventBus<Local>,
    cwd: &str,
) -> (
    maestro_extensions::types::ExtensionHandle<Local>,
    ExtensionRunner<Local>,
    maestro_extensions::EventBus<Local>,
) {
    let (owner, runner, events) =
        super::retained_components::loaded_in(events, super::session::runner_session(cwd).0).await;
    (owner, runner, events)
}

/// Distinguishes an admitted healthy view from a retained but faulted one.
async fn expected(
    runner: &ExtensionRunner<Local>,
    component: &maestro_extensions::ComponentHandle,
    fault: bool,
) -> String {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let context = runner.create_command_context();
    if fault {
        let failure = command("trap".into(), context)
            .await
            .unwrap_err()
            .to_string();
        assert!(
            failure.contains("component fault") && failure.contains("wasm trap"),
            "{failure}"
        );
        assert_eq!(component.try_render(47).unwrap_err().to_string(), failure);
        assert!(
            runner
                .get_message_renderer::<serde_json::Value>("bridge-view")
                .is_some()
        );
        failure
    } else {
        assert_eq!(component.try_render(47).unwrap(), ["listener-tail:47"]);
        "command:copied|/lifetime-first|renderer barrier".into()
    }
}

/// A surviving final consumer must still own the pump admission.
fn pending(teardown: &mut tokio::sync::oneshot::Receiver<()>) {
    assert!(matches!(
        teardown.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
}

/// Retained faulted tails report once each; healthy tails complete without diagnostics.
fn diagnostics(errors: &[String], fault: bool, expected: &str) {
    if fault {
        assert_eq!(
            errors,
            [
                format!("Event handler error (renderer-created): {expected}\n"),
                format!("Event handler error (renderer-created): {expected}\n"),
            ]
        );
    } else {
        assert!(errors.is_empty());
    }
}

/// Signals the release of the closure that captures it, once its last owner drops it.
pub(super) struct Released(pub(super) Option<tokio::sync::oneshot::Sender<()>>);
impl Drop for Released {
    fn drop(&mut self) {
        let _ = self.0.take().unwrap().send(());
    }
}

/// Retains the teardown signal in the original error reporter, independently of its controller.
pub(super) fn observed_reporter(
    errors: &Rc<RefCell<Vec<String>>>,
) -> (
    maestro_extensions::ErrorReporter<Local>,
    tokio::sync::oneshot::Receiver<()>,
) {
    let (released, teardown) = tokio::sync::oneshot::channel();
    let released = Released(Some(released));
    let reports = errors.clone();
    (
        Rc::new(move |error| {
            let _retained = &released;
            reports.borrow_mut().push(error.to_owned());
        }),
        teardown,
    )
}

/// Completes only the admitted tails retained before unsubscription.
async fn settle_tails(tails: &super::canonical_api::Tails) {
    let pending = std::mem::take(&mut *tails.borrow_mut());
    for tail in pending {
        tail.await;
    }
}
