//! Native history selection and compact publication.
use super::{FileEntry, NewSessionOptions, SessionManager, conversions::invalid, files};
use serde::Serialize;
use serde_json::{Value, ser::Formatter};
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
        for (position, entry) in entries.iter().enumerate() {
            if entry["type"] == "session" {
                continue;
            }
            let id = entry["id"]
                .as_str()
                .ok_or_else(|| invalid("nonstring entry id"))?;
            by_id.insert(id.to_owned(), position);
            leaf_id = Some(id.to_owned());
        }
        let id = entries[0]["id"].as_str().unwrap_or_default().to_owned();
        self.file_entries = entries;
        self.by_id = by_id;
        self.leaf_id = leaf_id;
        self.session_id = id;
        self.session_file = Some(path);
        self.flushed = exists;
        Ok(())
    }

    /// Publish accepted records with the native append effect.
    pub(super) fn persist_entry(&mut self) -> io::Result<()> {
        self.publish(|path, bytes| {
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?
                .write_all(bytes)
        })
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
    if let Ok(path) = maestro_path::try_resolve(&[path], &[]) {
        return Ok(path);
    }
    let directories = if cfg!(windows) {
        maestro_path::env_drive_directories(|name| std::env::var_os(name))
    } else {
        Vec::new()
    };
    let drives: Vec<_> = directories
        .iter()
        .map(|(letter, path)| (*letter, path.as_str()))
        .collect();
    if let Ok(path) = maestro_path::try_resolve(&[path], &drives) {
        return Ok(path);
    }
    let cwd = std::env::current_dir()?.to_string_lossy().into_owned();
    Ok(maestro_path::resolve(
        &[path],
        &maestro_path::Cwd {
            current: &cwd,
            drive_directories: &drives,
        },
    ))
}

/// Rewrite retained records in order, leaving native partial writes in place.
fn rewrite(path: &str, entries: &[FileEntry]) -> io::Result<()> {
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
