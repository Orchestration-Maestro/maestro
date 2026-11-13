//! Typed native resources serviced by the existing local request pump.
use super::{
    Admission, Capability, Lease, Registry, Work,
    held::{NativeChild, NativeReader, NativeWriter},
    pump::State,
};
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
pub struct Child(pub(crate) Arc<Lease>);
/// Store-safe ownership of the original native byte reader.
#[derive(Clone)]
pub struct Reader(pub(crate) Arc<Lease>);
/// Store-safe ownership of the original native byte writer.
#[derive(Clone)]
pub struct Writer(pub(crate) Arc<Lease>);

impl Child {
    /// Reads the original admitted child's PID.
    pub(crate) fn pid(&self) -> Result<Option<u32>, String> {
        self.0.run::<NativeChild, _>(|child, _| Ok(child.pid()))
    }
    /// Acquires the original input pipe without resetting it.
    pub(crate) fn stdin(&self) -> Result<Option<Writer>, String> {
        self.0.run::<NativeChild, _>(|child, registry| {
            child.stdin().map(|pipe| registry.writer(pipe)).transpose()
        })
    }
    /// Acquires one original output pipe without resetting it.
    pub(crate) fn output(&self, stderr: bool) -> Result<Option<Reader>, String> {
        self.0.run::<NativeChild, _>(move |child, registry| {
            let pipe = if stderr {
                child.stderr()
            } else {
                child.stdout()
            };
            pipe.map(|pipe| registry.reader(pipe)).transpose()
        })
    }
    /// Signals this original child only while it is live.
    pub(crate) fn kill(&self, signal: Option<String>) -> Result<bool, String> {
        self.0
            .run::<NativeChild, _>(move |child, _| child.kill(signal))
    }
}
impl Child {
    /// Waits for the sole native collector's completion; a dropped wait leaves the child alone.
    pub(crate) async fn wait(&self) -> Result<native_io::ExitStatus, String> {
        self.0
            .observe::<NativeChild, _>(|child, _| {
                let child = child.clone();
                Ok(Box::pin(async move { child.wait().await }) as IoFuture<'static, _>)
            })
            .await
    }
}
impl Writer {
    /// Writes every admitted byte in order.
    pub(crate) async fn write(&self, bytes: Vec<u8>) -> Result<(), String> {
        self.0
            .admit::<NativeWriter, _, _>(move |writer, _| {
                let writer = writer.clone();
                Ok(Box::pin(async move { writer.write(&bytes).await }) as IoFuture<'static, _>)
            })
            .await
    }
    /// Closes input after admitted writes.
    pub(crate) async fn end(&self) -> Result<(), String> {
        self.0
            .admit::<NativeWriter, _, _>(|writer, _| {
                let writer = writer.clone();
                Ok(Box::pin(async move { writer.end().await }) as IoFuture<'static, _>)
            })
            .await
    }
}
impl Reader {
    /// Consumes the next raw output chunk.
    pub(crate) async fn read(&self) -> Result<Option<Vec<u8>>, String> {
        self.0
            .observe::<NativeReader, _>(|reader, _| {
                let reader = reader.clone();
                Ok(Box::pin(async move { reader.read().await }) as IoFuture<'static, _>)
            })
            .await
    }
    /// Closes only this byte consumer.
    pub(crate) fn close(&self) -> Result<(), String> {
        self.0.run::<NativeReader, _>(|reader, _| reader.close())
    }
}

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
    SaveFile(String, Vec<u8>, u32, Reply<Option<String>>, Admission),
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
    pub(super) fn writer(&self, writer: Rc<dyn native_io::Writable>) -> Result<Writer, String> {
        self.insert(Capability::NativeWriter(writer)).map(Writer)
    }
}
impl State {
    /// Selects the original capability outside every registry borrow.
    pub(super) fn native(&self, request: Request) {
        let operations = self
            .registry
            .resources
            .borrow()
            .native_io
            .clone()
            .ok_or_else(|| "native I/O capability unavailable".to_owned());
        match request {
            Request::SaveFile(path, bytes, mode, reply, admission) => self.admit(
                Box::pin(async move {
                    let result = match operations {
                        Ok(io) => io.save_file(path, bytes, mode).await,
                        Err(error) => Err(error),
                    };
                    let _ = reply.send(result);
                }),
                admission,
            ),
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
