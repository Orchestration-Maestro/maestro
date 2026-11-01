//! Per-file ordering of mutations: operations on one file run one at a time, in call order.

use crate::{path_utils::resolve_path, utils::diagnostics::diagnostic};
use maestro_models::{BoxFuture, DiagnosticErrorInfo};
use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    io,
    path::PathBuf,
    sync::{
        Arc, LazyLock, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::oneshot;

/// A caller admitted to a file's queue; the front entry is the one allowed to run.
struct Waiter {
    /// Identifies the caller among the live entries of its file.
    id: u64,
    /// Wakes the caller once it reaches the front; `None` when it is already there.
    wake: Option<oneshot::Sender<()>>,
}

/// The live queues by file identity; a file with no admitted caller has no entry.
static QUEUES: LazyLock<Mutex<HashMap<PathBuf, VecDeque<Waiter>>>> = LazyLock::new(Mutex::default);

/// Source of caller identities.
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Lock the queues; a panic in another caller never leaves them half edited.
fn queues() -> MutexGuard<'static, HashMap<PathBuf, VecDeque<Waiter>>> {
    QUEUES.lock().unwrap_or_else(PoisonError::into_inner)
}

tokio::task_local! {
    /// The admission of the queued operation running on this task.
    static RUNNING: Arc<Admission>;
}

/// The running queued operation's admission, so work that outlives the operation can keep the
/// file's queue held; `None` outside a queued operation.
pub(crate) fn running_admission() -> Option<Arc<Admission>> {
    RUNNING.try_with(Arc::clone).ok()
}

/// The file's identity: its canonical path, or its lexical absolute path when the file or a
/// parent directory does not exist yet. Any other canonicalization failure is returned.
fn queue_key(file_path: &str) -> io::Result<PathBuf> {
    let resolved = PathBuf::from(resolve_path(&[file_path])?);
    match std::fs::canonicalize(&resolved) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(resolved),
        canonical => canonical,
    }
}

/// A caller's place in its file's queue, given up when dropped.
pub(crate) struct Admission {
    /// File identity.
    key: PathBuf,
    /// Identity of this caller.
    id: u64,
}

impl Admission {
    /// Join the end of the file's queue; the receiver completes when the caller may run.
    fn join(key: PathBuf) -> (Self, Option<oneshot::Receiver<()>>) {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let mut map = queues();
        let waiters = map.entry(key.clone()).or_default();
        let (wake, receiver) = if waiters.is_empty() {
            (None, None)
        } else {
            let (sender, receiver) = oneshot::channel();
            (Some(sender), Some(receiver))
        };
        waiters.push_back(Waiter { id, wake });
        (Self { key, id }, receiver)
    }
}

impl Drop for Admission {
    fn drop(&mut self) {
        let successor = {
            let mut map = queues();
            let Some(waiters) = map.get_mut(&self.key) else {
                return;
            };
            let Some(position) = waiters.iter().position(|waiter| waiter.id == self.id) else {
                return;
            };
            waiters.remove(position);
            let successor = if position == 0 {
                waiters.front_mut().and_then(|next| next.wake.take())
            } else {
                None
            };
            if waiters.is_empty() {
                map.remove(&self.key);
            }
            successor
        };
        if let Some(wake) = successor {
            // The receiver is gone only when the successor was dropped, and that removes it.
            let _ = wake.send(());
        }
    }
}

/// Run `operation` once every earlier call for the same file has finished.
///
/// The call takes its place in the queue immediately, before the returned future is first
/// polled. Files are identified by canonical path when it can be resolved, otherwise by lexical
/// absolute path; symlink aliases that canonicalize share a queue and different files proceed
/// independently. A failed operation never blocks later calls. Dropping the returned future
/// before `operation` has started leaves the queue without running it. Blocking file work that
/// the operation started keeps the queue held until it settles, even when the future is dropped
/// first.
///
/// # Errors
/// A canonicalization failure other than a missing path settles the call before `operation`
/// runs.
pub fn with_file_mutation_queue<T, F>(
    file_path: &str,
    operation: impl FnOnce() -> F + Send + 'static,
) -> BoxFuture<Result<T, DiagnosticErrorInfo>>
where
    T: Send + 'static,
    F: Future<Output = Result<T, DiagnosticErrorInfo>> + Send + 'static,
{
    let key = match queue_key(file_path) {
        Ok(key) => key,
        Err(error) => return Box::pin(std::future::ready(Err(diagnostic(&error)))),
    };
    let (admission, turn) = Admission::join(key);
    let admission = Arc::new(admission);
    Box::pin(async move {
        if let Some(turn) = turn {
            // The predecessor's admission drop sends or removes this sender.
            let _ = turn.await;
        }
        let result = RUNNING.scope(Arc::clone(&admission), operation()).await;
        drop(admission);
        result
    })
}

#[cfg(test)]
mod tests;
