//! One synchronized append-only rotating file.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tracing_subscriber::fmt::writer::MakeWriter;

/// Shared writer factory retaining one synchronized file.
#[derive(Clone)]
pub(super) struct RotatingFileMakeWriter {
    /// Synchronized retained output.
    pub(super) state: Arc<Mutex<RotatingFileState>>,
}

impl RotatingFileMakeWriter {
    /// Open an append-only log, creating its parent directory.
    pub(super) fn new(
        dir: &Path,
        file_name: &str,
        max_bytes: u64,
        retained_files: usize,
    ) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join(file_name);
        let mut state = RotatingFileState {
            path,
            max_bytes,
            retained_files,
            file: None,
            current_size: 0,
            disabled: false,
        };
        state.open_current_file()?;
        Ok(Self {
            state: Arc::new(Mutex::new(state)),
        })
    }
}

impl<'a> MakeWriter<'a> for RotatingFileMakeWriter {
    type Writer = RotatingFileGuard;

    /// Borrow a writer backed by the same retained file.
    fn make_writer(&'a self) -> Self::Writer {
        RotatingFileGuard {
            state: Arc::clone(&self.state),
        }
    }
}

/// One event writer sharing the retained file.
pub(super) struct RotatingFileGuard {
    /// Synchronized retained output.
    pub(super) state: Arc<Mutex<RotatingFileState>>,
}

impl Write for RotatingFileGuard {
    /// Append bytes, disabling file output after any failure.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let Ok(mut state) = self.state.lock() else {
            return Ok(buf.len());
        };
        if state.disabled {
            return Ok(buf.len());
        }
        if state.rotate_if_needed(buf.len() as u64).is_err() {
            state.disabled = true;
            return Ok(buf.len());
        }
        if let Some(file) = state.file.as_mut() {
            if let Ok(written) = file.write(buf) {
                state.current_size = state.current_size.saturating_add(written as u64);
                Ok(written)
            } else {
                state.disabled = true;
                Ok(buf.len())
            }
        } else {
            Ok(buf.len())
        }
    }

    /// Flush the current file, swallowing failures after disabling output.
    fn flush(&mut self) -> io::Result<()> {
        let Ok(mut state) = self.state.lock() else {
            return Ok(());
        };
        if state.disabled {
            return Ok(());
        }
        if let Some(file) = state.file.as_mut()
            && file.flush().is_err()
        {
            state.disabled = true;
        }
        Ok(())
    }
}

/// File size, retention and permanent failure state.
pub(super) struct RotatingFileState {
    /// Native current log path.
    pub(super) path: PathBuf,
    /// Maximum bytes before rotation, zero for unlimited.
    pub(super) max_bytes: u64,
    /// Number of numbered generations to preserve.
    pub(super) retained_files: usize,
    /// Open current output, absent during rotation.
    pub(super) file: Option<File>,
    /// Observed bytes in the current output.
    pub(super) current_size: u64,
    /// Whether output has permanently failed.
    pub(super) disabled: bool,
}

impl RotatingFileState {
    /// Rotate before a write that exceeds the configured byte limit.
    fn rotate_if_needed(&mut self, incoming_len: u64) -> io::Result<()> {
        if self.file.is_none() {
            self.open_current_file()?;
        }
        if self.max_bytes == 0 || self.current_size.saturating_add(incoming_len) <= self.max_bytes {
            return Ok(());
        }
        self.rotate_files()?;
        self.open_current_file()
    }

    /// Open the current file and observe its existing size.
    fn open_current_file(&mut self) -> io::Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        self.current_size = file.metadata().map_or(0, |meta| meta.len());
        self.file = Some(file);
        Ok(())
    }

    /// Remove the oldest generation and move remaining generations downward.
    pub(super) fn rotate_files(&mut self) -> io::Result<()> {
        self.file.take();
        if self.retained_files == 0 {
            match fs::remove_file(&self.path) {
                Ok(()) => {}
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
            self.current_size = 0;
            return Ok(());
        }

        let oldest = rotated_log_path(&self.path, self.retained_files);
        match fs::remove_file(&oldest) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }

        for index in (1..=self.retained_files).rev() {
            let source = if index == 1 {
                self.path.clone()
            } else {
                rotated_log_path(&self.path, index - 1)
            };
            let target = rotated_log_path(&self.path, index);
            if !source.exists() {
                continue;
            }
            fs::rename(source, target)?;
        }

        self.current_size = 0;
        Ok(())
    }
}

/// Append a numeric generation suffix to the complete filename.
pub(super) fn rotated_log_path(path: &Path, index: usize) -> PathBuf {
    let suffix = format!(".{index}");
    let file_name = path.file_name().map_or_else(
        || suffix.clone().into(),
        |name| {
            let mut name = name.to_os_string();
            name.push(&suffix);
            name
        },
    );
    path.with_file_name(file_name)
}
