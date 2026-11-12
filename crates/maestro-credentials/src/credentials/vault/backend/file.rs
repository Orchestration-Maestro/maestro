//! Native credential file guarded by a sidecar lock file.
use super::{AsyncLockUpdate, AuthStorageBackend, AuthStorageError, AuthStorageFuture, LockUpdate};
use maestro_lock::{CreationMode, WriteLock};
use std::fs::{self, OpenOptions, TryLockError};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

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
    /// The text last read under the lock, with the file identity it was read at.
    remembered: Mutex<Option<(FileIdentity, String)>>,
}

/// What an edited, replaced or restored credential file changes: its size, modification time
/// and, where the platform reports them, device, inode and change time. Maestro writes the
/// file in place, so the inode stays; the change time moves with every write, at the
/// filesystem's timestamp resolution, and cannot be set back, which separates an edit that
/// restores the old modification time. Writes inside one timestamp tick look the same, and
/// so do equal-size edits where the platform reports no change time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    /// Size in bytes.
    len: u64,
    /// Modification time at the platform's resolution.
    modified: SystemTime,
    /// Device and inode numbers, absent where the platform has none.
    node: Option<(u64, u64)>,
    /// Change time as seconds and nanoseconds, absent where the platform has none.
    changed: Option<(i64, i64)>,
}

impl FileIdentity {
    /// The identity of the file at `path`, following links; `None` when it cannot be read.
    fn of(path: &Path) -> Option<Self> {
        let metadata = fs::metadata(path).ok()?;
        #[cfg(unix)]
        let (node, changed) = {
            use std::os::unix::fs::MetadataExt;
            (
                Some((metadata.dev(), metadata.ino())),
                Some((metadata.ctime(), metadata.ctime_nsec())),
            )
        };
        #[cfg(not(unix))]
        let (node, changed) = (None, None);
        Some(Self {
            len: metadata.len(),
            modified: metadata.modified().ok()?,
            node,
            changed,
        })
    }
}

impl FileAuthStorageBackend {
    /// Address the credential file; nothing is created until first use.
    #[must_use]
    pub fn new(auth_path: &str) -> Self {
        Self {
            auth_path: auth_path.to_owned(),
            draw: maestro_models::unit_random,
            wait: std::thread::sleep,
            remembered: Mutex::new(None),
        }
    }

    /// Replace random draws for deterministic acquisition tests.
    #[cfg(test)]
    pub(in crate::credentials::vault) fn with_draw(mut self, draw: fn() -> f64) -> Self {
        self.draw = draw;
        self
    }

    /// Replace blocking waits for controlled competing writers.
    #[cfg(test)]
    pub(in crate::credentials::vault) fn with_wait(mut self, wait: fn(Duration)) -> Self {
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
    maestro_path::resolve_final_links(path, create_missing).map(|_| ())
}

/// Create `candidate` with `{}` when nothing is there, returning `None`. A dangling link yields
/// its target for the next candidate; an existing file ends the walk unchanged.
fn create_missing(candidate: &Path) -> io::Result<Option<PathBuf>> {
    let mut open = OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut open, 0o600);
    match open.open(candidate) {
        Ok(mut file) => {
            io::Write::write_all(&mut file, b"{}")?;
            #[cfg(unix)]
            file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
            Ok(None)
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => match fs::metadata(candidate) {
            Err(missing) if missing.kind() == ErrorKind::NotFound => {
                fs::read_link(candidate).map(Some)
            }
            Err(other) => Err(other),
            Ok(_) => Ok(None),
        },
        Err(error) => Err(error),
    }
}

/// Publishes on the transaction's retained lock without acquiring it again, narrowing an
/// existing looser file to its owner first so no credential byte is written under looser bits.
fn publish(holder: WriteLock, text: &str) -> io::Result<()> {
    #[cfg(unix)]
    maestro_lock::narrow_existing(holder.path(), 0o600)?;
    holder
        .replace(text.as_bytes(), Some(CreationMode::AfterSync(0o600)))
        .map(|_| ())
}

impl AuthStorageBackend for FileAuthStorageBackend {
    /// Returns the remembered text while the file keeps the identity it was read at, without
    /// locking; otherwise reads it again under the lock.
    fn read_waiting(&self) -> Result<Option<String>, AuthStorageError> {
        let path = Path::new(&self.auth_path);
        let unchanged = FileIdentity::of(path).and_then(|now| {
            let remembered = maestro_lock::lock(&self.remembered);
            let (seen, text) = remembered.as_ref()?;
            (*seen == now).then(|| text.clone())
        });
        if unchanged.is_some() {
            return Ok(unchanged);
        }
        let _lock = self.acquire_waiting()?;
        let identity = FileIdentity::of(path);
        let text = self.read()?;
        *maestro_lock::lock(&self.remembered) = identity.zip(text.clone());
        Ok(text)
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
