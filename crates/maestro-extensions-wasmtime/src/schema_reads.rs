//! Scoped requests to the operand held on the validator's original stack.
use crate::{local_resources::Message, schema_wire::JsonKind};
use std::sync::mpsc::Sender;
/// The original requester receives one fallible operation result.
type Reply<T> = Sender<Result<T, String>>;
/// One typed operation on the currently borrowed JSON operand.
pub(crate) enum Request {
    /// Invoke a retained node against the operand on its original native stack.
    Invoke(crate::local_resources::schema::Invocation),
    /// Inspect one selected JSON kind.
    Kind(String, Reply<Option<JsonKind>>),
    /// Read one selected JSON value.
    Read(String, Reply<Option<String>>),
    /// Enumerate one selected object.
    Keys(String, Reply<Vec<String>>),
    /// Write one object member or array position.
    Set(String, String, String, Reply<()>),
    /// Delete one selected member.
    Delete(String, String, Reply<()>),
    /// Edit an array range with explicit removed results.
    Splice(String, (u32, u32), Vec<String>, Reply<Vec<String>>),
}
impl Request {
    /// Apply one operation to the operand still borrowed by its caller.
    pub(crate) fn answer(
        self,
        value: &mut serde_json::Value,
        registry: &crate::local_resources::Registry,
    ) {
        #[cfg(test)]
        self.measure();
        let mut node = crate::schema_node::JsonNode(value);
        match self {
            Self::Invoke(call) => call.answer(registry, node.0),
            Self::Kind(path, reply) => {
                let _ = reply.send(Ok(node.kind(&path)));
            }
            Self::Read(path, reply) => {
                let _ = reply.send(node.read(&path));
            }
            Self::Keys(path, reply) => {
                let _ = reply.send(node.keys(&path));
            }
            Self::Set(path, key, value, reply) => {
                let _ = reply.send(node.set(&path, &key, &value));
            }
            Self::Delete(path, key, reply) => {
                let _ = reply.send(node.delete(&path, &key));
            }
            Self::Splice(path, range, values, reply) => {
                let _ = reply.send(node.splice(&path, range, &values));
            }
        }
    }
    /// Settle a request after its borrowed operand has ended.
    pub(crate) fn reject(self) {
        let error = "extension JSON operand ended".to_owned();
        match self {
            Self::Invoke(call) => call.reject(),
            Self::Kind(_, r) => {
                let _ = r.send(Err(error));
            }
            Self::Read(_, r) => {
                let _ = r.send(Err(error));
            }
            Self::Keys(_, r) | Self::Splice(_, _, _, r) => {
                let _ = r.send(Err(error));
            }
            Self::Set(_, _, _, r) | Self::Delete(_, _, r) => {
                let _ = r.send(Err(error));
            }
        }
    }
}
/// The invocation identity and admission needed to reach its borrowed operand.
#[derive(Clone)]
pub(crate) struct Endpoint {
    /// The stable invocation identity.
    pub(crate) id: u64,
    /// The existing local request sender.
    pub(crate) requests: tokio::sync::mpsc::UnboundedSender<Message>,
}
impl Endpoint {
    /// Use the existing local receiver for one typed JSON operation.
    pub(crate) fn read<T>(&self, make: impl FnOnce(Reply<T>) -> Request) -> Result<T, String> {
        let (send, receive) = std::sync::mpsc::channel();
        self.requests
            .send(Message::Json(self.id, make(send)))
            .map_err(|_| "extension JSON operand ended")?;
        receive
            .recv()
            .map_err(|_| "extension JSON operand reply ended")?
    }
}

#[cfg(test)]
thread_local! {pub(crate) static BYTES:std::cell::Cell<(usize,usize)>=const{std::cell::Cell::new((0,0))};}
#[cfg(test)]
impl Request {
    /// Account exact selected request/result fields, excluding canonical ABI framing.
    fn measure(&self) {
        let (request, response) = match self {
            Self::Read(path, _) => (12 + path.len(), 11),
            Self::Set(path, key, value, _) => (28 + path.len() + key.len() + value.len(), 1),
            _ => return,
        };
        BYTES.set((BYTES.get().0 + request, BYTES.get().1 + response));
    }
}
