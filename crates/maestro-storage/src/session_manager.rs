use crate::{Dirent, DirentKind, Storage};
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

/// Native supplied-path byte access.
pub struct FileStorage;

impl Storage for FileStorage {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
    fn mkdir(&self, path: &Path) -> io::Result<()> {
        if path.as_os_str().is_empty() {
            return Err(io::ErrorKind::NotFound.into());
        }
        fs::create_dir_all(path)
    }
    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(path)
    }
    fn read_prefix(&self, path: &Path, length: usize) -> io::Result<Vec<u8>> {
        let file = fs::File::open(path)?;
        let mut bytes = vec![0; length];
        if length == 0 {
            return Ok(bytes);
        }
        #[cfg(unix)]
        let result = std::os::unix::fs::FileExt::read_at(&file, &mut bytes, 0);
        #[cfg(windows)]
        let result = std::os::windows::fs::FileExt::seek_read(&file, &mut bytes, 0);
        match result {
            Ok(count) => {
                bytes.truncate(count);
                Ok(bytes)
            }
            Err(error) => {
                // A failed positional read retains the successfully opened descriptor.
                std::mem::forget(file);
                Err(error)
            }
        }
    }
    fn read_dir(&self, path: &Path) -> io::Result<Vec<std::ffi::OsString>> {
        let names = fs::read_dir(path)?
            .map(|entry| entry.map(|e| e.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        #[cfg(unix)]
        let names = {
            use std::os::unix::ffi::OsStrExt;
            let mut names = names;
            names.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            names
        };
        Ok(names)
    }
    fn modified(&self, path: &Path) -> io::Result<std::time::SystemTime> {
        fs::metadata(path)?.modified()
    }
    fn append_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?
            .write_all(bytes)
    }
    fn write_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        fs::write(path, bytes)
    }
    fn read_file_async(
        &self,
        path: &Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<Vec<u8>>> + 'static>> {
        let path = path.to_owned();
        admit(move || fs::read(path))
    }
    fn modified_async(
        &self,
        path: &Path,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = io::Result<std::time::SystemTime>> + 'static>,
    > {
        let path = path.to_owned();
        admit(move || FileStorage.modified(&path))
    }
    fn read_dir_async(
        &self,
        path: &Path,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = io::Result<Vec<std::ffi::OsString>>> + 'static>,
    > {
        let path = path.to_owned();
        admit(move || FileStorage.read_dir(&path))
    }
    fn read_dir_with_file_types_async(
        &self,
        path: &Path,
    ) -> Pin<Box<dyn Future<Output = io::Result<Vec<Dirent>>> + 'static>> {
        let path = path.to_owned();
        admit(move || {
            let entries = fs::read_dir(path)?
                .map(|entry| {
                    let entry = entry?;
                    let kind = entry.file_type()?;
                    let kind = if kind.is_file() {
                        DirentKind::File
                    } else if kind.is_dir() {
                        DirentKind::Directory
                    } else if kind.is_symlink() {
                        DirentKind::SymbolicLink
                    } else {
                        DirentKind::Other
                    };
                    Ok(Dirent {
                        name: entry.file_name(),
                        kind,
                    })
                })
                .collect::<io::Result<Vec<_>>>()?;
            #[cfg(unix)]
            let entries = {
                use std::os::unix::ffi::OsStrExt;
                let mut entries = entries;
                entries.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
                entries
            };
            Ok(entries)
        })
    }
}

use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::{Arc, Condvar, Mutex, OnceLock},
    task::{Context, Poll, Waker},
    thread,
};

type Job = Box<dyn FnOnce() + Send>;
#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    stopped: bool,
}
struct WorkQueue {
    state: Mutex<Queue>,
    available: Condvar,
}
struct Pool {
    queue: Arc<WorkQueue>,
    _workers: Vec<thread::JoinHandle<()>>,
}

impl Pool {
    fn new() -> io::Result<Self> {
        let queue = Arc::new(WorkQueue {
            state: Mutex::new(Queue::default()),
            available: Condvar::new(),
        });
        let mut workers = Vec::new();
        for index in 0..4 {
            let shared = Arc::clone(&queue);
            match thread::Builder::new()
                .name(format!("maestro-files-{index}"))
                .spawn(move || {
                    loop {
                        let job = {
                            let mut state = shared.state.lock().unwrap();
                            while state.jobs.is_empty() && !state.stopped {
                                state = shared.available.wait(state).unwrap();
                            }
                            if state.stopped {
                                return;
                            }
                            state.jobs.pop_front().unwrap()
                        };
                        job();
                    }
                }) {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    queue.state.lock().unwrap().stopped = true;
                    queue.available.notify_all();
                    for worker in workers {
                        let _ = worker.join();
                    }
                    return Err(error);
                }
            }
        }
        Ok(Self {
            queue,
            _workers: workers,
        })
    }
    fn submit(&self, job: Job) {
        self.queue.state.lock().unwrap().jobs.push_back(job);
        self.queue.available.notify_one();
    }
}

struct ResultState<T> {
    result: Option<io::Result<T>>,
    waker: Option<Waker>,
}
struct FileResult<T> {
    state: Arc<Mutex<ResultState<T>>>,
}
impl<T> Future for FileResult<T> {
    type Output = io::Result<T>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Clone and drop caller-owned wakers outside the result lock.
        let waker = cx.waker().clone();
        let (result, previous) = {
            let mut state = self.state.lock().unwrap();
            let result = state.result.take();
            let previous = if result.is_none() {
                state.waker.replace(waker)
            } else {
                state.waker.take()
            };
            (result, previous)
        };
        drop(previous);
        match result {
            Some(result) => Poll::Ready(result),
            None => Poll::Pending,
        }
    }
}
fn admit<T: Send + 'static>(
    operation: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> Pin<Box<dyn Future<Output = io::Result<T>>>> {
    static POOL: OnceLock<Mutex<Option<Pool>>> = OnceLock::new();
    let state = Arc::new(Mutex::new(ResultState {
        result: None,
        waker: None,
    }));
    let mut pool = POOL.get_or_init(|| Mutex::new(None)).lock().unwrap();
    if pool.is_none() {
        match Pool::new() {
            Ok(new) => *pool = Some(new),
            Err(error) => return Box::pin(std::future::ready(Err(error))),
        }
    }
    let shared = Arc::clone(&state);
    pool.as_ref().unwrap().submit(Box::new(move || {
        let result = operation();
        let waker = {
            let mut state = shared.lock().unwrap();
            state.result = Some(result);
            state.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }));
    Box::pin(FileResult { state })
}
