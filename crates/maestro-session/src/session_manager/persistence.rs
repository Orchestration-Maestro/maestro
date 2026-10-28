//! Native history selection and compact publication.
use super::{
    FileEntry, NewSessionOptions, SessionManager, build_index, conversions::invalid, files,
    session_header,
};
use maestro_request::presence::Presence;
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
    /// The source is read with [`files::load_entries_from_file`] and converted by the
    /// historical conversions before the current header is stamped; its file is never
    /// rewritten. The new header keeps `source_path` exactly as authored as parent metadata.
    ///
    /// # Errors
    /// Fails with `Cannot fork: source session file is empty or invalid: {source_path}`
    /// before creating anything; a null source record, later directory, write and reopen
    /// errors are propagated and leave records already written in place.
    pub fn fork_from(
        source_path: &str,
        target_cwd: &str,
        session_dir: Option<&str>,
        agent_dir: &str,
    ) -> io::Result<Self> {
        Self::fork_with(source_path, target_cwd, session_dir, agent_dir, append)
    }

    /// Fork with a replaceable append effect, applied to the header and every record.
    pub(super) fn fork_with(
        source_path: &str,
        target_cwd: &str,
        session_dir: Option<&str>,
        agent_dir: &str,
        mut write: impl FnMut(&str, &[u8]) -> io::Result<()>,
    ) -> io::Result<Self> {
        let mut entries = files::load_entries_from_file(source_path)?;
        if entries.is_empty() {
            return Err(invalid(&format!(
                "Cannot fork: source session file is empty or invalid: {source_path}"
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
        let records = entries
            .iter()
            .filter(|entry| entry.is_null() || entry["type"] != "session");
        for entry in std::iter::once(&header).chain(records) {
            if entry.is_null() {
                return Err(invalid("source session record is null"));
            }
            write(&path, &encode(entry)?)?;
        }
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

    /// Open or recover an explicit lexical filename, reading it once.
    ///
    /// # Errors
    /// Propagates required cwd, file, conversion and selected-field errors.
    pub fn open(
        path: &str,
        session_dir: Option<&str>,
        cwd_override: Option<&str>,
    ) -> io::Result<Self> {
        let path = absolute(path)?;
        let entries = files::load_entries_from_file(&path)?;
        let cwd = match cwd_override {
            Some(cwd) => cwd.to_owned(),
            None => match entries.first().and_then(|header| header.get("cwd")) {
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
        manager.install(path, entries)?;
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
    /// Propagates native reads, conversion, rewrite and selected identity errors.
    pub fn set_session_file(&mut self, session_file: &str) -> io::Result<()> {
        let path = absolute(session_file)?;
        let entries = files::load_entries_from_file(&path)?;
        self.install(path, entries)
    }

    /// Prepare a selected history without publishing inconsistent retained state.
    fn install(&mut self, path: String, mut entries: Vec<FileEntry>) -> io::Result<()> {
        let exists = std::path::Path::new(&path).exists();
        let mut by_id = std::collections::HashMap::new();
        let mut leaf_id = None;
        if entries.is_empty() {
            entries = Self::in_memory(Some(&self.cwd))?.file_entries;
            if exists && self.persist {
                rewrite(&path, &entries)?;
            }
        } else if super::conversions::migrate(&mut entries, uuid::Uuid::new_v4)? && self.persist {
            rewrite(&path, &entries)?;
        }
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
        self.flushed = exists;
        Ok(())
    }

    /// Write the extracted history now only when it holds an assistant message.
    ///
    /// The publication state changes only after the scan, and the write when an
    /// assistant is found, succeed.
    pub(super) fn publish_branched(
        &mut self,
        write: impl FnOnce(&str, &[FileEntry]) -> io::Result<()>,
    ) -> io::Result<()> {
        let Some(path) = self.session_file.as_deref().filter(|_| self.persist) else {
            return Ok(());
        };
        let assistant = has_assistant(&self.file_entries)?;
        if assistant {
            write(path, &self.file_entries)?;
        }
        self.flushed = assistant;
        Ok(())
    }

    /// Publish accepted records with the native append effect.
    pub(super) fn persist_entry(&mut self) -> io::Result<()> {
        self.publish(append)
    }

    /// Keep memory acceptance before a private, unstored append effect.
    pub(super) fn publish(
        &mut self,
        mut append: impl FnMut(&str, &[u8]) -> io::Result<()>,
    ) -> io::Result<()> {
        let Some(path) = self.session_file.as_deref().filter(|_| self.persist) else {
            return Ok(());
        };
        if !self.flushed {
            if !has_assistant(&self.file_entries)? {
                return Ok(());
            }
            for entry in &self.file_entries {
                append(path, &encode(entry)?)?;
            }
            self.flushed = true;
        } else if let Some(entry) = self.file_entries.last() {
            append(path, &encode(entry)?)?;
        }
        Ok(())
    }
}

/// Append bytes to a native file, creating it when absent.
pub(super) fn append(path: &str, bytes: &[u8]) -> io::Result<()> {
    super::storage::with_write_lock(path, || {
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?
            .write_all(bytes)
    })
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

/// Rewrite retained records in order, leaving native partial writes in place.
pub(super) fn rewrite(path: &str, entries: &[FileEntry]) -> io::Result<()> {
    let mut bytes = Vec::new();
    for entry in entries {
        bytes.extend(encode(entry)?);
    }
    fs::write(path, bytes)
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
