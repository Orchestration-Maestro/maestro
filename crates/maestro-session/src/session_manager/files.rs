//! Native JSONL loading and continuation selection.
use super::{FileEntry, parse_session_entries};
use serde_json::Value;
use std::{
    fs,
    io::{self, Read},
    time::UNIX_EPOCH,
};

/// Encode an authored working directory into its session directory name.
///
/// Strips one leading separator and replaces separators and drive colons with
/// dashes. Performs no I/O and does not normalize the authored path.
pub(crate) fn encode_session_dir(cwd: &str) -> String {
    let cwd = cwd.strip_prefix(['/', '\\']).unwrap_or(cwd);
    format!("--{}--", cwd.replace(['/', '\\', ':'], "-"))
}

/// The directory beneath a base that holds one working directory's sessions.
///
/// Only this module builds one, so a caller receives a session directory
/// instead of spelling one:
///
/// ```compile_fail,E0603
/// let _ = maestro_session::SessionDir(String::from("--workspace--"));
/// ```
pub struct SessionDir(String);

impl SessionDir {
    /// The directory path this record names.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The directory beneath `base` that holds the sessions of `cwd`.
#[must_use]
pub fn sessions_dir(base: &str, cwd: &str) -> SessionDir {
    SessionDir(maestro_path::join(&[base, &encode_session_dir(cwd)]))
}

/// Encode the authored cwd into a default directory beneath the resolved agent root.
///
/// # Errors
/// Propagates native directory creation errors.
pub fn get_default_session_dir(cwd: &str, agent_dir: &str) -> io::Result<String> {
    let directory = maestro_path::join(&[agent_dir, "sessions", &encode_session_dir(cwd)]);
    if !std::path::Path::new(&directory).exists() {
        fs::create_dir_all(&directory)?;
    }
    Ok(directory)
}

/// Load raw records after admitting the first parsed session header.
///
/// # Errors
/// Propagates native errors reading an existing file; a missing file is empty.
pub fn load_entries_from_file(file_path: &str) -> io::Result<Vec<FileEntry>> {
    if !std::path::Path::new(file_path).exists() {
        return Ok(Vec::new());
    }
    Ok(entries_from_bytes(&fs::read(file_path)?))
}

/// Parses raw bytes with the same first-header admission as native loading.
pub(super) fn entries_from_bytes(bytes: &[u8]) -> Vec<FileEntry> {
    let entries = parse_session_entries(&String::from_utf8_lossy(bytes));
    if entries.first().is_some_and(valid_header) {
        entries
    } else {
        Vec::new()
    }
}

/// Whether the raw first record admits a session identity.
fn valid_header(header: &Value) -> bool {
    header["type"] == "session" && header["id"].is_string()
}

/// Probe once through an owned or borrowed reader without whole-file parsing.
pub(super) fn probe(mut reader: impl Read) -> io::Result<bool> {
    let mut buffer = [0; 512];
    let count = reader.read(&mut buffer)?;
    let text = String::from_utf8_lossy(&buffer[..count]);
    Ok(text
        .split('\n')
        .next()
        .and_then(|line| serde_json::from_str(line).ok())
        .is_some_and(|header| valid_header(&header)))
}

/// Path of a directory entry whose name ends in exactly `.jsonl`.
pub(super) fn jsonl_path(dir: &str, entry: &fs::DirEntry) -> Option<String> {
    let name = entry.file_name();
    let name = name.to_string_lossy();
    name.ends_with(".jsonl")
        .then(|| maestro_path::join(&[dir, &name]))
}

/// Select the valid JSONL file with greatest millisecond mtime; equal times go to the
/// first file in name order. Directory/stat errors discard selection; individual
/// probe failures skip a file.
#[must_use]
pub fn find_most_recent_session(session_dir: &str) -> Option<String> {
    let paths: Vec<_> = maestro_listing::read_dir(session_dir)
        .ok()?
        .into_iter()
        .filter_map(|entry| jsonl_path(session_dir, &entry))
        .filter(|path| fs::File::open(path).and_then(probe).unwrap_or(false))
        .collect();
    let mut files = paths
        .into_iter()
        .map(|path| {
            let millis = modified_millis(&fs::metadata(&path).ok()?)?;
            Some((path, millis))
        })
        .collect::<Option<Vec<_>>>()?;
    files.sort_by_key(|file| std::cmp::Reverse(file.1));
    files.into_iter().next().map(|(path, _)| path)
}

/// Whole epoch milliseconds of a modification time; earlier times are negative.
pub(super) fn modified_millis(metadata: &fs::Metadata) -> Option<i128> {
    Some(match metadata.modified().ok()?.duration_since(UNIX_EPOCH) {
        Ok(duration) => i128::try_from(duration.as_millis()).ok()?,
        Err(error) => -i128::try_from(error.duration().as_millis()).ok()?,
    })
}
