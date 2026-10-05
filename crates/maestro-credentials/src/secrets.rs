//! Replaceable lazy secret access with owned native process work.
use crate::{CredentialError, worker::Work};
use maestro_models::{Cancellation, SecretString};
use std::{
    future::Future,
    io::Read,
    path::{Path, PathBuf},
    pin::Pin,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
    thread::JoinHandle,
    time::{Duration, Instant},
};

/// Secret access separate from metadata and credential policy.
pub trait SecretResolver: Send + Sync {
    /// Read a named nonempty environment value with no literal fallback.
    fn environment(&self, name: &str) -> Option<SecretString>;
    /// Lazily resolve leading !helper, exact environment name, then literal.
    /// Creation/polling must not block on process or storage work.
    fn resolve(
        &self,
        value: SecretString,
        cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<Option<SecretString>, CredentialError>> + Send + '_>>;
}

/// Native secret access bound to one explicitly supplied helper directory.
/// Helpers capture output, discard diagnostics and retain cleanup ownership.
pub struct NativeSecretResolver {
    working_directory: PathBuf,
    runtime: Arc<dyn HelperRuntime>,
}
impl NativeSecretResolver {
    /// Bind an absolute directory without I/O, helpers or ambient discovery.
    pub fn new(working_directory: PathBuf) -> Result<Self, CredentialError> {
        if !working_directory.is_absolute() {
            return Err(CredentialError::InvalidPath);
        }
        Ok(Self {
            working_directory,
            runtime: Arc::new(NativeRuntime {
                origin: Instant::now(),
            }),
        })
    }
}
impl SecretResolver for NativeSecretResolver {
    fn environment(&self, name: &str) -> Option<SecretString> {
        self.runtime
            .environment(name)
            .filter(|value| !value.is_empty())
            .map(SecretString::new)
    }
    fn resolve(
        &self,
        value: SecretString,
        cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<Option<SecretString>, CredentialError>> + Send + '_>>
    {
        Box::pin(async move {
            if cancellation.is_cancelled() {
                return Err(CredentialError::Cancelled);
            }
            if let Some(command) = value.expose().strip_prefix('!') {
                let key = (self.working_directory.clone(), value.expose().to_owned());
                let work = {
                    let mut cache = cache().lock().unwrap_or_else(|p| p.into_inner());
                    cache
                        .entry(key)
                        .or_insert_with(|| {
                            let runtime = self.runtime.clone();
                            let directory = self.working_directory.clone();
                            let command = command.to_owned();
                            Work::start(move || Ok(helper(runtime.as_ref(), &command, &directory)))
                        })
                        .clone()
                };
                work.wait(cancellation).await
            } else {
                Ok(self
                    .environment(value.expose())
                    .or_else(|| (!value.expose().is_empty()).then_some(value)))
            }
        })
    }
}
type HelperCache = std::collections::BTreeMap<(PathBuf, String), Arc<Work<Option<SecretString>>>>;
fn cache() -> &'static Mutex<HelperCache> {
    static CACHE: OnceLock<Mutex<HelperCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HelperCache::new()))
}
/// Clear process-wide helper successes and unresolved failures explicitly.
/// In-flight waiters retain their work, but old completions cannot refill the cleared cache.
/// This does not cancel processes, revoke secrets or undo helper effects.
pub fn reset_secret_helper_cache() {
    // Only admission inserts entries; completion never writes into a later generation.
    let retired = std::mem::take(&mut *cache().lock().unwrap_or_else(|p| p.into_inner()));
    drop(retired);
}
trait HelperProcess: Send {
    fn status(&mut self) -> Result<Option<bool>, ()>;
    fn finish(self: Box<Self>, kill: bool) -> Option<Vec<u8>>;
}
trait HelperRuntime: Send + Sync {
    fn environment(&self, name: &str) -> Option<String>;
    fn spawn(&self, command: &str, directory: &Path) -> Result<Box<dyn HelperProcess>, ()>;
    fn now(&self) -> Duration;
    fn wait(&self, duration: Duration);
}
fn helper(runtime: &dyn HelperRuntime, command: &str, directory: &Path) -> Option<SecretString> {
    let started = runtime.now();
    let mut process = runtime.spawn(command, directory).ok()?;
    loop {
        let elapsed = runtime.now().saturating_sub(started);
        if elapsed >= Duration::from_millis(10_000) {
            process.finish(true);
            return None;
        }
        match process.status() {
            Ok(Some(success)) => {
                let output = process.finish(false)?;
                if !success {
                    return None;
                }
                let output = String::from_utf8(output).ok()?;
                let output = output.trim();
                return (!output.is_empty()).then(|| SecretString::new(output.into()));
            }
            Err(()) => {
                process.finish(true);
                return None;
            }
            Ok(None) => {
                runtime.wait(Duration::from_millis(20).min(Duration::from_millis(10_000) - elapsed))
            }
        }
    }
}
struct NativeRuntime {
    origin: Instant,
}
impl HelperRuntime for NativeRuntime {
    fn environment(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
    fn spawn(&self, command: &str, directory: &Path) -> Result<Box<dyn HelperProcess>, ()> {
        #[cfg(not(windows))]
        let mut shell = {
            let mut command = Command::new("sh");
            command.arg("-c");
            command
        };
        #[cfg(windows)]
        let mut shell = {
            let mut command = Command::new("cmd");
            command.arg("/C");
            command
        };
        let mut child = shell
            .arg(command)
            .current_dir(directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| ())?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let mut process = NativeProcess {
            child: Some(child),
            stdout: None,
            stderr: None,
        };
        let mut stdout = stdout.ok_or(())?;
        let mut stderr = stderr.ok_or(())?;
        process.stdout = Some(
            std::thread::Builder::new()
                .spawn(move || {
                    let mut bytes = vec![];
                    stdout.read_to_end(&mut bytes).map(|_| bytes)
                })
                .map_err(|_| ())?,
        );
        process.stderr = Some(
            std::thread::Builder::new()
                .spawn(move || std::io::copy(&mut stderr, &mut std::io::sink()))
                .map_err(|_| ())?,
        );
        Ok(Box::new(process))
    }
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
    fn wait(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}
struct NativeProcess {
    child: Option<Child>,
    stdout: Option<JoinHandle<std::io::Result<Vec<u8>>>>,
    stderr: Option<JoinHandle<std::io::Result<u64>>>,
}
impl NativeProcess {
    fn settle(&mut self, kill: bool) -> Option<Vec<u8>> {
        let reaped = if let Some(mut child) = self.child.take() {
            if kill {
                let _ = child.kill();
            }
            child.wait().is_ok()
        } else {
            true
        };
        let output = self
            .stdout
            .take()
            .and_then(|reader| reader.join().ok())
            .and_then(Result::ok);
        let drained = self
            .stderr
            .take()
            .is_none_or(|reader| matches!(reader.join(), Ok(Ok(_))));
        if reaped && drained { output } else { None }
    }
}
impl HelperProcess for NativeProcess {
    fn status(&mut self) -> Result<Option<bool>, ()> {
        self.child
            .as_mut()
            .ok_or(())?
            .try_wait()
            .map(|status| status.map(|status| status.success()))
            .map_err(|_| ())
    }
    fn finish(mut self: Box<Self>, kill: bool) -> Option<Vec<u8>> {
        self.settle(kill)
    }
}
impl Drop for NativeProcess {
    fn drop(&mut self) {
        self.settle(true);
    }
}

#[cfg(test)]
mod tests;
