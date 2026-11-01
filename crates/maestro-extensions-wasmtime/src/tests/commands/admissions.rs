//! Independently admitted work outlives a cancelled importer and its last resource.
use crate::{command_reads::Request, context_reads::Owner, local_resources::Pump, tests::runner};
use maestro_extensions::ExtensionCommandContextActions;
use std::{
    cell::RefCell,
    future::Future,
    rc::Rc,
    task::{Context, Waker},
};
use tokio::sync::oneshot;

#[test]
fn cancelled_import_does_not_revoke_its_admitted_operation() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let runner = runner("/admission");
            let retained = Rc::new(());
            let released = Rc::downgrade(&retained);
            let (resume, resumed) = oneshot::channel();
            let operands = RefCell::new(Some((retained, resumed)));
            runner.bind_command_context(Some(ExtensionCommandContextActions::new(
                maestro_extensions::CommandContextOperations {
                    wait_for_idle: Rc::new(move || {
                        let (retained, resumed) = operands.borrow_mut().take().unwrap();
                        Box::pin(async move {
                            resumed.await?;
                            drop(retained);
                            Ok(())
                        })
                    }),
                    ..crate::context_actions::silent_command_operations()
                },
            )));
            let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
            let endpoint = pump
                .registry()
                .context(Owner::Command(runner.create_command_context()))
                .unwrap();
            let mut waiting = Box::pin(endpoint.command(Request::Wait));
            let mut context = Context::from_waker(Waker::noop());
            assert!(waiting.as_mut().poll(&mut context).is_pending());
            drop(waiting);
            assert!(pump.answer_next().await);
            drop(endpoint);
            drop(admission);
            drop(runner);
            let mut draining = Box::pin(pump.serve());
            assert!(draining.as_mut().poll(&mut context).is_pending());
            assert!(released.upgrade().is_some());
            resume.send(()).unwrap();
            draining.await;
            assert!(released.upgrade().is_none());
        });
}
