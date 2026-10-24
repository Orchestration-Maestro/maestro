//! Work a sendable producer hands to the local owners it updates.

use maestro_app::DiagnosticErrorInfo;
use maestro_tui::tui::TuiRuntime;
use std::rc::Rc;
use tokio::sync::{mpsc, oneshot};

/// The local delivery of one producer's work.
///
/// A producer that must be sendable sends typed items through [`LocalDispatch::sender`]; one
/// receiver runs on the terminal runtime and applies each item to the local owners. Nothing
/// in the type names what the items are, so every caller shares this one delivery.
pub(crate) struct LocalDispatch<T> {
    /// The sendable producer side; dropping it ends the delivery.
    sender: mpsc::UnboundedSender<T>,
    /// Reports that the delivery ended, so the caller can wait for every admitted item.
    delivered: oneshot::Receiver<()>,
}

/// Starts the delivery of `apply` for every item the returned dispatch receives.
pub(crate) fn local_dispatch<T: 'static>(
    runtime: &Rc<dyn TuiRuntime>,
    mut apply: impl FnMut(T) -> Result<(), DiagnosticErrorInfo> + 'static,
) -> LocalDispatch<T> {
    let (sender, mut receiver) = mpsc::unbounded_channel::<T>();
    let (drained, delivered) = oneshot::channel::<()>();
    runtime.spawn_local(Box::pin(async move {
        while let Some(item) = receiver.recv().await {
            if apply(item).is_err() {
                break;
            }
        }
        drop(drained);
        Ok(())
    }));
    LocalDispatch { sender, delivered }
}

impl<T: Send + 'static> LocalDispatch<T> {
    /// A sendable producer of further items.
    pub(crate) fn sender(&self) -> Box<dyn FnMut(T) + Send> {
        let sender = self.sender.clone();
        // A closed receiver means the local owners are gone; the item has nowhere to go.
        Box::new(move |item| drop(sender.send(item)))
    }

    /// Waits until the producer dropped its sender and every admitted item was applied.
    pub(crate) async fn settle(self) {
        drop(self.sender);
        drop(self.delivered.await);
    }
}
