//! One reactor writer with synchronous takeover of retained output at shutdown.

use std::cell::{Cell, RefCell};
use std::io;
use std::num::NonZeroUsize;
use std::os::fd::AsFd;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll};

use maestro_process::byte_queue::{WriteStatus, Writes};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::io::{Errno, write};
use rustix::stdio::stdout;
use tokio::io::Interest;
use tokio::sync::mpsc;
use tokio::task::{JoinHandle, LocalSet};

use super::descriptor::Source;
use crate::output_errors::FailureSource;
use crate::terminal::{Shared, output::OutputWriter};

/// Largest request sent to a terminal in one syscall.
const MAX_TTY_WRITE_CHUNK_BYTES: NonZeroUsize = NonZeroUsize::MIN.saturating_add(16_383);

/// Retained data, independent of any suspended writer future.
struct Inner {
    /// Ordered bytes and exact partial-front accounting.
    queue: RefCell<Writes>,
    /// Whether a permanent failure has ended delivery.
    dead: Cell<bool>,
    /// The one ingress receives both host frames and foreign-thread writes.
    incoming: RefCell<mpsc::UnboundedReceiver<Vec<u8>>>,
}

impl Inner {
    /// Moves every currently admitted message once into retained byte advancement.
    fn receive(&self) {
        while let Ok(bytes) = self.incoming.borrow_mut().try_recv() {
            self.queue.borrow_mut().enqueue(bytes);
        }
    }

    /// Checks retained work before parking, including bytes moved by backlog observation.
    fn poll_receive(&self, cx: &mut Context<'_>) -> Poll<bool> {
        if !self.queue.borrow().is_empty() {
            return Poll::Ready(true);
        }
        match self.incoming.borrow_mut().poll_recv(cx) {
            Poll::Ready(Some(bytes)) => {
                self.queue.borrow_mut().enqueue(bytes);
                Poll::Ready(true)
            }
            Poll::Ready(None) => Poll::Ready(false),
            Poll::Pending => Poll::Pending,
        }
    }

    /// Advances the same retained queue for asynchronous and final synchronous writes.
    fn advance(&self, source: &impl AsFd) -> io::Result<()> {
        let status = self
            .queue
            .borrow_mut()
            .flush_bounded(MAX_TTY_WRITE_CHUNK_BYTES, |bytes| write_once(source, bytes));
        write_result(status)
    }

    /// Discards the failed queue and refuses later admission.
    fn fail(&self) {
        self.dead.set(true);
        self.incoming.borrow_mut().close();
        while self.incoming.borrow_mut().try_recv().is_ok() {}
        *self.queue.borrow_mut() = Writes::default();
    }
}

/// Converts queue progress into readiness or the original write failure.
fn write_result(status: WriteStatus) -> io::Result<()> {
    match status {
        WriteStatus::Complete => Ok(()),
        WriteStatus::Blocked => Err(io::ErrorKind::WouldBlock.into()),
        WriteStatus::Zero => Err(io::ErrorKind::WriteZero.into()),
        WriteStatus::Failed(error) => Err(error),
    }
}

/// Owns output readiness, the retained queue and its one writer task.
pub(super) struct TtyWriter {
    /// Descriptor kept alive through synchronous takeover.
    source: Rc<Source>,
    /// Data retained even when the asynchronous task is cancelled.
    inner: Rc<Inner>,
    /// The task to revoke before final drain.
    task: JoinHandle<()>,
    /// Every producer admits through the same ordered ingress.
    sender: OutputWriter,
}

impl TtyWriter {
    /// Registers output when supported, leaving regular destinations synchronous.
    pub(super) fn new(local: &LocalSet, shared: Weak<Shared>) -> io::Result<Option<Self>> {
        let source = Rc::new(Source::open(stdout(), Interest::WRITABLE)?);
        if source.reactor().is_none() {
            return Ok(None);
        }
        let (sender, incoming) = mpsc::unbounded_channel();
        let sender = OutputWriter::new(sender);
        let inner = Rc::new(Inner {
            queue: RefCell::default(),
            dead: Cell::new(false),
            incoming: RefCell::new(incoming),
        });
        let task = local.spawn_local(pump(
            Rc::clone(&source),
            Rc::clone(&inner),
            shared,
            sender.clone(),
        ));
        Ok(Some(Self {
            source,
            inner,
            task,
            sender,
        }))
    }

    /// Admits owned bytes before waking the writer; failed queues accept no more data.
    pub(super) fn write(&self, data: &str) -> io::Result<()> {
        if data.is_empty() || self.inner.dead.get() {
            return Ok(());
        }
        self.sender.admit(data.as_bytes())
    }

    /// Clones the sendable admission handle without sharing the local queue.
    pub(super) fn output_writer(&self) -> crate::terminal::output::OutputWriter {
        self.sender.clone()
    }

    /// Accepted bytes not yet written, including a partially drained front.
    pub(super) fn pending(&self) -> usize {
        self.inner.receive();
        self.inner.queue.borrow().pending()
    }

    /// Revokes asynchronous delivery and drains before restoring descriptor flags.
    pub(super) fn finish(&self) -> io::Result<()> {
        self.task.abort();
        self.sender.close();
        self.inner.incoming.borrow_mut().close();
        self.inner.receive();
        let drained = flush_sync(&self.inner, &self.source);
        drained.and(self.source.restore())
    }
}

/// Writes one request, retrying interrupted syscalls without losing progress.
fn write_once(source: &impl AsFd, bytes: &[u8]) -> io::Result<usize> {
    loop {
        match write(source, bytes) {
            Err(Errno::INTR) => {}
            result => return result.map_err(Into::into),
        }
    }
}

/// Blocks on writability, leaving interruption retry to the drain loop.
fn wait_writable(source: &impl AsFd) -> io::Result<()> {
    let mut waiting = [PollFd::new(source, PollFlags::OUT)];
    match poll(&mut waiting, None) {
        Ok(_) | Err(Errno::INTR) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Advances retained buffers synchronously until empty or permanently failed.
fn flush_sync(inner: &Inner, source: &impl AsFd) -> io::Result<()> {
    while !inner.queue.borrow().is_empty() {
        let result = match inner.advance(source) {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => wait_writable(source),
            result => result,
        };
        if let Err(error) = result {
            inner.fail();
            return Err(error);
        }
    }
    Ok(())
}

/// Uses the same advancement owner for output without a live reactor writer.
pub(super) fn write_sync(text: &str) -> io::Result<()> {
    let (_, incoming) = mpsc::unbounded_channel();
    let inner = Inner {
        queue: RefCell::default(),
        dead: Cell::new(false),
        incoming: RefCell::new(incoming),
    };
    inner.queue.borrow_mut().enqueue(text.as_bytes().to_vec());
    flush_sync(&inner, &stdout())
}

impl Drop for TtyWriter {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

/// Waits without holding the queue borrow, leaving admission and input runnable.
async fn pump(source: Rc<Source>, inner: Rc<Inner>, shared: Weak<Shared>, sender: OutputWriter) {
    let result = drain(&source, &inner).await;
    if let Err(error) = result {
        sender.close();
        inner.fail();
        if let Some(shared) = shared.upgrade() {
            shared.fail(FailureSource::Output, error);
        }
    }
}

/// Drains each admission using the existing descriptor readiness registration.
async fn drain(source: &Source, inner: &Inner) -> io::Result<()> {
    let Some(descriptor) = source.reactor() else {
        return Ok(());
    };
    loop {
        if !std::future::poll_fn(|cx| inner.poll_receive(cx)).await {
            return Ok(());
        }
        inner.receive();
        while !inner.queue.borrow().is_empty() {
            let mut ready = descriptor.writable().await?;
            if let Ok(result) = ready.try_io(|_| inner.advance(source)) {
                result?;
            }
        }
    }
}

#[cfg(test)]
mod tests;
