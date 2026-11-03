//! Native I/O adapters retain their domain owners on the application's local executor.
use maestro_extensions::native_io::{
    ChildProcess, ExitStatus, IoFuture, NativeIo, Readable, Request, Response, SpawnOptions, Stdio,
    Writable,
};
/// Native paths, permissions and byte publication policy.
mod files;
/// HTTP response and body resource ownership.
mod http;
use http::Http;
use maestro_models::providers::http::{Raced, race};
use maestro_process::child_process::{RawChild, RawInput, RawOutput};
use std::{
    process::Stdio as NativeStdio,
    rc::{Rc, Weak},
};
use tokio::task::LocalSet;

/// Supplies native operations without retaining the task set from one of its own tasks.
#[must_use]
pub fn extension_native_io(local: &Rc<LocalSet>) -> Rc<dyn NativeIo> {
    Rc::new(Operations {
        local: Rc::downgrade(local),
    })
}
/// Application composition of existing native effect owners.
struct Operations {
    /// Scheduling belongs to the application, not to the runtime adapter.
    local: Weak<LocalSet>,
}
impl NativeIo for Operations {
    fn save_file(&self, path: String, bytes: Vec<u8>, mode: u32) -> IoFuture<'_, Option<String>> {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || files::save_file(&path, &bytes, mode))
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())
        })
    }
    fn file_mode(&self, path: &str) -> Result<u32, String> {
        files::file_mode(path).map_err(|error| error.to_string())
    }
    fn set_file_mode(&self, path: &str, mode: u32) -> Result<(), String> {
        files::set_file_mode(path, mode).map_err(|error| error.to_string())
    }
    fn create_dir(&self, path: &str, mode: u32) -> Result<(), String> {
        files::create_dir(path, mode).map_err(|error| error.to_string())
    }

    fn watch(
        &self,
        path: String,
        listener: Rc<dyn Fn()>,
    ) -> Result<Rc<dyn maestro_extensions::native_io::FsWatcher>, String> {
        use maestro_watch::fs_watch::WatchOperations;
        let local = self.local.upgrade().ok_or("native executor ended")?;
        let watch = maestro_watch::fs_watch::NativeWatchOperations::new(local)
            .watch(&path, Rc::new(move |_| listener()), Rc::new(|| {}))
            .map_err(|error| error.to_string())?;
        Ok(Rc::new(Watch(std::cell::RefCell::new(watch))))
    }
    fn fetch(&self, request: Request) -> IoFuture<'_, Rc<dyn Response>> {
        Box::pin(async move {
            let signal = request.signal;
            if signal
                .as_ref()
                .is_some_and(maestro_models::Cancellation::is_aborted)
            {
                return Err(maestro_models::FetchError::Aborted.to_string());
            }
            let response = maestro_models::fetch_optional_body(maestro_models::HttpRequest {
                url: request.url,
                method: request.method.unwrap_or_else(|| "GET".into()),
                headers: request.headers.into_iter().collect(),
                body: request.body,
                signal: signal.clone(),
            });
            let response = match race(response, None, signal.as_ref()).await {
                Raced::Done(result) => result.map_err(|e| e.to_string())?,
                Raced::Cancelled | Raced::TimedOut => {
                    return Err(maestro_models::FetchError::Aborted.to_string());
                }
            };
            Ok(Rc::new(Http::new(response, signal)) as Rc<dyn Response>)
        })
    }
    fn spawn(
        &self,
        command: String,
        args: Vec<String>,
        options: SpawnOptions,
    ) -> IoFuture<'_, Rc<dyn ChildProcess>> {
        Box::pin(async move {
            let local = self.local.upgrade().ok_or("native executor ended")?;
            let mut command = maestro_process::command(command);
            command.args(args);
            if let Some(cwd) = options.cwd {
                command.current_dir(cwd);
            }
            if let Some(env) = options.env {
                command.env_clear().envs(env);
            }
            let (child, collect) =
                RawChild::spawn(command, options.stdio.map(descriptor), options.detached)
                    .map_err(|e| e.to_string())?;
            drop(local.spawn_local(collect));
            Ok(Rc::new(Child::new(child)) as Rc<dyn ChildProcess>)
        })
    }
    fn kill(&self, pid: i32, signal: Option<String>) -> Result<(), String> {
        maestro_process::child_process::signal_pid(pid, signal.as_deref())
            .map_err(|e| e.to_string())
    }
}
/// Projects one descriptor selection without supplying a fallback pipe.
fn descriptor(value: Stdio) -> NativeStdio {
    match value {
        Stdio::Pipe => NativeStdio::piped(),
        Stdio::Ignore => NativeStdio::null(),
        Stdio::Inherit => NativeStdio::inherit(),
    }
}
/// A native child and stable trait handles for its selected descriptors.
struct Child {
    /// Native lifecycle, byte queues and waiter observations.
    raw: RawChild,
    /// Input aliases share the same native writer.
    stdin: Option<Rc<dyn Writable>>,
    /// Output aliases share the same native consumer.
    stdout: Option<Rc<dyn Readable>>,
    /// Error aliases share their own native consumer.
    stderr: Option<Rc<dyn Readable>>,
}
impl Child {
    /// Retains each original descriptor without cloning its consuming state.
    fn new(child: RawChild) -> Self {
        Self {
            stdin: child
                .stdin
                .clone()
                .map(|pipe| Rc::new(Input(pipe)) as Rc<dyn Writable>),
            stdout: child
                .stdout
                .clone()
                .map(|pipe| Rc::new(Output(pipe)) as Rc<dyn Readable>),
            stderr: child
                .stderr
                .clone()
                .map(|pipe| Rc::new(Output(pipe)) as Rc<dyn Readable>),
            raw: child,
        }
    }
}
impl ChildProcess for Child {
    fn pid(&self) -> Option<u32> {
        self.raw.pid()
    }
    fn stdin(&self) -> Option<Rc<dyn Writable>> {
        self.stdin.clone()
    }
    fn stdout(&self) -> Option<Rc<dyn Readable>> {
        self.stdout.clone()
    }
    fn stderr(&self) -> Option<Rc<dyn Readable>> {
        self.stderr.clone()
    }
    fn wait(&self) -> IoFuture<'_, ExitStatus> {
        Box::pin(async move {
            let status = self.raw.wait().await.map_err(|e| e.to_string())?;
            Ok(ExitStatus {
                code: status.code(),
                signal: status.signal().map(str::to_owned),
            })
        })
    }
    fn kill(&self, signal: Option<String>) -> Result<bool, String> {
        self.raw.kill(signal.as_deref()).map_err(|e| e.to_string())
    }
}
/// Trait projection of the process owner's consuming output.
struct Output(RawOutput);
impl Readable for Output {
    fn read(&self) -> IoFuture<'_, Option<Vec<u8>>> {
        Box::pin(async move { self.0.read().await.map_err(|e| e.to_string()) })
    }
    fn close(&self) -> Result<(), String> {
        self.0.close();
        Ok(())
    }
}
/// Trait projection of the process owner's ordered input.
struct Input(RawInput);
impl Writable for Input {
    fn write<'a>(&'a self, bytes: &'a [u8]) -> IoFuture<'a, ()> {
        Box::pin(async move { self.0.write(bytes).await.map_err(|e| e.to_string()) })
    }
    fn end(&self) -> IoFuture<'_, ()> {
        Box::pin(async move { self.0.end().await.map_err(|e| e.to_string()) })
    }
}

/// Projection of the watch owner's existing close and last-drop behavior.
struct Watch(std::cell::RefCell<Box<dyn maestro_watch::fs_watch::FsWatcher>>);
impl maestro_extensions::native_io::FsWatcher for Watch {
    fn close(&self) -> Result<(), String> {
        self.0
            .borrow_mut()
            .close()
            .map_err(|error| error.to_string())
    }
}
