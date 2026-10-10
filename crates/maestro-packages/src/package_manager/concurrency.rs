//! Four owned workers with indexed results and independent failure delivery.
use super::{
    DefaultPackageManager, PackageOperations,
    update::{Candidate, PackageUpdate},
};
use std::{cell::RefCell, io, rc::Rc};
use tokio::sync::mpsc;

/// The input iterator retained by each admitted worker.
type Pending = Rc<RefCell<std::iter::Enumerate<std::vec::IntoIter<Candidate>>>>;
/// Indexed worker outcomes delivered independently of the receiver lifetime.
type Sender = mpsc::UnboundedSender<(usize, io::Result<Option<PackageUpdate>>)>;

impl<O: PackageOperations + 'static> DefaultPackageManager<O> {
    /// Keeps admitted workers alive independently of the result receiver.
    pub(super) async fn check_candidates(
        self: &Rc<Self>,
        candidates: Vec<Candidate>,
    ) -> io::Result<Vec<PackageUpdate>> {
        let count = candidates.len();
        if count == 0 {
            return Ok(Vec::new());
        }
        let pending = Rc::new(RefCell::new(candidates.into_iter().enumerate()));
        let (send, mut receive) = mpsc::unbounded_channel();
        for _ in 0..count.min(4) {
            let manager = Rc::clone(self);
            let pending = Rc::clone(&pending);
            let send = send.clone();
            self.operations
                .spawn(Box::pin(manager.check_worker(pending, send)))?;
        }
        drop(send);
        let mut results: Vec<Option<PackageUpdate>> = (0..count).map(|_| None).collect();
        for _ in 0..count {
            let (index, result) = receive
                .recv()
                .await
                .ok_or_else(|| io::Error::other("package workers stopped before completion"))?;
            results[index] = result?;
        }
        Ok(results.into_iter().flatten().collect())
    }
    /// Stops only this worker on failure; successful workers keep claiming inputs.
    async fn check_worker(self: Rc<Self>, pending: Pending, send: Sender) {
        loop {
            let task = pending.borrow_mut().next();
            let Some((index, candidate)) = task else {
                break;
            };
            let result = self.check_candidate(candidate).await;
            let failed = result.is_err();
            let _ = send.send((index, result));
            if failed {
                break;
            }
        }
    }
}
