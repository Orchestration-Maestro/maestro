//! Native JSONL loading and continuation selection.
use super::{FileEntry, parse_session_entries};
use serde_json::Value;
use std::{
    fs,
    io::{self, Read},
    time::UNIX_EPOCH,
};

/// Encode the authored cwd into a default directory beneath the resolved agent root.
///
/// # Errors
/// Propagates native directory creation errors.
pub fn get_default_session_dir(cwd: &str, agent_dir: &str) -> io::Result<String> {
    let cwd = cwd.strip_prefix(['/', '\\']).unwrap_or(cwd);
    let safe = format!("--{}--", cwd.replace(['/', '\\', ':'], "-"));
    let directory = maestro_path::join(&[agent_dir, "sessions", &safe]);
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
    let bytes = fs::read(file_path)?;
    let entries = parse_session_entries(&String::from_utf8_lossy(&bytes));
    Ok(if entries.first().is_some_and(valid_header) {
        entries
    } else {
        Vec::new()
    })
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

/// Select the valid JSONL file with greatest millisecond mtime, retaining native ties.
/// Directory/stat errors discard selection; individual probe failures skip a file.
#[must_use]
pub fn find_most_recent_session(session_dir: &str) -> Option<String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(session_dir).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.strip_suffix(".jsonl").is_none() {
            continue;
        }
        let path = maestro_path::join(&[session_dir, &name]);
        if !fs::File::open(&path).and_then(probe).unwrap_or(false) {
            continue;
        }
        let modified = fs::metadata(&path).ok()?.modified().ok()?;
        let millis = match modified.duration_since(UNIX_EPOCH) {
            Ok(duration) => i128::try_from(duration.as_millis()).ok()?,
            Err(error) => -i128::try_from(error.duration().as_millis()).ok()?,
        };
        files.push((path, millis));
    }
    files.sort_by_key(|file| std::cmp::Reverse(file.1));
    files.into_iter().next().map(|(path, _)| path)
}
