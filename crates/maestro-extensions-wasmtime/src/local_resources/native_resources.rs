//! Typed native resources serviced by the existing local request pump.
use super::{Admission, Capability, Lease, Registry, Work, pump::State};
use maestro_extensions::native_io::{self, IoFuture};
use std::{
    rc::Rc,
    sync::{Arc, mpsc::Sender},
};
use tokio::sync::oneshot;

/// Synchronous operation result delivered to the Store thread.
type Reply<T> = Sender<Result<T, String>>;
/// Asynchronous operation result delivered without blocking the local pump.
type Async<T> = oneshot::Sender<Result<T, String>>;

/// Store-safe ownership of the original native child.
#[derive(Clone)]
pub(crate) struct Child(pub(crate) Arc<Lease>);
/// Store-safe ownership of the original native byte reader.
#[derive(Clone)]
pub(crate) struct Reader(pub(crate) Arc<Lease>);
/// Store-safe ownership of the original native byte writer.
#[derive(Clone)]
pub(crate) struct Writer(pub(crate) Arc<Lease>);

/// Literal operands for one native child launch.
pub(crate) struct Spawn {
    /// Program name without shell interpretation.
    pub(crate) command: String,
    /// Literal argv.
    pub(crate) args: Vec<String>,
    /// Native child selections, with absence preserved.
    pub(crate) options: native_io::SpawnOptions,
}
/// One operation on an original native capability.
pub(crate) enum Request {
    /// Atomic byte publication and a possible postcommit warning.
    SaveFile(String, Vec<u8>, u32, Reply<Option<String>>),
    /// Native permission observation.
    FileMode(String, Reply<u32>),
    /// Native permission change.
    SetFileMode(String, u32, Reply<()>),
    /// Recursive native directory creation.
    CreateDir(String, u32, Reply<()>),
    /// Admits a literal native launch.
    Spawn(Spawn, Async<Child>, Admission),
    /// Signals a native PID or supported process group.
    Kill(i32, Option<String>, Reply<()>),
    /// Reads the original admitted child's PID.
    Pid(Reply<Option<u32>>),
    /// Acquires the original input pipe without resetting it.
    Stdin(Reply<Option<Writer>>),
    /// Acquires one original output pipe without resetting it.
    Output(bool, Reply<Option<Reader>>),
    /// Waits for the sole native collector's completion.
    Wait(Async<native_io::ExitStatus>, Admission),
    /// Signals this original child only while it is live.
    ChildKill(Option<String>, Reply<bool>),
    /// Consumes the next raw output chunk.
    Read(Async<Option<Vec<u8>>>, Admission),
    /// Closes only the selected byte consumer.
    Close(Reply<()>),
    /// Writes every admitted byte in order.
    Write(Vec<u8>, Async<()>, Admission),
    /// Closes input after admitted writes.
    End(Async<()>, Admission),
}
impl Request {
    /// Settles a rejected capability without stranding a native caller.
    fn reject(self, error: String) {
        match self {
            Self::Spawn(_, reply, _) => {
                let _ = reply.send(Err(error));
            }
            Self::SaveFile(_, _, _, reply) => {
                let _ = reply.send(Err(error));
            }
            Self::FileMode(_, reply) => {
                let _ = reply.send(Err(error));
            }
            Self::SetFileMode(_, _, reply)
            | Self::CreateDir(_, _, reply)
            | Self::Kill(_, _, reply)
            | Self::Close(reply) => {
                let _ = reply.send(Err(error));
            }
            Self::Pid(reply) => {
                let _ = reply.send(Err(error));
            }
            Self::Stdin(reply) => {
                let _ = reply.send(Err(error));
            }
            Self::Output(_, reply) => {
                let _ = reply.send(Err(error));
            }
            Self::Wait(reply, _) => {
                let _ = reply.send(Err(error));
            }
            Self::ChildKill(_, reply) => {
                let _ = reply.send(Err(error));
            }
            Self::Read(reply, _) => {
                let _ = reply.send(Err(error));
            }
            Self::Write(_, reply, _) | Self::End(reply, _) => {
                let _ = reply.send(Err(error));
            }
        }
    }
}
impl Registry {
    /// Retains the original native child, not a reconstructed identity or snapshot.
    fn child(&self, child: Rc<dyn native_io::ChildProcess>) -> Result<Child, String> {
        self.insert(Capability::NativeChild(child)).map(Child)
    }
    /// Gives a byte-consumer alias its own release while preserving shared consumption.
    pub(super) fn reader(&self, reader: Rc<dyn native_io::Readable>) -> Result<Reader, String> {
        self.insert(Capability::NativeReader(reader)).map(Reader)
    }
    /// Gives an input alias its own release without replacing the native writer.
    fn writer(&self, writer: Rc<dyn native_io::Writable>) -> Result<Writer, String> {
        self.insert(Capability::NativeWriter(writer)).map(Writer)
    }
}
impl State {
    /// Selects the original capability outside every registry borrow.
    pub(super) fn native(&self, id: u64, request: Request) {
        let operations = self
            .registry
            .resources
            .borrow()
            .native_io
            .clone()
            .ok_or_else(|| "native I/O capability unavailable".to_owned());
        match request {
            Request::SaveFile(path, bytes, mode, reply) => {
                let _ = reply.send(operations.and_then(|io| io.save_file(&path, &bytes, mode)));
            }
            Request::FileMode(path, reply) => {
                let _ = reply.send(operations.and_then(|io| io.file_mode(&path)));
            }
            Request::SetFileMode(path, mode, reply) => {
                let _ = reply.send(operations.and_then(|io| io.set_file_mode(&path, mode)));
            }
            Request::CreateDir(path, mode, reply) => {
                let _ = reply.send(operations.and_then(|io| io.create_dir(&path, mode)));
            }
            Request::Spawn(spawn, reply, admission) => self.native_spawn(spawn, reply, admission),
            Request::Kill(pid, signal, reply) => {
                let result = operations.and_then(|io| io.kill(pid, signal));
                let _ = reply.send(result);
            }
            request @ (Request::Pid(_)
            | Request::Stdin(_)
            | Request::Output(..)
            | Request::Wait(..)
            | Request::ChildKill(..)) => self.native_child(id, request),
            request => self.native_pipe(id, request),
        }
    }
    /// Admits native startup on the same pending-work set as every other operation.
    fn native_spawn(&self, spawn: Spawn, reply: Async<Child>, admission: Admission) {
        let operations = self.registry.resources.borrow().native_io.clone();
        let Some(io) = operations else {
            let _ = reply.send(Err("native I/O capability unavailable".into()));
            return;
        };
        let registry = self.registry.clone();
        self.admit(
            Box::pin(async move {
                let result = io
                    .spawn(spawn.command, spawn.args, spawn.options)
                    .await
                    .and_then(|child| registry.child(child));
                let _ = reply.send(result);
            }),
            admission,
        );
    }
    /// Retains the exact child while controls and independently owned waits run.
    fn native_child(&self, id: u64, request: Request) {
        let capability = self.registry.capability(id);
        let Some(Capability::NativeChild(child)) = capability.as_deref() else {
            request.reject("native child resource ended".into());
            return;
        };
        match request {
            Request::Pid(reply) => {
                let _ = reply.send(Ok(child.pid()));
            }
            Request::Stdin(reply) => {
                let _ = reply.send(
                    child
                        .stdin()
                        .map(|pipe| self.registry.writer(pipe))
                        .transpose(),
                );
            }
            Request::Output(stderr, reply) => {
                let pipe = if stderr {
                    child.stderr()
                } else {
                    child.stdout()
                };
                let _ = reply.send(pipe.map(|pipe| self.registry.reader(pipe)).transpose());
            }
            Request::ChildKill(signal, reply) => {
                let _ = reply.send(child.kill(signal));
            }
            Request::Wait(reply, admission) => {
                let child = child.clone();
                self.admit(
                    consuming(Box::pin(async move { child.wait().await }), reply),
                    admission,
                );
            }
            request => request.reject("operation requires a byte resource".into()),
        }
    }
    /// Admits byte operations without blocking sibling descriptors or controls.
    fn native_pipe(&self, id: u64, request: Request) {
        let capability = self.registry.capability(id);
        match (capability.as_deref(), request) {
            (Some(Capability::NativeReader(reader)), Request::Read(reply, admission)) => {
                let reader = reader.clone();
                self.admit(
                    consuming(Box::pin(async move { reader.read().await }), reply),
                    admission,
                );
            }
            (Some(Capability::NativeReader(reader)), Request::Close(reply)) => {
                let _ = reply.send(reader.close());
            }
            (Some(Capability::NativeWriter(writer)), Request::Write(bytes, reply, admission)) => {
                let writer = writer.clone();
                self.admit(
                    Box::pin(async move {
                        let _ = reply.send(writer.write(&bytes).await);
                    }),
                    admission,
                );
            }
            (Some(Capability::NativeWriter(writer)), Request::End(reply, admission)) => {
                let writer = writer.clone();
                self.admit(
                    Box::pin(async move {
                        let _ = reply.send(writer.end().await);
                    }),
                    admission,
                );
            }
            (_, request) => request.reject("native byte resource ended".into()),
        }
    }
}

/// A dropped consumer stops only its pending observation, not the underlying native owner.
pub(super) fn consuming<T: 'static>(work: IoFuture<'static, T>, mut reply: Async<T>) -> Work {
    Box::pin(async move {
        let result = {
            let closed = std::pin::pin!(reply.closed());
            match futures_util::future::select(work, closed).await {
                futures_util::future::Either::Left((result, _)) => Some(result),
                futures_util::future::Either::Right(_) => None,
            }
        };
        if let Some(result) = result {
            let _ = reply.send(result);
        }
    })
}
