//! Nested admissions first-poll outside the pending set's active native callback.
use crate::read_imports::ReadEndpoint;
use crate::{
    command_reads::Request,
    context_reads::{Owner, Request as Read},
    local_resources::{Completion, Pump, Registry},
    tests::runner,
};
use maestro_extensions::ExtensionCommandContextActions;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc,
};
use tokio::sync::oneshot;

#[test]
fn nested_admission_starts_before_native_callback_returns_and_wakes_after_staging() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let runner = runner("/nested-admission");
            let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
            let (start, started) = oneshot::channel();
            let (release, released) = oneshot::channel();
            let (actions, incoming, finished) =
                operations(pump.registry(), started, release, released);
            runner.bind_command_context(Some(actions));
            let endpoint = pump
                .registry()
                .context(Owner::Command(runner.create_command_context()))
                .unwrap();
            let nested_context = endpoint.clone();
            let nested = std::thread::spawn(move || {
                let completion = incoming.recv().unwrap();
                let mut work = Box::pin(nested_context.command(Request::Wait));
                let mut context = std::task::Context::from_waker(std::task::Waker::noop());
                assert!(std::future::Future::poll(work.as_mut(), &mut context).is_pending());
                assert_eq!(nested_context.read(Read::Cwd).unwrap(), "/nested-admission");
                completion.complete(Ok(()));
                tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap()
                    .block_on(work)
                    .unwrap();
            });
            let caller = std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap()
                    .block_on(endpoint.command(Request::Wait))
                    .unwrap();
            });
            assert!(pump.answer_next().await);
            start.send(()).unwrap();
            drop(admission);
            pump.serve().await;
            nested.join().unwrap();
            caller.join().unwrap();
            assert!(finished.get());
        });
}

/// Native operations distinguish the outer pending poll from its nested admission.
fn operations(
    registry: Registry,
    started: oneshot::Receiver<()>,
    release: oneshot::Sender<()>,
    released: oneshot::Receiver<()>,
) -> (
    ExtensionCommandContextActions,
    mpsc::Receiver<Completion>,
    Rc<Cell<bool>>,
) {
    let started = Rc::new(RefCell::new(Some(started)));
    let release = Rc::new(RefCell::new(Some(release)));
    let released = Rc::new(RefCell::new(Some(released)));
    let (calls, incoming) = mpsc::channel::<Completion>();
    let selected = Rc::new(Cell::new(0));
    let child_started = Rc::new(Cell::new(false));
    let child_finished = Rc::new(Cell::new(false));
    let finished = child_finished.clone();
    let actions =
        ExtensionCommandContextActions::new(maestro_extensions::CommandContextOperations {
            wait_for_idle: Rc::new(move || {
                let index = selected.get();
                selected.set(index + 1);
                if index != 0 {
                    assert_eq!(index, 1);
                    let released = released.borrow_mut().take().unwrap();
                    let child_started = child_started.clone();
                    let child_finished = child_finished.clone();
                    return Box::pin(async move {
                        child_started.set(true);
                        released.await?;
                        child_finished.set(true);
                        Ok(())
                    });
                }
                let started = started.borrow_mut().take().unwrap();
                let release = release.borrow_mut().take().unwrap();
                let calls = calls.clone();
                let registry = registry.clone();
                let child_started = child_started.clone();
                Box::pin(async move {
                    started.await?;
                    registry.invoke_sync(None, |completion, _| {
                        calls.send(completion).map_err(|error| error.to_string())
                    })?;
                    assert!(
                        child_started.get(),
                        "nested operation did not reach its first await"
                    );
                    release.send(()).unwrap();
                    Ok(())
                })
            }),
            ..crate::context_actions::silent_command_operations()
        });
    (actions, incoming, finished)
}
