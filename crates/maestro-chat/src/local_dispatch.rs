//! Work a sendable producer hands to the local owners it updates.

use maestro_app::DiagnosticErrorInfo;
use maestro_tui::tui::TuiRuntime;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{mpsc, oneshot};

/// One delivery: work the producer admitted, or the caller's explicit end of the delivery.
enum Delivery<T> {
    /// Work to apply to the local owners.
    Work(T),
    /// The caller closed the delivery; every work item before it is already applied.
    Finish,
}

/// The local delivery of one producer's work.
///
/// A producer that must be sendable sends typed items through [`LocalDispatch::sender`]; one
/// receiver runs on the terminal runtime and applies each item to the local owners. Nothing
/// in the type names what the items are, so every caller shares this one delivery.
///
/// [`LocalDispatch::settle`] closes the delivery and waits until every admitted item was
/// applied, so a producer that keeps its sender alive cannot hold the drain open. An update
/// that fails ends the delivery and the failure is returned by `settle`.
pub(crate) struct LocalDispatch<T> {
    /// The owned producer side; dropping it ends the delivery too.
    sender: mpsc::UnboundedSender<Delivery<T>>,
    /// Whether the delivery was closed; `sender` refuses work afterwards.
    closed: Arc<AtomicBool>,
    /// The failure the local update reported, kept for the caller.
    failure: Rc<RefCell<Option<DiagnosticErrorInfo>>>,
    /// Reports that the delivery ended, so the caller can wait for every admitted item.
    delivered: oneshot::Receiver<()>,
}

/// Starts the delivery of `apply` for every item the returned dispatch receives.
pub(crate) fn local_dispatch<T: 'static>(
    runtime: &Rc<dyn TuiRuntime>,
    mut apply: impl FnMut(T) -> Result<(), DiagnosticErrorInfo> + 'static,
) -> LocalDispatch<T> {
    let (sender, mut receiver) = mpsc::unbounded_channel::<Delivery<T>>();
    let (drained, delivered) = oneshot::channel::<()>();
    let failure: Rc<RefCell<Option<DiagnosticErrorInfo>>> = Rc::default();
    let reported = Rc::clone(&failure);
    runtime.spawn_local(Box::pin(async move {
        while let Some(delivery) = receiver.recv().await {
            let admitted = match delivery {
                Delivery::Work(item) => deliver(item, &mut apply, &reported),
                Delivery::Finish => false,
            };
            if !admitted {
                break;
            }
        }
        drop(drained);
        Ok(())
    }));
    LocalDispatch {
        sender,
        closed: Arc::default(),
        failure,
        delivered,
    }
}

/// Applies one admitted item, keeping a failure for the caller and ending the delivery.
fn deliver<T>(
    item: T,
    apply: &mut impl FnMut(T) -> Result<(), DiagnosticErrorInfo>,
    failure: &RefCell<Option<DiagnosticErrorInfo>>,
) -> bool {
    match apply(item) {
        Ok(()) => true,
        Err(error) => {
            failure.borrow_mut().replace(error);
            false
        }
    }
}

impl<T: Send + 'static> LocalDispatch<T> {
    /// A sendable producer of further items; once the delivery is closed it admits none.
    pub(crate) fn sender(&self) -> Box<dyn FnMut(T) + Send> {
        let sender = self.sender.clone();
        let closed = Arc::clone(&self.closed);
        // A closed receiver means the local owners are gone; the item has nowhere to go.
        Box::new(move |item| {
            if !closed.load(Ordering::SeqCst) {
                drop(sender.send(Delivery::Work(item)));
            }
        })
    }

    /// Stops admitting work: everything already admitted is still applied, and the
    /// receiver ends once it reaches this point.
    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        drop(self.sender.send(Delivery::Finish));
    }

    /// Closes the delivery, waits until the receiver ended, and returns the failure of the
    /// local update it met, if any.
    pub(crate) async fn settle(self) -> Result<(), DiagnosticErrorInfo> {
        self.close();
        let Self {
            failure, delivered, ..
        } = self;
        drop(delivered.await);
        match failure.borrow_mut().take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::local_dispatch;
    use crate::components::session_selector::list::tests::session_picker::manual_runtime::ManualRuntime;
    use maestro_app::DiagnosticErrorInfo;
    use std::{
        cell::RefCell,
        future::Future,
        pin::Pin,
        rc::Rc,
        task::{Context, Poll, Waker},
    };

    /// Runs `body` inside a current-thread runtime and task set, where the delivery's
    /// receiver task makes progress.
    fn on_tasks<T>(body: impl Future<Output = T>) -> T {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        tokio::task::LocalSet::new().block_on(&runtime, body)
    }

    /// Runs the delivery's receiver task beside `body`.
    async fn driving<T>(runtime: &ManualRuntime, body: impl Future<Output = T>) -> T {
        let tasks: Vec<_> = runtime
            .take_futures()
            .into_iter()
            .map(|future| tokio::task::spawn_local(async move { drop(future.await) }))
            .collect();
        let value = body.await;
        for task in tasks {
            task.await.unwrap();
        }
        value
    }

    /// Polls `future` until it finishes, yielding so the delivery's receiver runs.
    async fn settled<T>(future: &mut (impl Future<Output = T> + Unpin)) -> T {
        for _ in 0..64 {
            if let Some(value) = poll_once(future) {
                return value;
            }
            tokio::task::yield_now().await;
        }
        panic!("the delivery settles");
    }

    /// Polls `future` once with a waker that observes nothing.
    fn poll_once<F: Future + Unpin>(future: &mut F) -> Option<F::Output> {
        match Pin::new(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(output) => Some(output),
            Poll::Pending => None,
        }
    }

    #[test]
    fn retained_callback_drain_settles_after_close() {
        let runtime = ManualRuntime::new();
        let applied: Rc<RefCell<Vec<u32>>> = Rc::default();
        let seen = Rc::clone(&applied);
        let dispatch = local_dispatch(&runtime.handle(), move |item: u32| {
            seen.borrow_mut().push(item);
            Ok(())
        });
        let mut retained = dispatch.sender();
        retained(1);
        retained(2);
        let mut settle = Box::pin(dispatch.settle());
        on_tasks(driving(&runtime, async {
            assert!(settled(&mut settle).await.is_ok(), "the drain settles");
            // A producer that keeps its callback alive cannot extend the delivery.
            retained(3);
        }));
        assert_eq!(applied.borrow().clone(), [1, 2]);
    }

    #[test]
    fn presentation_failure_surfaces_at_settle() {
        let runtime = ManualRuntime::new();
        let dispatch = local_dispatch(&runtime.handle(), |item: u32| {
            if item == 2 {
                return Err(DiagnosticErrorInfo::error("the dialog could not be shown"));
            }
            Ok(())
        });
        let mut send = dispatch.sender();
        send(1);
        send(2);
        send(3);
        let mut settle = Box::pin(dispatch.settle());
        let error = on_tasks(driving(&runtime, async { settled(&mut settle).await }))
            .expect_err("the update failure is returned");
        assert_eq!(error.message, "the dialog could not be shown");
    }

    #[test]
    fn clean_delivery_settles_without_a_failure() {
        let runtime = ManualRuntime::new();
        let applied: Rc<RefCell<Vec<u32>>> = Rc::default();
        let seen = Rc::clone(&applied);
        let dispatch = local_dispatch(&runtime.handle(), move |item: u32| {
            seen.borrow_mut().push(item);
            Ok(())
        });
        let mut send = dispatch.sender();
        send(7);
        let mut settle = Box::pin(dispatch.settle());
        let outcome = on_tasks(driving(&runtime, async { settled(&mut settle).await }));
        assert!(outcome.is_ok());
        assert_eq!(applied.borrow().clone(), [7]);
    }
}
