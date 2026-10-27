//! Supplied native-resource observations shared by controlled and generated imports.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::child_process::{ExitStatus, SpawnOptions, Stdio};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

/// Supplied byte chunks and an optional late operation failure.
type Chunks = VecDeque<Result<Vec<u8>, String>>;

/// One supplied consuming byte resource, not a native collector.
#[derive(Clone)]
pub struct Reader(Arc<Mutex<Chunks>>, Arc<AtomicBool>, Option<Arc<AtomicBool>>);
impl Reader {
    /// Supplies exact chunks through the existing consuming byte resource.
    pub fn new(chunks: Vec<Vec<u8>>) -> Self {
        Self(
            Arc::new(Mutex::new(chunks.into_iter().map(Ok).collect())),
            Arc::default(),
            None,
        )
    }
    /// Retains the original supplied cancellation flag for later body operations.
    pub fn with_signal(mut self, signal: Option<Arc<AtomicBool>>) -> Self {
        self.2 = signal;
        self
    }
    /// Appends one supplied failure after the existing prefix.
    pub fn fail(&self, error: String) {
        self.0.lock().unwrap().push_back(Err(error));
    }
    /// Consumes the next supplied chunk without decoding it.
    pub fn read(&self) -> Result<Option<Vec<u8>>, String> {
        self.1.store(true, Ordering::SeqCst);
        if self
            .2
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
        {
            self.close();
            return Err("Aborted".into());
        }
        self.0.lock().unwrap().pop_front().transpose()
    }
    /// Claims a whole-body operation only before any supplied read or close.
    pub fn claim(&self) -> bool {
        !self.1.swap(true, Ordering::SeqCst)
    }
    /// Releases all supplied unread chunks.
    pub fn close(&self) {
        self.1.store(true, Ordering::SeqCst);
        self.0.lock().unwrap().clear();
    }
}
/// An input resource that records exact writes and explicit EOF.
#[derive(Clone, Default)]
pub struct Writer {
    /// Bytes received by this particular original writer.
    pub bytes: Arc<Mutex<Vec<u8>>>,
    /// Whether input was closed.
    pub ended: Arc<AtomicBool>,
}
impl Writer {
    /// Records the supplied byte sequence, rejecting writes after EOF.
    pub fn write(&self, bytes: &[u8]) -> Result<(), String> {
        if self.ended.load(Ordering::SeqCst) {
            return Err("controlled input closed".into());
        }
        self.bytes.lock().unwrap().extend_from_slice(bytes);
        Ok(())
    }
}
/// Supplied liveness and the completion selected by the controlled host.
struct ChildState {
    /// Whether a signal may still be delivered.
    live: bool,
    /// Selected numeric exit.
    code: Option<i32>,
    /// Selected terminating signal.
    signal: Option<String>,
}
/// Independent supplied child state and pipe identities.
#[derive(Clone)]
pub struct Child {
    /// Supplied PID.
    pub pid: u32,
    /// Original selected input.
    pub stdin: Option<Writer>,
    /// Original selected output.
    pub stdout: Option<Reader>,
    /// Original selected error output.
    pub stderr: Option<Reader>,
    /// Whether the child is still signalable.
    state: Arc<Mutex<ChildState>>,
}
impl Child {
    /// Returns the supplied completion without consuming either reader.
    pub fn wait(&self) -> ExitStatus {
        let mut state = self.state.lock().unwrap();
        state.live = false;
        ExitStatus {
            code: state.code,
            signal: state.signal.clone(),
        }
    }
    /// Records delivery to this original live child only once.
    pub fn kill(&self, signal: Option<String>) -> bool {
        let mut state = self.state.lock().unwrap();
        if !state.live {
            return false;
        }
        *state = ChildState {
            live: false,
            code: None,
            signal: Some(signal.unwrap_or_else(|| "SIGTERM".into())),
        };
        true
    }
}
/// Literal launches and supplied bytes for the author transport witnesses.
#[derive(Default)]
pub struct Bank {
    /// Literal launch operands, including omitted versus empty fields.
    pub launches: Vec<(String, Vec<String>, SpawnOptions)>,
    /// Supplied chunks for the next child's stdout.
    pub stdout: Vec<Vec<u8>>,
    /// Supplied chunks for the next child's stderr.
    pub stderr: Vec<Vec<u8>>,
    /// Supplied failure after the next child's stdout prefix.
    pub read_error: Option<String>,
    /// Supplied ordinary exit code; omitted selects seven.
    pub exit_code: Option<i32>,
    /// Most recently admitted original child and its resource aliases.
    pub child: Option<Child>,
    /// Supplied operation failure.
    pub error: Option<String>,
    /// Literal global PID and signal operands.
    pub signals: Vec<(i32, Option<String>)>,
}
impl Bank {
    /// Admits supplied metadata and creates one set of shared pipe identities.
    pub fn spawn(
        &mut self,
        command: String,
        args: Vec<String>,
        options: SpawnOptions,
    ) -> Result<Child, String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let reader = |selected: Stdio, bytes: &Vec<Vec<u8>>, error: Option<&String>| {
            (selected == Stdio::Pipe).then(|| {
                let chunks = bytes.iter().cloned().map(Ok).chain(error.cloned().map(Err));
                Reader(Arc::new(Mutex::new(chunks.collect())), Arc::default(), None)
            })
        };
        let child = Child {
            pid: 1234,
            stdin: (options.stdio[0] == Stdio::Pipe).then(Writer::default),
            stdout: reader(options.stdio[1], &self.stdout, self.read_error.as_ref()),
            stderr: reader(options.stdio[2], &self.stderr, None),
            state: Arc::new(Mutex::new(ChildState {
                live: true,
                code: Some(self.exit_code.unwrap_or(7)),
                signal: None,
            })),
        };
        self.launches.push((command, args, options));
        self.child = Some(child.clone());
        Ok(child)
    }
    /// Records native signal operands or returns the supplied failure.
    pub fn kill(&mut self, pid: i32, signal: Option<String>) -> Result<(), String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        self.signals.push((pid, signal));
        Ok(())
    }
}

use super::host::{Imports, State};
use crate::bindings::host_side::maestro::extension::host;
use wasmtime::component::{Accessor, Resource};

/// Converts the generated host's descriptor selections once.
fn descriptor(value: host::Stdio) -> Stdio {
    match value {
        host::Stdio::Pipe => Stdio::Pipe,
        host::Stdio::Ignore => Stdio::Ignore,
        host::Stdio::Inherit => Stdio::Inherit,
    }
}
/// Supplies a child to the generated component through the same observation bank.
pub(super) fn spawn(
    accessor: &Accessor<State, Imports>,
    command: String,
    args: Vec<String>,
    options: host::SpawnOptions,
) -> wasmtime::Result<Result<Resource<Child>, String>> {
    accessor.with(|mut a| {
        let state = a.get();
        let options = SpawnOptions {
            cwd: options.cwd,
            env: options.env,
            detached: options.detached,
            stdio: [
                descriptor(options.stdio.0),
                descriptor(options.stdio.1),
                descriptor(options.stdio.2),
            ],
        };
        match state.observed.native_io.spawn(command, args, options) {
            Ok(child) => Ok(Ok(state.table.push(child)?)),
            Err(error) => Ok(Err(error)),
        }
    })
}
impl host::HostChildProcess for State {
    fn pid(&mut self, child: Resource<Child>) -> wasmtime::Result<Option<u32>> {
        Ok(Some(self.table.get(&child)?.pid))
    }
    fn stdin(&mut self, child: Resource<Child>) -> wasmtime::Result<Option<Resource<Writer>>> {
        self.table
            .get(&child)?
            .stdin
            .clone()
            .map(|pipe| self.table.push(pipe))
            .transpose()
            .map_err(Into::into)
    }
    fn stdout(&mut self, child: Resource<Child>) -> wasmtime::Result<Option<Resource<Reader>>> {
        self.table
            .get(&child)?
            .stdout
            .clone()
            .map(|pipe| self.table.push(pipe))
            .transpose()
            .map_err(Into::into)
    }
    fn stderr(&mut self, child: Resource<Child>) -> wasmtime::Result<Option<Resource<Reader>>> {
        self.table
            .get(&child)?
            .stderr
            .clone()
            .map(|pipe| self.table.push(pipe))
            .transpose()
            .map_err(Into::into)
    }
    fn kill(
        &mut self,
        child: Resource<Child>,
        signal: Option<String>,
    ) -> wasmtime::Result<Result<bool, String>> {
        Ok(Ok(self.table.get(&child)?.kill(signal)))
    }
    fn drop(&mut self, child: Resource<Child>) -> wasmtime::Result<()> {
        self.table.delete(child)?;
        Ok(())
    }
}
impl host::HostChildProcessWithStore<State> for Imports {
    fn wait(
        accessor: &Accessor<State, Self>,
        child: Resource<Child>,
    ) -> impl Future<Output = wasmtime::Result<Result<host::ExitStatus, String>>> {
        std::future::ready(accessor.with(|mut a| {
            let status = a.get().table.get(&child)?.wait();
            Ok(Ok(host::ExitStatus {
                code: status.code,
                signal: status.signal,
            }))
        }))
    }
}
impl host::HostReadable for State {
    fn close(&mut self, reader: Resource<Reader>) -> wasmtime::Result<Result<(), String>> {
        self.table.get(&reader)?.close();
        Ok(Ok(()))
    }
    fn drop(&mut self, reader: Resource<Reader>) -> wasmtime::Result<()> {
        self.table.delete(reader)?;
        Ok(())
    }
}
impl host::HostReadableWithStore<State> for Imports {
    fn read(
        accessor: &Accessor<State, Self>,
        reader: Resource<Reader>,
    ) -> impl Future<Output = wasmtime::Result<Result<Option<Vec<u8>>, String>>> {
        std::future::ready(accessor.with(|mut a| Ok(a.get().table.get(&reader)?.read())))
    }
}
impl host::HostWritable for State {
    fn drop(&mut self, writer: Resource<Writer>) -> wasmtime::Result<()> {
        self.table.delete(writer)?;
        Ok(())
    }
}
impl host::HostWritableWithStore<State> for Imports {
    fn write(
        accessor: &Accessor<State, Self>,
        writer: Resource<Writer>,
        bytes: Vec<u8>,
    ) -> impl Future<Output = wasmtime::Result<Result<(), String>>> {
        std::future::ready(accessor.with(|mut a| Ok(a.get().table.get(&writer)?.write(&bytes))))
    }
    fn end(
        accessor: &Accessor<State, Self>,
        writer: Resource<Writer>,
    ) -> impl Future<Output = wasmtime::Result<Result<(), String>>> {
        std::future::ready(accessor.with(|mut a| {
            a.get()
                .table
                .get(&writer)?
                .ended
                .store(true, Ordering::SeqCst);
            Ok(Ok(()))
        }))
    }
}
