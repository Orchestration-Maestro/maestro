//! Component ownership of the original native mutation future, without another queue.
use crate::callback_imports::CallbackView;
use std::{
    future::{Future, poll_fn},
    pin::Pin,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    task::{Poll, Waker},
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// The original queue future owns admission; its operation holds the turn for the guest.
type NativeWork =
    Pin<Box<dyn Future<Output = Result<(), maestro_extensions::DiagnosticErrorInfo>> + Send>>;

/// A waiting import shares only the native future and its wakeup with its resource.
#[derive(Default)]
struct Work {
    /// Dropping this future releases the native queue's own admission.
    future: Option<NativeWork>,
    /// Wakes a waiting import when its resource closes.
    waker: Option<Waker>,
}

/// The component resource owns one original native future, not a second queue entry.
struct FileMutation {
    /// Shared only with a currently waiting import.
    work: Arc<Mutex<Work>>,
    /// Set by the native operation when all predecessors have finished.
    acquired: Arc<AtomicBool>,
}
impl FileMutation {
    /// Takes the native queue turn synchronously, before the guest polls its future.
    fn new(path: &str) -> Self {
        let acquired = Arc::new(AtomicBool::new(false));
        let entered = acquired.clone();
        let future = maestro_extensions::with_file_mutation_queue(path, move || async move {
            entered.store(true, Ordering::Relaxed);
            std::future::pending().await
        });
        Self {
            work: Arc::new(Mutex::new(Work {
                future: Some(future),
                waker: None,
            })),
            acquired,
        }
    }
}
impl Drop for FileMutation {
    fn drop(&mut self) {
        let Work { future, waker } = {
            let mut work = self.work.lock().unwrap_or_else(PoisonError::into_inner);
            std::mem::take(&mut *work)
        };
        drop(future);
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

/// Polls only the original native queue; the guest callback never runs under this mutex.
async fn wait(work: Arc<Mutex<Work>>, acquired: Arc<AtomicBool>) -> Result<(), String> {
    poll_fn(|context| {
        let mut work = work.lock().unwrap_or_else(PoisonError::into_inner);
        work.waker = Some(context.waker().clone());
        let result = match work.future.as_mut() {
            None => Poll::Ready(Err("file mutation resource ended".into())),
            Some(future) => match future.as_mut().poll(context) {
                Poll::Ready(result) => Poll::Ready(result.map_err(|error| error.to_string())),
                Poll::Pending if acquired.load(Ordering::Relaxed) => Poll::Ready(Ok(())),
                Poll::Pending => Poll::Pending,
            },
        };
        if result.is_ready() {
            work.waker = None;
        }
        result
    })
    .await
}

/// Links native future creation, waiting and destruction to the canonical resource.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, FileMutation>(host, "file-mutation")?;
    host.func_wrap(
        "with-file-mutation-queue",
        |mut store: StoreContextMut<'_, T>, (path,): (String,)| {
            let resource = store
                .data_mut()
                .ctx()
                .table
                .push(FileMutation::new(&path))?;
            Ok((Ok::<_, String>(resource),))
        },
    )?;
    host.func_wrap_concurrent(
        "[method]file-mutation.wait",
        |accessor, (resource,): (Resource<FileMutation>,)| {
            Box::pin(async move {
                let (work, acquired) = accessor.with(|mut access| {
                    let owner = access.get().ctx().table.get(&resource)?;
                    Ok::<_, wasmtime::Error>((owner.work.clone(), owner.acquired.clone()))
                })?;
                Ok((wait(work, acquired).await,))
            })
        },
    )?;
    Ok(())
}
