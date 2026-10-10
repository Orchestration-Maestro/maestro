//! Up to four owned workers with indexed results and independent failure delivery.
use super::{DefaultPackageManager, PackageFuture, PackageOperations};
use std::{cell::RefCell, io, rc::Rc};
use tokio::sync::mpsc;

/// The owned future iterator retained by admitted workers.
type Pending<T> = Rc<RefCell<std::iter::Enumerate<std::vec::IntoIter<PackageFuture<'static, T>>>>>;
/// Indexed outcomes delivered independently of the receiver lifetime.
type Sender<T> = mpsc::UnboundedSender<(usize, io::Result<T>)>;

impl<O: PackageOperations + 'static> DefaultPackageManager<O> {
    /// Keeps admitted work alive independently of the result receiver.
    pub(super) async fn run_with_concurrency<T: 'static>(
        &self,
        tasks: Vec<PackageFuture<'static, T>>,
    ) -> io::Result<Vec<T>> {
        let count = tasks.len();
        if count == 0 {
            return Ok(Vec::new());
        }
        let pending = Rc::new(RefCell::new(tasks.into_iter().enumerate()));
        let (send, mut receive) = mpsc::unbounded_channel();
        for _ in 0..count.min(4) {
            self.operations
                .spawn(Box::pin(worker(Rc::clone(&pending), send.clone())))?;
        }
        drop(send);
        let mut results: Vec<Option<T>> = (0..count).map(|_| None).collect();
        for _ in 0..count {
            let (index, result) = receive
                .recv()
                .await
                .ok_or_else(|| io::Error::other("package workers stopped before completion"))?;
            results[index] = Some(result?);
        }
        Ok(results.into_iter().flatten().collect())
    }
}
/// Stops only this worker on failure; successful workers keep claiming inputs.
async fn worker<T>(pending: Pending<T>, send: Sender<T>) {
    loop {
        let task = pending.borrow_mut().next();
        let Some((index, task)) = task else { break };
        let result = task.await;
        let failed = result.is_err();
        let _ = send.send((index, result));
        if failed {
            break;
        }
    }
}
