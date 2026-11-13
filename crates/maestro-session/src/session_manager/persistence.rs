//! Native history selection and compact publication.
use super::{
    FileEntry, NewSessionOptions, SessionManager, build_index, conversions::invalid, files,
    files::Snapshot, session_header,
};
use maestro_interchange::presence::Presence;
use serde::Serialize;
use serde_json::{Map, Value, ser::Formatter};
use std::{
    fs,
    io::{self, Write},
};

impl SessionManager {
    /// Create an unpublished native history in the selected or default directory.
    ///
    /// # Errors
    /// Propagates native directory creation errors.
    pub fn create(cwd: &str, session_dir: Option<&str>, agent_dir: &str) -> io::Result<Self> {
        let mut manager = Self::native(cwd, session_dir, agent_dir)?;
        manager.new_session(NewSessionOptions::default());
        Ok(manager)
    }

    /// Copy a source file's nonheader records into a new published native history.
    ///
    /// The source is recovered and read under exclusion, then converted before the
    /// current header is stamped. The new header keeps `source_path` exactly as
    /// authored as parent metadata.
    ///
    /// # Errors
    /// Fails with `Cannot fork: the source session file has no valid entries: {source_path}`
    /// before creating anything; later directory, write and reopen errors are propagated
    /// and leave bytes already written in place. Empty JSON values are skipped.
    pub fn fork_from(
        source_path: &str,
        target_cwd: &str,
        session_dir: Option<&str>,
        agent_dir: &str,
    ) -> io::Result<Self> {
        Self::fork_with(source_path, target_cwd, session_dir, agent_dir, append)
    }

    /// Fork with a replaceable append effect, applied once to the header and every record.
    pub(super) fn fork_with(
        source_path: &str,
        target_cwd: &str,
        session_dir: Option<&str>,
        agent_dir: &str,
        mut write: impl FnMut(&str, &[u8]) -> io::Result<()>,
    ) -> io::Result<Self> {
        let mut entries = super::storage::read(source_path)?.entries;
        if entries.is_empty() {
            return Err(invalid(&format!(
                "Cannot fork: the source session file has no valid entries: {source_path}"
            )));
        }
        super::conversions::migrate(&mut entries, uuid::Uuid::new_v4)?;
        let dir = match session_dir {
            Some(dir) => dir.to_owned(),
            None => files::get_default_session_dir(target_cwd, agent_dir)?,
        };
        if !std::path::Path::new(&dir).exists() {
            if dir.is_empty() {
                return Err(io::ErrorKind::NotFound.into());
            }
            fs::create_dir_all(&dir)?;
        }
        let id = uuid::Uuid::now_v7().to_string();
        let (header, file) =
            session_header(&id, target_cwd, Presence::Present(source_path.to_owned()));
        let path = maestro_path::join(&[&dir, &file]);
        let records = entries.iter().filter(|entry| entry["type"] != "session");
        write(
            &path,
            &encode_entries(std::iter::once(&header).chain(records))?,
        )?;
        Self::open(&path, Some(&dir), Some(target_cwd))
    }

    /// Resolve the selected directory without allocating a discarded header.
    fn native(cwd: &str, session_dir: Option<&str>, agent_dir: &str) -> io::Result<Self> {
        let directory = match session_dir {
            Some(directory) => directory.to_owned(),
            None => files::get_default_session_dir(cwd, agent_dir)?,
        };
        Self::base(cwd.to_owned(), directory, true)
    }

    /// Open or recover an explicit lexical filename, retrying conversion if its bytes change.
    ///
    /// # Errors
    /// Propagates required cwd, file, conversion and selected-field errors. A nonempty file
    /// without a valid session header fails with `Session file is not a valid maestro session: {path}`
    /// and is left unchanged; an empty file is initialised.
    pub fn open(
        path: &str,
        session_dir: Option<&str>,
        cwd_override: Option<&str>,
    ) -> io::Result<Self> {
        let path = absolute(path)?;
        let snapshot = super::storage::read(&path)?;
        let cwd = match cwd_override {
            Some(cwd) => cwd.to_owned(),
            None => match snapshot
                .entries
                .first()
                .and_then(|header| header.get("cwd"))
            {
                None | Some(Value::Null) => std::env::current_dir()?.to_string_lossy().into_owned(),
                Some(value) => value
                    .as_str()
                    .ok_or_else(|| invalid("nonstring cwd"))?
                    .to_owned(),
            },
        };
        let directory =
            session_dir.map_or_else(|| maestro_path::join(&[&path, ".."]), str::to_owned);
        let mut manager = Self::base(cwd, directory, true)?;
        manager.install(path, snapshot)?;
        Ok(manager)
    }

    /// Continue the mtime-selected file, or create an unpublished history.
    ///
    /// # Errors
    /// Propagates native creation, selection and conversion errors.
    pub fn continue_recent(
        cwd: &str,
        session_dir: Option<&str>,
        agent_dir: &str,
    ) -> io::Result<Self> {
        let mut manager = Self::native(cwd, session_dir, agent_dir)?;
        if let Some(path) = files::find_most_recent_session(&manager.session_dir) {
            manager.set_session_file(&path)?;
        } else {
            manager.new_session(NewSessionOptions::default());
        }
        Ok(manager)
    }

    /// Switch histories, committing candidate state only after fallible preparation.
    ///
    /// # Errors
    /// Propagates native reads, conversion, rewrite and selected identity errors. A nonempty
    /// file without a valid session header fails as [`SessionManager::open`] does.
    pub fn set_session_file(&mut self, session_file: &str) -> io::Result<()> {
        let path = absolute(session_file)?;
        let snapshot = if self.persist {
            super::storage::read(&path)?
        } else {
            super::storage::snapshot_native(std::path::Path::new(&path))?
        };
        self.install(path, snapshot)
    }

    /// Prepare a selected history without publishing inconsistent retained state.
    fn install(&mut self, path: String, snapshot: Snapshot) -> io::Result<()> {
        let exists = std::path::Path::new(&path).exists();
        let entries = self.prepare_entries(&path, snapshot)?;
        let mut by_id = std::collections::HashMap::new();
        let mut leaf_id = None;
        let (mut labels, mut label_timestamps) = (Map::new(), Map::new());
        build_index(
            &entries,
            &mut by_id,
            &mut leaf_id,
            &mut labels,
            &mut label_timestamps,
        )?;
        let id = entries[0]["id"].as_str().unwrap_or_default().to_owned();
        self.file_entries = entries;
        self.by_id = by_id;
        self.leaf_id = leaf_id;
        self.labels = labels;
        self.label_timestamps = label_timestamps;
        self.session_id = id;
        self.session_file = Some(path);
        self.saved = if exists { self.file_entries.len() } else { 0 };
        Ok(())
    }

    /// Repeats conversion when a locked re-read observes a concurrent writer.
    fn prepare_entries(&self, path: &str, mut snapshot: Snapshot) -> io::Result<Vec<FileEntry>> {
        loop {
            snapshot.require_session(path)?;
            let mut entries = std::mem::take(&mut snapshot.entries);
            let changed = if entries.is_empty() {
                entries = Self::in_memory(Some(&self.cwd))?.file_entries;
                std::path::Path::new(path).exists()
            } else {
                super::conversions::migrate(&mut entries, uuid::Uuid::new_v4)?
            };
            if !changed || !self.persist {
                return Ok(entries);
            }
            match super::storage::rewrite_snapshot(
                path,
                &snapshot.digest,
                &encode_entries(&entries)?,
            )? {
                Some(current) => snapshot = current,
                None => return Ok(entries),
            }
        }
    }

    /// Write the extracted history now only when it holds an assistant message.
    ///
    /// The history counts as unsaved until the write succeeds. A failed write is cut
    /// back to an empty file, so the next save writes the whole history once.
    pub(super) fn publish_branched(
        &mut self,
        write: impl FnOnce(&str, &[FileEntry]) -> io::Result<()>,
    ) -> io::Result<()> {
        let Some(path) = self.session_file.as_deref().filter(|_| self.persist) else {
            return Ok(());
        };
        self.saved = 0;
        if has_assistant(&self.file_entries)? {
            write(path, &self.file_entries).map_err(|error| restore_length(path, 0, error))?;
            self.saved = self.file_entries.len();
        }
        Ok(())
    }

    /// Publish accepted records with the native append effect.
    pub(super) fn persist_entry(&mut self) -> io::Result<()> {
        self.publish(append)
    }

    /// Keep memory acceptance before a private, unstored append effect.
    ///
    /// One append holds every record the file does not hold yet, so a save that follows
    /// a failed one writes the records that one missed. Nothing is written before the
    /// history holds an assistant message. A failed append is cut back to the earlier
    /// length, so no torn line is left for the next record to join.
    pub(super) fn publish(
        &mut self,
        mut append: impl FnMut(&str, &[u8]) -> io::Result<()>,
    ) -> io::Result<()> {
        let Some(path) = self.session_file.as_deref().filter(|_| self.persist) else {
            return Ok(());
        };
        if self.saved == 0 && !has_assistant(&self.file_entries)? {
            return Ok(());
        }
        let unsaved = self.file_entries.get(self.saved..).unwrap_or_default();
        let bytes = encode_entries(unsaved)?;
        let before = fs::metadata(path).map_or(0, |metadata| metadata.len());
        append(path, &bytes).map_err(|error| restore_length(path, before, error))?;
        self.saved = self.file_entries.len();
        Ok(())
    }
}

/// Append bytes to a native file, creating it when absent.
pub(super) fn append(path: &str, bytes: &[u8]) -> io::Result<()> {
    super::storage::with_write_lock(path, |holder| match holder {
        Some(holder) => holder.append(bytes),
        None => fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?
            .write_all(bytes),
    })
}

/// Cut a failed save back to its earlier length so the retry writes it once.
///
/// Returns the save error, extended with the restore error when the cut also fails.
fn restore_length(path: &str, length: u64, error: io::Error) -> io::Error {
    let cut = super::storage::with_write_lock(path, |_holder| {
        fs::OpenOptions::new()
            .write(true)
            .open(path)?
            .set_len(length)
    });
    match cut {
        Err(restore) if restore.kind() != io::ErrorKind::NotFound => io::Error::new(
            error.kind(),
            format!("{error}; restoring the session file failed: {restore}"),
        ),
        _ => error,
    }
}

/// Read message roles only until the first assistant is encountered.
fn has_assistant(entries: &[FileEntry]) -> io::Result<bool> {
    for entry in entries.iter().filter(|entry| entry["type"] == "message") {
        let message = entry
            .get("message")
            .filter(|message| !message.is_null())
            .ok_or_else(|| invalid("missing message receiver"))?;
        if message["role"] == "assistant" {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Resolve a path lazily, preserving the original native cwd error.
fn absolute(path: &str) -> io::Result<String> {
    maestro_resources::paths::resolve(&[path])
}

/// Publishes branch records, overwriting any existing bytes under exclusive ownership.
pub(super) fn rewrite(path: &str, entries: &[FileEntry]) -> io::Result<()> {
    #[cfg(test)]
    if let Some(before) = super::tests::BEFORE_BRANCH.take() {
        before(path);
    }
    maestro_lock::acquire_write(std::path::Path::new(path), true)?
        .replace(&encode_entries(entries)?, None)?;
    Ok(())
}

/// Encodes retained records in their publication order.
fn encode_entries<'a>(entries: impl IntoIterator<Item = &'a FileEntry>) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for entry in entries {
        bytes.extend(encode(entry)?);
    }
    Ok(bytes)
}

/// Compact float spelling for the JSONL wire producer.
struct ScriptNumbers;
impl Formatter for ScriptNumbers {
    fn write_f64<W: ?Sized + Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        writer.write_all(ryu_js::Buffer::new().format_finite(value).as_bytes())
    }
}

/// Encode a retained record followed by its LF delimiter.
fn encode(entry: &FileEntry) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    entry.serialize(&mut serde_json::Serializer::with_formatter(
        &mut bytes,
        ScriptNumbers,
    ))?;
    bytes.push(b'\n');
    Ok(bytes)
}
