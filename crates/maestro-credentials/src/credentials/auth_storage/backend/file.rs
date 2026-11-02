//! Native credential file guarded by a sidecar lock file.
use super::{AsyncLockUpdate, AuthStorageBackend, AuthStorageError, AuthStorageFuture, LockUpdate};
use maestro_lock::{CreationMode, WriteLock};
use std::fs::{self, OpenOptions, TryLockError};
use std::io::{self, ErrorKind};
use std::path::Path;
use std::time::Duration;

/// Number of waits before scheduled acquisition gives up on contention.
const LOCK_WAITS: u32 = 10;
/// Initial exponential wait in milliseconds, before random lengthening.
const LOCK_MIN_MS: f64 = 100.0;
/// Maximum wait in milliseconds, applied after random lengthening and rounding.
const LOCK_MAX_MS: f64 = 10_000.0;

/// Credential file at a caller-resolved path; relative paths use the working directory.
#[derive(Debug)]
pub struct FileAuthStorageBackend {
    /// Credential file location.
    auth_path: String,
    /// Fresh draw in `[0, 1)` for each contended wait.
    draw: fn() -> f64,
    /// Blocking wait used by synchronous rereads.
    wait: fn(Duration),
}

impl FileAuthStorageBackend {
    /// Address the credential file; nothing is created until first use.
    #[must_use]
    pub fn new(auth_path: &str) -> Self {
        Self {
            auth_path: auth_path.to_owned(),
            draw: maestro_models::unit_random,
            wait: std::thread::sleep,
        }
    }

    /// Replace random draws for deterministic acquisition tests.
    #[cfg(test)]
    pub(in crate::credentials::auth_storage) fn with_draw(mut self, draw: fn() -> f64) -> Self {
        self.draw = draw;
        self
    }

    /// Replace blocking waits for controlled competing writers.
    #[cfg(test)]
    pub(in crate::credentials::auth_storage) fn with_wait(mut self, wait: fn(Duration)) -> Self {
        self.wait = wait;
        self
    }

    /// Create missing credential parent directories privately.
    fn create_parents(&self) -> io::Result<()> {
        if let Some(parent) = Path::new(&self.auth_path).parent() {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
            builder.create(parent)?;
        }
        Ok(())
    }

    /// Lock the sidecar, retrying only while another holder has it.
    fn acquire(&self) -> Result<WriteLock, AuthStorageError> {
        self.create_parents()?;
        maestro_lock::acquire_write_bounded(Path::new(&self.auth_path), true).map_err(lock_error)
    }

    /// Waits shared by blocking rereads and asynchronous locked refreshes.
    fn scheduled_delays(&self) -> impl Iterator<Item = Duration> + '_ {
        (0..LOCK_WAITS).map(|attempt| {
            let millis = ((1.0 + (self.draw)()) * LOCK_MIN_MS * f64::from(1_u32 << attempt))
                .round()
                .min(LOCK_MAX_MS);
            Duration::from_secs_f64(millis / 1000.0)
        })
    }

    /// Acquire once, returning the next contention delay or the terminal acquisition error.
    fn attempt(
        &self,
        delays: &mut impl Iterator<Item = Duration>,
    ) -> Result<Result<WriteLock, Duration>, AuthStorageError> {
        match maestro_lock::try_acquire_write(Path::new(&self.auth_path), true) {
            Ok(holder) => Ok(Ok(holder)),
            Err(error @ TryLockError::WouldBlock) => {
                delays.next().map(Err).ok_or_else(|| lock_error(error))
            }
            Err(error) => Err(lock_error(error)),
        }
    }

    /// Lock the sidecar, blocking for scheduled delays while another holder has it.
    fn acquire_waiting(&self) -> Result<WriteLock, AuthStorageError> {
        self.create_parents()?;
        let mut delays = self.scheduled_delays();
        loop {
            match self.attempt(&mut delays)? {
                Ok(holder) => return Ok(holder),
                Err(delay) => (self.wait)(delay),
            }
        }
    }

    /// Lock the sidecar, awaiting scheduled delays while another holder has it.
    async fn acquire_async(&self) -> Result<WriteLock, AuthStorageError> {
        self.create_parents()?;
        let mut delays = self.scheduled_delays();
        loop {
            match self.attempt(&mut delays)? {
                Ok(holder) => return Ok(holder),
                Err(delay) => tokio::time::sleep(delay).await,
            }
        }
    }

    /// Initialize a missing file (following a symlink), then read it, under the held lock.
    fn read(&self) -> io::Result<Option<String>> {
        let path = Path::new(&self.auth_path);
        initialize(path)?;
        match fs::read(path) {
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
}

/// Preserves native acquisition errors and distinguishes exhausted contention.
fn lock_error(error: TryLockError) -> AuthStorageError {
    match error {
        TryLockError::Error(error) => error.into(),
        error @ TryLockError::WouldBlock => error.into(),
    }
}

/// Create the file with `{}` only if absent, following dangling links; an existing file is never opened for writing.
fn initialize(path: &Path) -> io::Result<()> {
    let mut candidate = path.to_path_buf();
    loop {
        let mut open = OpenOptions::new();
        open.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut open, 0o600);
        match open.open(&candidate) {
            Ok(mut file) => {
                io::Write::write_all(&mut file, b"{}")?;
                #[cfg(unix)]
                file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
                return Ok(());
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                match fs::metadata(&candidate) {
                    Err(missing) if missing.kind() == ErrorKind::NotFound => {
                        let link = fs::read_link(&candidate)?;
                        candidate = candidate.parent().unwrap_or(Path::new("")).join(link);
                    }
                    Err(other) => return Err(other),
                    Ok(_) => return Ok(()),
                }
            }
            Err(error) => return Err(error),
        }
    }
}

/// Publishes `text` as the credential file at `path` through the shared recoverable writer,
/// readable only by its owner on Unix.
///
/// The text is written in place under the shared exclusion, so a hard link to the file sees it,
/// and a write cut short is completed, not rolled back, by the next coordinated writer. A symbolic
/// link at `path` is followed and a missing parent directory is created. An existing file must be
/// writable by its owner: it is narrowed to mode 0600 through the handle that proved this, so the
/// new text is never written under looser bits, and a read-only file is refused without a change.
/// A failed parent-directory sync after the write is not reported: the text is already written.
///
/// # Errors
/// Returns the native error of the narrowing or of the publication.
pub fn write_auth_file(path: &str, text: &str) -> io::Result<()> {
    #[cfg(unix)]
    narrow(Path::new(path))?;
    maestro_lock::replace(
        Path::new(path),
        text.as_bytes(),
        maestro_lock::Replace {
            through_symlinks: true,
            creation_mode: Some(CreationMode::AfterSync(0o600)),
        },
    )
    .map(|_| ())
}

/// Narrows an existing writable file before publishing any credential bytes.
#[cfg(unix)]
fn narrow(path: &Path) -> io::Result<()> {
    match OpenOptions::new().write(true).open(path) {
        Ok(file) => file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?,
        Err(error) if error.kind() != ErrorKind::NotFound => return Err(error),
        Err(_) => (),
    }
    Ok(())
}

/// Publishes on the transaction's retained lock without acquiring it again.
fn publish(holder: WriteLock, text: &str) -> io::Result<()> {
    #[cfg(unix)]
    narrow(holder.path())?;
    holder
        .replace(text.as_bytes(), Some(CreationMode::AfterSync(0o600)))
        .map(|_| ())
}

impl AuthStorageBackend for FileAuthStorageBackend {
    fn read_waiting(&self) -> Result<Option<String>, AuthStorageError> {
        let _lock = self.acquire_waiting()?;
        self.read().map_err(Into::into)
    }

    fn with_lock(
        &self,
        update: &mut dyn FnMut(Option<&str>) -> LockUpdate,
    ) -> Result<(), AuthStorageError> {
        let lock = self.acquire()?;
        let current = self.read()?;
        if let Some(next) = update(current.as_deref())? {
            publish(lock, &next)?;
        }
        Ok(())
    }

    fn with_lock_async<'a>(
        &'a self,
        update: AsyncLockUpdate<'a>,
    ) -> AuthStorageFuture<'a, Result<(), AuthStorageError>> {
        Box::pin(async move {
            let lock = self.acquire_async().await?;
            let current = self.read()?;
            if let Some(next) = update(current).await? {
                publish(lock, &next)?;
            }
            Ok(())
        })
    }
}
