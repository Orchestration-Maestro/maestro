//! Typed forwarding to the models-owned event buffer on the existing local pump.
use crate::{
    guest_calls::Handle,
    local_resources::{Admission, Capability, Lease, Message, Registry, Work},
    stream_calls::{Operation, Selection},
};
use maestro_extensions::{EventStream, Local};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, mpsc::Sender},
};
use tokio::sync::oneshot;

/// Native stream with local callback captures and independently retained result errors.
type Buffer = EventStream<Handle, Result<Handle, String>, Local>;
/// One local buffer owner; callback failures retain their original message.
pub(crate) struct Owner {
    /// The only FIFO and final-result owner.
    buffer: Buffer,
    /// First callback failure, read outside callback-table and buffer locks.
    error: Rc<RefCell<Option<String>>>,
}
/// Catchable synchronous operation result.
type Reply = Sender<Result<(), String>>;
/// Typed requests to the original native buffer.
pub(crate) enum Request {
    /// Admit one retained local value.
    Push(Handle, Reply),
    /// End without conflating absent results and JSON null.
    End(Option<Handle>, Reply),
    /// Observe one competing-reader event.
    Next(oneshot::Sender<Result<Option<Handle>, String>>, Admission),
    /// Observe the final result independently.
    Result(oneshot::Sender<Result<Handle, String>>, Admission),
}
/// Store-safe ownership of one native stream.
#[derive(Clone)]
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);
impl Endpoint {
    /// Admit through the existing synchronous callback continuation.
    pub(crate) fn push(&self, value: Handle) -> crate::import_route::Route {
        self.0
            .route(|id, reply| Message::Stream(id, Request::Push(value, reply)))
    }
    /// End through the existing first-result owner.
    pub(crate) fn end(&self, value: Option<Handle>) -> crate::import_route::Route {
        self.0
            .route(|id, reply| Message::Stream(id, Request::End(value, reply)))
    }
    /// Observe one native event while retaining only this observation's admission.
    pub(crate) async fn next(&self) -> Result<Option<Handle>, String> {
        self.0
            .request_async(|id, reply, admission| {
                Message::Stream(id, Request::Next(reply, admission))
            })
            .await
    }
    /// Observe the native result without consuming queued events.
    pub(crate) async fn result(&self) -> Result<Handle, String> {
        self.0
            .request_async(|id, reply, admission| {
                Message::Stream(id, Request::Result(reply, admission))
            })
            .await
    }
}
impl crate::api_reads::Endpoint {
    /// Request a native buffer through the already admitted local owner.
    pub(crate) fn new_stream(
        &self,
        classifier: Handle,
        extractor: Handle,
    ) -> Result<Endpoint, String> {
        self.0.request(|_, reply| {
            Message::NewStream(Creation {
                classifier,
                extractor,
                reply,
            })
        })
    }
}
impl Registry {
    /// Retain the original native buffer, using the models execution family for local callbacks.
    pub(crate) fn stream(&self, classifier: Handle, extractor: Handle) -> Result<Endpoint, String> {
        let error = Rc::new(RefCell::new(None));
        let classify_error = error.clone();
        let extract_error = error.clone();
        let classify_registry = self.clone();
        let extract_registry = self.clone();
        let buffer = Buffer::new_local(
            move |value: &Handle| match classifier.stream_call(
                &classify_registry,
                value.clone(),
                Operation::Classify,
            ) {
                Ok(Selection::Complete(complete)) => complete,
                other => {
                    *classify_error.borrow_mut() = Some(selection_error(other));
                    true
                }
            },
            move |value| {
                if let Some(error) = extract_error.borrow().clone() {
                    return Err(error);
                }
                match extractor.stream_call(&extract_registry, value.clone(), Operation::Extract) {
                    Ok(Selection::Extracted(value)) => Ok(value),
                    other => {
                        let error = selection_error(other);
                        *extract_error.borrow_mut() = Some(error.clone());
                        Err(error)
                    }
                }
            },
        );
        self.insert(Capability::Stream(Owner { buffer, error }))
            .map(Endpoint)
    }
}
/// Preserve an original failure; reject a callback of the wrong kind explicitly.
fn selection_error(result: Result<Selection, String>) -> String {
    result
        .err()
        .unwrap_or_else(|| "extension stream callback result kind mismatch".into())
}
impl Request {
    /// Select the original buffer outside the capability-table borrow, then start one operation.
    pub(crate) fn answer(self, registry: &Registry, id: u64) -> Option<Work> {
        let capability = registry.capability(id);
        let Some(Capability::Stream(owner)) = capability.as_deref() else {
            self.reject();
            return None;
        };
        match self {
            Self::Push(value, reply) => {
                owner.buffer.push(value);
                let result = owner.error.borrow().clone().map_or(Ok(()), Err);
                let _ = reply.send(result);
            }
            Self::End(value, reply) => {
                owner.buffer.end(value.map(Ok));
                let _ = reply.send(Ok(()));
            }
            Self::Next(mut reply, admission) => {
                let next = owner.buffer.next();
                return Some(Box::pin(async move {
                    tokio::select! {
                        value = next => { let _ = reply.send(Ok(value)); }
                        () = reply.closed() => {}
                    }
                    drop(admission);
                }));
            }
            Self::Result(mut reply, admission) => {
                let result = owner.buffer.result();
                return Some(Box::pin(async move {
                    tokio::select! {
                        value = result => { let _ = reply.send(value); }
                        () = reply.closed() => {}
                    }
                    drop(admission);
                }));
            }
        }
        None
    }
    /// Settle requests whose original resource has ended.
    fn reject(self) {
        let error = "extension stream resource ended".to_owned();
        match self {
            Self::Push(_, reply) | Self::End(_, reply) => {
                let _ = reply.send(Err(error));
            }
            Self::Next(reply, _) => {
                let _ = reply.send(Err(error));
            }
            Self::Result(reply, _) => {
                let _ = reply.send(Err(error));
            }
        }
    }
}

impl super::pump::State {
    /// Start one original stream operation on the existing pending-work owner.
    pub(super) fn stream_request(&self, id: u64, request: Request) {
        if let Some(work) = request.answer(&self.registry, id) {
            self.stage(work);
        }
    }
}

/// Retained callbacks and the original stream-creation reply.
pub(crate) struct Creation {
    /// Synchronous classifier admission.
    classifier: Handle,
    /// Synchronous extractor admission.
    extractor: Handle,
    /// Caller awaiting the native buffer resource.
    reply: Sender<Result<Endpoint, String>>,
}
impl Creation {
    /// Retain the native buffer through the existing local registry.
    pub(super) fn answer(self, registry: &Registry) {
        let _ = self
            .reply
            .send(registry.stream(self.classifier, self.extractor));
    }
}
