//! Requests: their identifiers, their correlation with answers and their deadline.
use super::{Inner, RpcClient, RpcClientError};
use crate::rpc_types::{RpcData, RpcResponse, RpcSessionCommand};
use maestro_json::jsonl::serialize_json_line;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use tokio::{sync::oneshot, time::Instant};

/// How long a request waits for its answer, from the moment it is sent.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Settles one request with its answer, or with the failure to decode it.
type Responder = oneshot::Sender<Result<RpcResponse, RpcClientError>>;

/// A request awaiting its answer, until its deadline.
pub(super) struct Pending {
    /// Settles the request.
    respond: Responder,
    /// From when an answer no longer counts.
    deadline: Instant,
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
        let deadline = Instant::now() + REQUEST_TIMEOUT;
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
            state
                .pending
                .insert(id.clone(), Pending { respond, deadline });
            (input, id, answer)
        };
        let _pending = PendingRequest {
            inner: &self.inner,
            id: id.clone(),
        };
        if let Some(fields) = record.as_object_mut() {
            fields.insert("id".to_owned(), Value::String(id));
        }
        let line = serialize_json_line(&record)?;
        let exchange = async {
            let (report, written) = oneshot::channel();
            input
                .send((line, report))
                .map_err(|_| RpcClientError::Stopped)?;
            written.await.map_err(|_| RpcClientError::Stopped)??;
            answer.await.map_err(|_| RpcClientError::Stopped)?
        };
        let response = match tokio::time::timeout_at(deadline, exchange).await {
            Ok(response) => response?,
            Err(_) => {
                return Err(RpcClientError::RequestTimeout {
                    command: kind,
                    stderr: self.get_stderr(),
                });
            }
        };
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
        match self.correlated(&record) {
            Some(respond) => {
                let _ =
                    respond.send(RpcResponse::deserialize(record).map_err(RpcClientError::from));
            }
            None => self.dispatch(&record),
        }
    }

    /// Removes and returns the pending request that `record` answers before its deadline;
    /// an answer at or after the deadline goes to the event callbacks, like the answer to a
    /// request that has already timed out.
    fn correlated(&self, record: &Value) -> Option<Responder> {
        if record["type"] != "response" {
            return None;
        }
        let id = record.get("id")?.as_str()?;
        let mut state = self.state.borrow_mut();
        if state.pending.get(id)?.deadline <= Instant::now() {
            return None;
        }
        state.pending.remove(id).map(|pending| pending.respond)
    }
}

/// The failure for a successful answer whose data does not have its command's shape.
pub(super) fn unexpected_data() -> RpcClientError {
    RpcClientError::Decode(serde::de::Error::custom(
        "the answer's data does not have its command's shape",
    ))
}
