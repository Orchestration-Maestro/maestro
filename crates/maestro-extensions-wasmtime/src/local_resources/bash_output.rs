//! Per-execution owned byte streams keep borrowed native sinks out of the registry.
use super::{Capability, Lease, Message, Registry, pump::State};
use maestro_extensions::types::ExtensionRuntimeState;
use std::{cell::RefCell, future::Future, rc::Rc, sync::Arc, task::Poll};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

/// Catchable completion of forwarding one canonical output chunk.
pub(super) type Reply = std::sync::mpsc::Sender<Result<(), String>>;

/// The guest resource retains only its original frame identity and admission.
#[derive(Clone)]
pub(crate) struct Endpoint(Arc<Lease>);

impl Endpoint {
    /// Forwards owned raw bytes on the sole request pump without storing a sink borrow.
    pub(crate) fn on_data(&self, bytes: Vec<u8>) -> Result<(), String> {
        self.0
            .request(|id, reply| Message::BashOutput(id, bytes, reply))
    }
}

/// Registry-owned frame data; no callback or borrowed native operand is retained.
pub(crate) struct Frame {
    /// Native validity owner supplies the original catchable retirement error.
    state: ExtensionRuntimeState,
    /// Taking the sender closes output after all previously forwarded chunks.
    sender: RefCell<Option<UnboundedSender<Vec<u8>>>>,
}

impl Frame {
    /// Retires the key before closing the stream, including on cancelled execution.
    fn retire(&self) {
        self.state.invalidate(None);
        self.sender.borrow_mut().take();
    }

    /// Rejects retired keys before forwarding a chunk into its own execution stream.
    fn forward(&self, bytes: Vec<u8>) -> Result<(), String> {
        self.state
            .assert_active()
            .map_err(|error| error.to_string())?;
        self.sender
            .borrow()
            .as_ref()
            .ok_or_else(|| "extension output stream ended".to_owned())?
            .send(bytes)
            .map_err(|_| "extension output stream ended".to_owned())
    }
}

/// One execution's stream; the native sink is borrowed only by its active future.
pub(crate) struct Output {
    /// The native execution independently retains the same frame's admission.
    _lease: Arc<Lease>,
    /// Shared validity, not a retained borrowed sink.
    frame: Rc<Frame>,
    /// Per-frame data stream, never a second request receiver.
    bytes: UnboundedReceiver<Vec<u8>>,
}

impl Output {
    /// Drains every forwarded byte before returning the actual guest operation result.
    pub(crate) async fn finish(
        &mut self,
        operation: impl Future<Output = Result<Option<f64>, String>>,
        sink: &mut dyn FnMut(&[u8]),
    ) -> Result<Option<f64>, String> {
        let mut operation = std::pin::pin!(operation);
        let mut result = None;
        std::future::poll_fn(|context| {
            if result.is_none()
                && let Poll::Ready(value) = operation.as_mut().poll(context)
            {
                self.frame.retire();
                result = Some(value);
            }
            loop {
                match self.bytes.poll_recv(context) {
                    Poll::Ready(Some(bytes)) => sink(&bytes),
                    Poll::Ready(None) => return result.take().map_or(Poll::Pending, Poll::Ready),
                    Poll::Pending => return Poll::Pending,
                }
            }
        })
        .await
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        self.frame.retire();
    }
}

impl Registry {
    /// Allocates an independent frame for each execution, including overlapping ones.
    pub(crate) fn bash_output(&self) -> Result<(Endpoint, Output), String> {
        let (sender, bytes) = unbounded_channel();
        let frame = Rc::new(Frame {
            state: ExtensionRuntimeState::new(),
            sender: RefCell::new(Some(sender)),
        });
        let lease = self.insert(Capability::BashOutput(frame.clone()))?;
        Ok((
            Endpoint(lease.clone()),
            Output {
                _lease: lease,
                frame,
                bytes,
            },
        ))
    }
}

impl State {
    /// Forwards through the original frame outside capability-table borrows.
    pub(super) fn bash_output(&self, id: u64, bytes: Vec<u8>, reply: &Reply) {
        let capability = self.registry.capability(id);
        let result = match capability.as_deref() {
            Some(Capability::BashOutput(frame)) => frame.forward(bytes),
            _ => Err("extension bash-output resource ended".to_owned()),
        };
        let _ = reply.send(result);
    }
}
