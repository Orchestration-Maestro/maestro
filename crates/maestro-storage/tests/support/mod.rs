use maestro_storage::{Dirent, DirentKind, Storage};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

#[derive(Clone)]
enum Node {
    Directory,
    File(Vec<u8>),
}

type Completion = Option<Box<dyn FnOnce()>>;

#[derive(Default)]
pub struct Controlled {
    nodes: RefCell<BTreeMap<PathBuf, Node>>,
    partial: std::cell::Cell<Option<usize>>,
    listings: RefCell<BTreeMap<PathBuf, Vec<Dirent>>>,
    links: RefCell<BTreeMap<PathBuf, PathBuf>>,
    held: std::cell::Cell<bool>,
    requests: RefCell<Vec<Completion>>,
    completions: std::rc::Rc<std::cell::Cell<usize>>,
}

impl Controlled {
    pub fn new(root: &Path) -> Self {
        let this = Self::default();
        this.nodes
            .borrow_mut()
            .insert(root.to_owned(), Node::Directory);
        this
    }
    pub fn link(&self, path: &Path, target: &Path) {
        self.links
            .borrow_mut()
            .insert(path.to_owned(), target.to_owned());
    }
    pub fn hold(&self) {
        self.held.set(true);
    }
    pub fn admitted(&self) -> usize {
        self.requests.borrow().len()
    }
    pub fn completed(&self) -> usize {
        self.completions.get()
    }
    pub fn release(&self, index: usize) {
        let request = self.requests.borrow_mut()[index].take().unwrap();
        request();
    }
    fn result<T: 'static>(
        &self,
        value: io::Result<T>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<T>>>> {
        if !self.held.get() {
            return Box::pin(std::future::ready(value));
        }
        let state = std::rc::Rc::new(RefCell::new(PendingState {
            value: None,
            waker: None,
        }));
        let shared = state.clone();
        let completions = self.completions.clone();
        self.requests.borrow_mut().push(Some(Box::new(move || {
            completions.set(completions.get() + 1);
            let waker = {
                let mut state = shared.borrow_mut();
                state.value = Some(value);
                state.waker.take()
            };
            if let Some(waker) = waker {
                waker.wake();
            }
        })));
        Box::pin(PendingResult { state })
    }
    pub fn entries(&self, path: &Path, entries: Vec<Dirent>) {
        self.listings.borrow_mut().insert(path.to_owned(), entries);
    }
    fn kinds(&self, path: &Path) -> io::Result<Vec<Dirent>> {
        if path.as_os_str().is_empty() {
            return Err(empty_path_error());
        }
        if let Some(entries) = self.listings.borrow().get(path) {
            return Ok(entries.clone());
        }
        self.read_dir(path)?
            .into_iter()
            .map(|name| {
                let kind = match self.nodes.borrow().get(&path.join(&name)) {
                    Some(Node::Directory) => DirentKind::Directory,
                    _ => DirentKind::File,
                };
                Ok(Dirent { name, kind })
            })
            .collect()
    }
    pub fn fail_after(&self, count: usize) {
        self.partial.set(Some(count));
    }
    fn store(&self, path: &Path, bytes: &[u8], append: bool) -> io::Result<()> {
        if path.as_os_str().is_empty() {
            return Err(empty_path_error());
        }
        let mut nodes = self.nodes.borrow_mut();
        if !matches!(
            path.parent().and_then(|p| nodes.get(p)),
            Some(Node::Directory)
        ) {
            return Err(io::Error::from(io::ErrorKind::NotADirectory));
        }
        if matches!(nodes.get(path), Some(Node::Directory)) {
            return Err(io::Error::from(io::ErrorKind::IsADirectory));
        }
        let previous = if append {
            match nodes.get(path) {
                Some(Node::File(v)) => v.clone(),
                _ => vec![],
            }
        } else {
            vec![]
        };
        let mut result = previous;
        let partial = self.partial.take();
        result.extend_from_slice(&bytes[..partial.unwrap_or(bytes.len()).min(bytes.len())]);
        nodes.insert(path.to_owned(), Node::File(result));
        if partial.is_some() {
            Err(io::Error::from_raw_os_error(28))
        } else {
            Ok(())
        }
    }
}
fn empty_path_error() -> io::Error {
    #[cfg(unix)]
    {
        io::Error::from_raw_os_error(2)
    }
    #[cfg(not(unix))]
    {
        io::ErrorKind::NotFound.into()
    }
}

impl Storage for Controlled {
    fn exists(&self, path: &Path) -> bool {
        if path.as_os_str().is_empty() {
            return false;
        }
        let links = self.links.borrow();
        let target = links.get(path).map_or(path, |path| path.as_path());
        self.nodes.borrow().contains_key(target)
    }
    fn mkdir(&self, path: &Path) -> io::Result<()> {
        if path.as_os_str().is_empty() {
            return Err(empty_path_error());
        }
        match self.nodes.borrow().get(path) {
            Some(Node::Directory) => return Ok(()),
            Some(Node::File(_)) => return Err(io::ErrorKind::NotADirectory.into()),
            None => {}
        }
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            self.mkdir(parent)?;
        }
        self.nodes
            .borrow_mut()
            .insert(path.to_owned(), Node::Directory);
        Ok(())
    }
    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>> {
        if path.as_os_str().is_empty() {
            return Err(empty_path_error());
        }
        match self.nodes.borrow().get(path) {
            Some(Node::File(v)) => Ok(v.clone()),
            Some(Node::Directory) => Err(io::ErrorKind::IsADirectory.into()),
            None => Err(io::ErrorKind::NotFound.into()),
        }
    }
    fn read_dir(&self, path: &Path) -> io::Result<Vec<std::ffi::OsString>> {
        if path.as_os_str().is_empty() {
            return Err(empty_path_error());
        }
        if let Some(entries) = self.listings.borrow().get(path) {
            return Ok(entries.iter().map(|entry| entry.name.clone()).collect());
        }
        let nodes = self.nodes.borrow();
        match nodes.get(path) {
            Some(Node::Directory) => Ok(nodes
                .keys()
                .filter(|p| p.parent() == Some(path))
                .map(|p| p.file_name().unwrap().to_owned())
                .collect()),
            Some(_) => Err(io::ErrorKind::NotADirectory.into()),
            None => Err(io::ErrorKind::NotFound.into()),
        }
    }
    fn modified(&self, path: &Path) -> io::Result<std::time::SystemTime> {
        if path.as_os_str().is_empty() {
            return Err(empty_path_error());
        }
        if self.exists(path) {
            Ok(std::time::UNIX_EPOCH + std::time::Duration::from_secs(123))
        } else {
            Err(io::ErrorKind::NotFound.into())
        }
    }
    fn read_prefix(&self, path: &Path, length: usize) -> io::Result<Vec<u8>> {
        let mut bytes = self.read_file(path)?;
        bytes.truncate(length);
        Ok(bytes)
    }
    fn write_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.store(path, bytes, false)
    }
    fn append_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.store(path, bytes, true)
    }
    fn read_file_async(
        &self,
        path: &Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<Vec<u8>>> + 'static>> {
        self.result(self.read_file(path))
    }
    fn modified_async(
        &self,
        path: &Path,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = io::Result<std::time::SystemTime>> + 'static>,
    > {
        self.result(self.modified(path))
    }
    fn read_dir_async(
        &self,
        path: &Path,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = io::Result<Vec<std::ffi::OsString>>> + 'static>,
    > {
        self.result(self.read_dir(path))
    }

    fn read_dir_with_file_types_async(
        &self,
        path: &Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<Vec<Dirent>>> + 'static>>
    {
        self.result(self.kinds(path))
    }
}

pub struct Fixture {
    pub root: PathBuf,
}
impl Fixture {
    pub fn new(label: &str, relative: bool) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let name = format!("maestro-bytes-{}-{label}-{id}", std::process::id());
        let root = if relative {
            PathBuf::from(name)
        } else {
            std::env::temp_dir().join(name)
        };
        #[cfg(not(target_arch = "wasm32"))]
        std::fs::create_dir(&root).unwrap();
        Self { root }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

pub fn both(label: &str, assertion: fn(&dyn Storage, &Path)) {
    for relative in [false, true] {
        let fixture = Fixture::new(label, relative);
        assertion(&Controlled::new(&fixture.root), &fixture.root);
        #[cfg(not(target_arch = "wasm32"))]
        assertion(&maestro_storage::FileStorage, &fixture.root);
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct ThreadWake(std::thread::Thread);
#[cfg(not(target_arch = "wasm32"))]
impl std::task::Wake for ThreadWake {
    fn wake(self: std::sync::Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &std::sync::Arc<Self>) {
        self.0.unpark();
    }
}

pub fn ready<T>(mut future: std::pin::Pin<Box<dyn std::future::Future<Output = T>>>) -> T {
    #[cfg(not(target_arch = "wasm32"))]
    let waker = std::task::Waker::from(std::sync::Arc::new(ThreadWake(std::thread::current())));
    #[cfg(target_arch = "wasm32")]
    let waker = std::task::Waker::noop().clone();
    let mut cx = std::task::Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(value) => return value,
            std::task::Poll::Pending => {
                #[cfg(not(target_arch = "wasm32"))]
                std::thread::park();
                #[cfg(target_arch = "wasm32")]
                panic!("controlled immediate result remained pending");
            }
        }
    }
}

struct PendingState<T> {
    value: Option<io::Result<T>>,
    waker: Option<std::task::Waker>,
}
struct PendingResult<T> {
    state: std::rc::Rc<RefCell<PendingState<T>>>,
}
impl<T> std::future::Future for PendingResult<T> {
    type Output = io::Result<T>;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let waker = cx.waker().clone();
        let (value, old) = {
            let mut state = self.state.borrow_mut();
            let value = state.value.take();
            let old = if value.is_none() {
                state.waker.replace(waker)
            } else {
                state.waker.take()
            };
            (value, old)
        };
        drop(old);
        match value {
            Some(value) => std::task::Poll::Ready(value),
            None => std::task::Poll::Pending,
        }
    }
}

pub const GUIDE: &str = r###"# Transcript byte access

`Storage` provides raw byte access at explicit caller-supplied paths. The session owner selects paths and owns history, formats and persistence timing. Storage does not parse JSON, decode UTF-8, frame lines, deduplicate bytes or normalize paths.

The eight synchronous operations are `exists`, `mkdir`, `read_file`, `read_prefix`, `read_dir`, `modified`, `append_file` and `write_file`. Existence follows targets and failed observations return false. Only mkdir recursively creates directories. Reads create nothing; append and write create files only when their parents exist. Empty appends still create files. An empty path returns `NotFound` for every I/O operation, synchronous or asynchronous, and false for existence; it never denotes the current directory.

Prefix reads open even for a zero-byte request. A nonzero request performs one bounded positional read at offset zero and returns only the bytes actually read, including incomplete UTF-8. A failed positional read intentionally retains the opened descriptor; successful reads and failed opens do not leak descriptors.

Append writes the supplied bytes in call order without framing. Write truncates the existing file in place, removing its old suffix. Both follow symbolic links, retain ordinary permissions and preserve the target identity rather than replacing it by rename. I/O errors retain native error data and may follow partial changes; they do not poison later operations. No transaction, flush guarantee, retry policy, byte cap or timeout is added.

Name-only directory listing returns every name without fetching member metadata. Modification time is a separate selected-target observation. The asynchronous kind listing returns owned `Dirent` values with File, Directory, SymbolicLink or Other classifications of the entries themselves, including dangling links. Native Unix listings use unsigned filename-byte order; controlled adapters retain their configured order. Windows enumeration equivalence still requires runtime qualification before release.

The four asynchronous operations are `read_file_async`, `modified_async`, `read_dir_async` and `read_dir_with_file_types_async`. Each admits owned work immediately, independent of polling, and publishes its own raw result on completion. Dropping the future abandons observation, not admitted work. No runtime-library type, aggregate wait, progress callback or application sorting policy is exposed.

`FileStorage` is a native unit adapter with one private process-shared pool of four filesystem workers. The fixed four-worker pool is the selected Rust library equivalent; it does not read another runtime's pool-size environment knob or promise identical scheduling. Queueing is uncapped, filesystem work runs outside the caller thread and future polling performs no I/O. Thread-creation failure returns an I/O error; a later admission can try initialization again.

The object-safe `Storage` interface, directory records and reusable `conformance` assertions are available to browser builds without requiring Send or Sync. `FileStorage` is native-only. A browser build verifies interface and controlled-adapter portability, not native filesystem support. The conformance assertions use supplied fresh roots and one unchanged caller against independent adapters.

This isolated native example writes and appends opaque bytes without selecting a session or configuration directory:

```rust
#[cfg(not(target_arch = "wasm32"))]
{
    use maestro_storage::{FileStorage, Storage};
    let root = std::env::temp_dir().join(format!("maestro-byte-example-{}", std::process::id()));
    std::fs::create_dir(&root)?;
    let storage = FileStorage;
    let directory = root.join("nested");
    storage.mkdir(&directory)?;
    let file = directory.join("bytes");
    storage.write_file(&file, b"\0\xff")?;
    storage.append_file(&file, b"\r\n")?;
    assert_eq!(storage.read_file(&file)?, b"\0\xff\r\n");
    std::fs::remove_dir_all(root)?;
}
# Ok::<(), std::io::Error>(())
```
"###;
pub const POINTER: &str = "# Storage\n\nSee [Transcript byte access](transcript.md) for the authoritative supplied-path byte interface. The session owner retains history and persistence policy.\n";

#[cfg(target_os = "linux")]
pub fn isolated_child(flag: &str, name: &str) -> bool {
    if std::env::var_os(flag).is_some() {
        return false;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env(flag, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

#[cfg(target_os = "linux")]
pub struct Release(pub Option<std::fs::File>);
#[cfg(target_os = "linux")]
impl Release {
    pub fn release(&mut self) {
        if let Some(mut file) = self.0.take() {
            std::io::Write::write_all(&mut file, b"released").unwrap();
        }
    }
}
#[cfg(target_os = "linux")]
impl Drop for Release {
    fn drop(&mut self) {
        self.release();
    }
}
#[cfg(target_os = "linux")]
pub struct Notify {
    pub wakes: std::sync::atomic::AtomicUsize,
    pub tx: std::sync::mpsc::Sender<()>,
}
#[cfg(target_os = "linux")]
impl std::task::Wake for Notify {
    fn wake(self: std::sync::Arc<Self>) {
        self.wakes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.tx.send(()).unwrap();
    }
}
