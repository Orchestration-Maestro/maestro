//! Readiness-driven terminal byte delivery.
use super::{
    PtyIoActorConfig,
    controls::{Admission, Control},
    unix::{ReadCallback, ReaderExitCallback},
};
pub(super) use crate::byte_queue::{WriteStatus, Writes};
use std::{fs::File, io, sync::Arc};
use tokio::{
    io::{Interest, unix::AsyncFd},
    sync::mpsc,
};

/// Owns the master, byte offsets, callbacks and input receivers.
pub(super) struct Runner {
    /// Readiness registration and sole terminal descriptor.
    pub(super) fd: AsyncFd<File>,
    /// Diagnostic pane identity.
    pub(super) pane: u32,
    /// User bytes awaiting admission to the write list.
    data: mpsc::Receiver<Vec<u8>>,
    /// Capacity-independent controls.
    control: mpsc::UnboundedReceiver<Control>,
    /// Coalesced controls and callback ordering.
    pub(super) controls: Arc<super::controls::Controls>,
    /// Shared admission revoked on every exit.
    admission: Arc<Admission>,
    /// Whether terminal reads and user input are active.
    running: bool,
    /// Ordered writes, including a partially written front element.
    pub(super) writes: Writes,
    /// Owned terminal consumer.
    on_read: ReadCallback,
    /// Owned one-shot exit notification.
    on_exit: Option<ReaderExitCallback>,
}

impl Runner {
    /// Registers the owned nonblocking descriptor.
    pub(super) fn new(
        config: PtyIoActorConfig,
        data: mpsc::Receiver<Vec<u8>>,
        control: mpsc::UnboundedReceiver<Control>,
        admission: Arc<Admission>,
        controls: Arc<super::controls::Controls>,
    ) -> io::Result<Self> {
        // The advisory covers a descriptor the registration does not own; this one owns the file.
        #[expect(
            deprecated,
            reason = "owns its descriptor; the advisory concerns unowned ones"
        )]
        let fd = AsyncFd::new(File::from(config.master_fd))?;
        Ok(Self {
            fd,
            pane: config.pane_id,
            data,
            control,
            admission,
            controls,
            running: !config.initially_quiesced,
            writes: Writes::default(),
            on_read: config.on_read,
            on_exit: config.on_reader_exit,
        })
    }

    /// Drops the descriptor and callback owner before acknowledging committed release.
    pub(super) async fn run(mut self) {
        let reply = self.drive().await;
        self.admission.close();
        if let Some(on_exit) = self.on_exit.take() {
            on_exit().await;
        }
        drop(self);
        if let Some(reply) = reply {
            let _ = reply.send(Ok(()));
        }
    }

    /// Gives independent controls priority over data and readiness.
    async fn drive(&mut self) -> Option<super::controls::Reply<()>> {
        loop {
            if let Flow::Exit(reply) = self.pending_controls().await {
                return reply;
            }
            self.apply_controls().await;
            self.flush();
            let interest = self.interest();
            tokio::select! {
                biased;
                command = self.control.recv() => {
                    let command=command?;
                    if let Flow::Exit(reply)=self.control(command).await {return reply;}
                },
                () = self.controls.changed.notified() => {},
                bytes = self.data.recv() => {
                    let bytes=bytes?;
                    if self.running {self.enqueue(bytes);}
                },
                ready = self.fd.ready(interest), if self.running || !self.writes.is_empty() => {
                    let readiness = ready.ok()?.ready();
                    if !self.service_ready(readiness) {return None;}
                },
            }
            tokio::task::consume_budget().await;
        }
    }

    /// Requests every active I/O direction in a single readiness wait.
    pub(in crate::terminal::channels) fn interest(&self) -> Interest {
        match (self.running, self.writes.is_empty()) {
            (true, false) => Interest::READABLE | Interest::WRITABLE,
            (true, true) => Interest::READABLE,
            (false, _) => Interest::WRITABLE,
        }
    }

    /// Services both ready directions before returning to control admission.
    pub(in crate::terminal::channels) fn service_ready(
        &mut self,
        readiness: tokio::io::Ready,
    ) -> bool {
        if readiness.is_readable() && self.running && !self.read_once() {
            return false;
        }
        if readiness.is_writable() && !self.writes.is_empty() {
            self.flush();
        }
        true
    }

    /// Drains independent controls before applying coalesced geometry or data.
    async fn pending_controls(&mut self) -> Flow {
        while let Ok(command) = self.control.try_recv() {
            if let exit @ Flow::Exit(_) = self.control(command).await {
                return exit;
            }
        }
        Flow::Continue
    }

    /// Keeps only nonempty terminal writes.
    pub(super) fn enqueue(&mut self, bytes: Vec<u8>) {
        self.writes.enqueue(bytes);
    }

    /// Reads one available chunk and appends its ordered terminal replies.
    fn read_once(&mut self) -> bool {
        let mut bytes = [0; 8192];
        let result = self.fd.try_io(Interest::READABLE, |file| {
            rustix::io::read(file, &mut bytes).map_err(Into::into)
        });
        self.read_result(&bytes, result)
    }

    /// Handles native read outcomes at the callback boundary.
    pub(super) fn read_result(&mut self, bytes: &[u8], result: io::Result<usize>) -> bool {
        match result {
            Ok(0) => false,
            Ok(length) => {
                let controls = Arc::clone(&self.controls);
                let lease = controls.order.enter();
                let result = (self.on_read)(&bytes[..length]);
                controls
                    .pending
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .responses
                    .extend(result.terminal_responses);
                drop(lease);
                let responses = std::mem::take(
                    &mut controls
                        .pending
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .responses,
                );
                for bytes in responses {
                    self.enqueue(bytes);
                }
                true
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                true
            }
            Err(error) => {
                tracing::debug!(pane=self.pane,err=%error,"PTY actor read failed");
                false
            }
        }
    }

    /// Flushes through readiness, retaining native failures for structured diagnostics.
    fn flush(&mut self) {
        let fd = &self.fd;
        let status = self.writes.flush(|bytes| {
            fd.try_io(Interest::WRITABLE, |file| {
                rustix::io::write(file, bytes).map_err(Into::into)
            })
        });
        self.report_write_status(status);
    }

    /// Records zero and permanently failed native writes without discarding their causes.
    pub(super) fn report_write_status(&self, status: WriteStatus) {
        match status {
            WriteStatus::Zero => {
                tracing::warn!(pane = self.pane, "PTY actor write returned zero bytes");
            }
            WriteStatus::Failed(error) => {
                tracing::warn!(pane=self.pane,err=%error,"PTY actor write failed");
            }
            WriteStatus::Complete | WriteStatus::Blocked => {}
        }
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        self.admission.close();
        tracing::debug!(pane = self.pane, "PTY actor exiting");
    }
}

/// A control either continues I/O or exits with an optional release acknowledgement.
pub(super) enum Flow {
    /// Continue the readiness loop.
    Continue,
    /// Drop the runner before acknowledging committed release.
    Exit(Option<super::controls::Reply<()>>),
}

impl Runner {
    /// Handles one independent control before considering user data.
    async fn control(&mut self, control: Control) -> Flow {
        match control {
            Control::Shutdown => return Flow::Exit(None),
            Control::Release(reply) => return Flow::Exit(Some(reply)),
            Control::Begin(reply) => {
                let result = self.begin_handoff().await;
                let _ = reply.send(result);
            }
            Control::Duplicate(reply) => {
                let result = if self.running {
                    Err(io::Error::other(
                        "PTY actor must be quiesced before handoff duplication",
                    ))
                } else {
                    crate::terminal::fd::duplicate(self.fd.get_ref())
                };
                let _ = reply.send(result);
            }
            Control::Foreground(reply) => {
                let _ = reply.send(
                    crate::terminal::jobs::foreground_process_group_id_for_tty_fd(
                        self.fd.get_ref(),
                    ),
                );
            }
            Control::Rollback(reply) => {
                self.running = true;
                let _ = reply.send(Ok(()));
            }
        }
        Flow::Continue
    }

    /// Drains all admitted user bytes before acknowledging quiescence.
    pub(super) async fn begin_handoff(&mut self) -> io::Result<()> {
        self.admission.wait_drained().await;
        while let Ok(bytes) = self.data.try_recv() {
            self.enqueue(bytes);
        }
        self.apply_controls().await;
        tokio::time::timeout(std::time::Duration::from_secs(2), self.drain())
            .await
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out draining PTY writes before handoff",
                )
            })??;
        self.running = false;
        Ok(())
    }

    /// Keeps reading replies while pending writes wait for terminal capacity.
    pub(super) async fn drain(&mut self) -> io::Result<()> {
        self.flush();
        while !self.writes.is_empty() {
            tokio::select! {
                ready=self.fd.readable()=>{
                    drop(ready?);
                    if !self.read_once() {return Err(io::Error::new(io::ErrorKind::BrokenPipe,"PTY closed while draining writes before handoff"));}
                },
                ready=self.fd.writable()=>{drop(ready?);self.flush();},
            }
            tokio::task::consume_budget().await;
        }
        Ok(())
    }
}
