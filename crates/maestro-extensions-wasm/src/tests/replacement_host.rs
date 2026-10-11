//! Supplied operation outcomes and literal operands shared by both hosts.
use super::observed::{Hold, Observed};
use serde::Deserialize;
use serde_json::{Value, json};
/// Decisions injected before admitting an operation's continuation or message.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Bank {
    /// Host error before any effect.
    pub error: Option<String>,
    /// Reject before recording host admission.
    pub stale: Option<bool>,
    /// Reserve the next gate for the admitted author callback.
    pub hold_callback: Option<bool>,
    /// Try the same callable slot again before the operation owner ends.
    pub repeat: Option<bool>,
    /// Host error after the send effect.
    pub after_error: Option<String>,
    /// Cancellation result.
    pub cancelled: bool,
    /// Leave the callback unused.
    pub unused: bool,
}
/// Record operands before exposing the pending host operation.
pub fn start(
    observed: &mut Observed,
    group: &str,
    id: &str,
    data: Option<&str>,
    callback: bool,
) -> Option<Hold> {
    if observed.replacement.stale == Some(true) {
        return None;
    }
    observed.log(format!(
        "replacement-call {}",
        json!({"group":group,"id":id,"data":super::setup_host::document(data),"callback":callback})
    ));
    if observed.replacement.hold_callback == Some(true) {
        None
    } else {
        observed.take_hold()
    }
}
/// Supply the selected outcome after the host's pending phase ends.
pub fn outcome(observed: &Observed) -> Result<bool, String> {
    match &observed.replacement.error {
        Some(error) => Err(error.clone()),
        None => Ok(observed.replacement.cancelled),
    }
}
/// Signal the identity of the send admitted before its completion gate.
pub fn send_start(observed: &mut Observed, group: &str, target: &str) -> Option<Hold> {
    observed.log(format!(
        "replacement-send-start {}",
        json!({"group":group,"target":target})
    ));
    observed.take_hold()
}
/// Record a message on the operation-bound reader, preserving order and repetition.
pub fn send(
    observed: &mut Observed,
    reader: &super::reader_host::Reader,
    route: (&str, &str),
    payload: &str,
    options: Option<&str>,
) -> Result<(), String> {
    let (group, target) = route;
    outcome(observed)?;
    let payload: Value = serde_json::from_str(payload).unwrap();
    observed.log(format!("replacement-send {}", json!({"group":group,"target":target,"payload":payload,"options":super::setup_host::document(options)})));
    let mut data = reader.0.lock().unwrap();
    let reply = data
        .replies
        .iter_mut()
        .find(|r| r.method == "getSessionName")
        .unwrap();
    let mut state: Vec<Value> =
        serde_json::from_str(reply.value.as_str().unwrap()).unwrap_or_default();
    state.push(json!({"group":group,"payload":payload}));
    reply.value = json!(serde_json::to_string(&state).unwrap());
    drop(data);
    observed.replacement.after_error.clone().map_or(Ok(()), Err)
}

use super::host::{Identity, Imports, Replaced, State, borrow};
use crate::bindings::host_side::maestro::extension::session::SessionChangeResult;
use wasmtime::component::{Accessor, Resource};
/// Invoke a continuation with an independently owned target and reader.
async fn continue_target(
    accessor: &Accessor<State, Imports>,
    callback: Option<Resource<Identity>>,
    id: &str,
) -> wasmtime::Result<Result<(), String>> {
    let Some(callback) = callback else {
        return Ok(Ok(()));
    };
    let (guest, target) = accessor.with(|mut a| -> wasmtime::Result<_> {
        let state = a.get();
        Ok((
            state.guest.clone().unwrap(),
            state.push(Replaced(
                super::observed::Session::new(id),
                Some(super::setup_host::reader(id)),
            ))?,
        ))
    })?;
    guest
        .call_invoke_with_session(accessor, borrow(callback.rep()), target)
        .await
}
/// Record the operands, pause at admission, and select the supplied terminal outcome.
pub(super) async fn command(
    accessor: &Accessor<State, Imports>,
    group: &str,
    id: &str,
    data: Option<&str>,
    callback: Option<Resource<Identity>>,
) -> wasmtime::Result<Result<SessionChangeResult, String>> {
    let _completion =
        Completion(accessor.with(|mut a| a.get().observed.replacement_finished.take()));
    let hold =
        accessor.with(|mut a| start(&mut a.get().observed, group, id, data, callback.is_some()));
    super::observed::pause(hold).await;
    let result = accessor.with(|mut a| outcome(&a.get().observed));
    let cancelled = match result {
        Ok(value) => value,
        Err(error) => return Ok(Err(error)),
    };
    accessor.with(|mut a| a.get().observed.log("replacement-rebound"));
    let repeated = callback.as_ref().map(Resource::rep);
    if !cancelled
        && !accessor.with(|mut a| a.get().observed.replacement.unused)
        && let Err(error) = continue_target(accessor, callback, id).await?
    {
        return Ok(Err(error));
    }
    if accessor.with(|mut a| a.get().observed.replacement.repeat == Some(true))
        && let Some(id) = repeated
    {
        let error = continue_target(accessor, Some(borrow(id)), "repeat-target")
            .await?
            .unwrap_err();
        accessor.with(|mut a| a.get().observed.log(format!("replacement-repeat {error}")));
    }
    Ok(Ok(SessionChangeResult { cancelled }))
}
/// Await a send on its retained operation reader, then expose the resulting state.
async fn send_bound(
    accessor: &Accessor<State, Imports>,
    this: Resource<Replaced>,
    group: &str,
    payload: &str,
    options: Option<&str>,
) -> wasmtime::Result<Result<(), String>> {
    let hold = accessor.with(|mut a| -> wasmtime::Result<_> {
        let state = a.get();
        let target = state.table.get(&this)?.0.cwd.clone();
        Ok(send_start(&mut state.observed, group, &target))
    })?;
    super::observed::pause(hold).await;
    accessor.with(|mut a| {
        let state = a.get();
        let target = state.table.get(&this)?.0.cwd.clone();
        let reader = state.table.get(&this)?.1.clone().unwrap();
        Ok(send(
            &mut state.observed,
            &reader,
            (group, &target),
            payload,
            options,
        ))
    })
}
impl
    crate::bindings::host_side::maestro::extension::host::HostReplacedSessionContextWithStore<State>
    for Imports
{
    async fn send_message(
        accessor: &Accessor<State, Self>,
        this: Resource<Replaced>,
        payload: String,
        options: Option<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        send_bound(accessor, this, "sendMessage", &payload, options.as_deref()).await
    }
    async fn send_user_message(
        accessor: &Accessor<State, Self>,
        this: Resource<Replaced>,
        payload: String,
        options: Option<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        send_bound(
            accessor,
            this,
            "sendUserMessage",
            &payload,
            options.as_deref(),
        )
        .await
    }
}

/// Observe the command resource supplied to the host, independently of operation operands.
pub(super) fn binding(
    accessor: &Accessor<State, Imports>,
    this: &Resource<super::host::Command>,
) -> wasmtime::Result<()> {
    accessor.with(|mut a| {
        let state = a.get();
        let cwd = state.table.get(this)?.0.cwd.clone();
        state
            .observed
            .log(format!("replacement-binding {}", json!(cwd)));
        Ok(())
    })
}

/// Completion notification after this host invocation ends or is dropped.
pub(super) struct Completion(pub(super) Option<tokio::sync::oneshot::Sender<()>>);
impl Drop for Completion {
    fn drop(&mut self) {
        if let Some(done) = self.0.take() {
            let _ = done.send(());
        }
    }
}
