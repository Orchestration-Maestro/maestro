//! Requests: their identifiers, their correlation with answers and their deadline.
use super::{Inner, RpcClient, RpcClientError, expiry_at};
use crate::rpc_types::{RpcData, RpcResponse, RpcSessionCommand};
use maestro_json::jsonl::serialize_json_line;
use maestro_watch::fs_watch::NativeTimer;
use serde::Deserialize;
use serde_json::Value;
use std::{rc::Rc, time::Duration};
use tokio::{sync::oneshot, time::Instant};

/// How long a request waits for its answer, from the moment it is sent.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Settles one request with its answer, or with the failure to decode it.
type Responder = oneshot::Sender<Result<RpcResponse, RpcClientError>>;

/// A request awaiting its answer, until its deadline. Whatever happens first settles it:
/// its answer, its write's failure, the end of its child or its deadline; each is decided
/// when it happens, so the request's outcome never depends on when its caller looks.
pub(super) struct Pending {
    /// Settles the request.
    respond: Responder,
    /// From when only the timer settles the request.
    deadline: Instant,
    /// Settles the request with its timeout at the deadline.
    _timer: NativeTimer,
}

/// Removes a request's correlation when its call ends, however it ends.
struct PendingRequest<'a> {
    /// The client state holding the correlation.
    inner: &'a Inner,
    /// The request's identifier.
    id: String,
}

impl Drop for PendingRequest<'_> {
    fn drop(&mut self) {
        let removed = self.inner.state.borrow_mut().pending.remove(&self.id);
        drop(removed);
    }
}

impl RpcClient {
    /// Sends `command` with the next identifier, waits for the answer correlated with it,
    /// and returns the data of a successful one.
    ///
    /// # Errors
    /// Returns [`RpcClientError::NotStarted`] without a child, the session's failure, a
    /// write, decoding or timeout failure, or [`RpcClientError::Stopped`] when the child is
    /// stopped or ends first.
    pub(super) async fn send(&self, command: RpcSessionCommand) -> Result<RpcData, RpcClientError> {
        let mut record = serde_json::to_value(&command)?;
        let kind = record["type"].as_str().unwrap_or_default().to_owned();
        let (input, id, answer) = {
            let mut state = self.inner.state.borrow_mut();
            let input = match &state.running {
                Some(running) => running.input.clone(),
                None => return Err(RpcClientError::NotStarted),
            };
            state.request_id += 1;
            let id = format!("req_{}", state.request_id);
            let (respond, answer) = oneshot::channel();
            let deadline = Instant::now() + REQUEST_TIMEOUT;
            let expiring = id.clone();
            let timer = expiry_at(deadline, Rc::downgrade(&self.inner), move |inner| {
                inner.expire(&expiring, kind);
            });
            let pending = Pending {
                respond,
                deadline,
                _timer: timer,
            };
            state.pending.insert(id.clone(), pending);
            (input, id, answer)
        };
        let _pending = PendingRequest {
            inner: &self.inner,
            id: id.clone(),
        };
        if let Some(fields) = record.as_object_mut() {
            fields.insert("id".to_owned(), Value::String(id.clone()));
        }
        let line = serialize_json_line(&record)?;
        input
            .send((line, id))
            .map_err(|_| RpcClientError::Stopped)?;
        let response = answer.await.map_err(|_| RpcClientError::Stopped)??;
        match response {
            RpcResponse::Success { data, .. } => Ok(data),
            RpcResponse::Error { error, .. } => Err(RpcClientError::Response(error)),
            RpcResponse::Workspace(_) => Err(unexpected_data()),
        }
    }
}

impl Inner {
    /// Settles the pending request a line answers, or delivers the line's record to the
    /// event callbacks; a line that is not JSON, or is `null`, is ignored.
    pub(super) fn handle_line(&self, line: &str) {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            return;
        };
        if record.is_null() {
            return;
        }
        let answered = (record["type"] == "response")
            .then(|| record.get("id").and_then(Value::as_str))
            .flatten()
            .and_then(|id| self.take_before_deadline(id));
        match answered {
            Some(respond) => {
                let _ =
                    respond.send(RpcResponse::deserialize(record).map_err(RpcClientError::from));
            }
            None => self.dispatch(&record),
        }
    }

    /// Settles request `id` with the failure to write it, unless it has settled already.
    pub(super) fn fail_write(&self, id: &str, error: std::io::Error) {
        if let Some(respond) = self.take_before_deadline(id) {
            let _ = respond.send(Err(RpcClientError::Io(error)));
        }
    }

    /// Removes and returns pending request `id` while its deadline has not come; from the
    /// deadline on, only its timer settles it, and an answer goes to the event callbacks,
    /// like the answer to a request that has already timed out.
    fn take_before_deadline(&self, id: &str) -> Option<Responder> {
        let mut state = self.state.borrow_mut();
        if state.pending.get(id)?.deadline <= Instant::now() {
            return None;
        }
        state.pending.remove(id).map(|pending| pending.respond)
    }

    /// Settles request `id` of type `command` with its timeout, unless it has settled
    /// already; the failure carries the diagnostic text of this moment.
    fn expire(&self, id: &str, command: String) {
        let expired = self.state.borrow_mut().pending.remove(id);
        if let Some(pending) = expired {
            let stderr = self.state.borrow().stderr.clone();
            let _ = pending
                .respond
                .send(Err(RpcClientError::RequestTimeout { command, stderr }));
        }
    }

    /// Ends every pending request whose deadline has not come with
    /// [`RpcClientError::Stopped`]; the others are left to their timers.
    pub(super) fn stop_requests(&self) {
        let now = Instant::now();
        let ended: Vec<_> = self
            .state
            .borrow_mut()
            .pending
            .extract_if(|_, pending| pending.deadline > now)
            .collect();
        drop(ended);
    }
}

/// The failure for a successful answer whose data does not have its command's shape.
pub(super) fn unexpected_data() -> RpcClientError {
    RpcClientError::Decode(serde::de::Error::custom(
        "the answer's data does not have its command's shape",
    ))
}
