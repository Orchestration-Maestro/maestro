//! Native session listing with per-candidate progress.
#![doc = include_str!("../../../../docs/discovery.md")]
use super::{
    SessionInfo, SessionManager,
    context::{ecma_whitespace, observation, property},
    files::{get_default_session_dir, modified_millis},
    parse_session_entries,
};
use crate::messages::{millis_number, timestamp_millis};
use futures_util::{StreamExt, stream::FuturesUnordered};
use serde_json::Value;
use std::io;
use tokio::fs::DirEntry;

/// Progress callback receiving completed and total candidate counts; an error stops the listing.
pub type SessionListProgress<'a> = dyn FnMut(usize, usize) -> io::Result<()> + 'a;

impl SessionManager {
    /// List the sessions of one project directory, most recently active first.
    /// An explicit `session_dir` is used verbatim; otherwise the default directory
    /// for `cwd` is created beneath `agent_dir`. A failing progress callback stops
    /// the listing with an empty result.
    ///
    /// # Errors
    /// Propagates the error creating an omitted default directory.
    pub async fn list(
        cwd: &str,
        session_dir: Option<&str>,
        agent_dir: &str,
        on_progress: Option<&mut SessionListProgress<'_>>,
    ) -> io::Result<Vec<SessionInfo>> {
        let dir = match session_dir {
            Some(dir) => dir.to_owned(),
            None => get_default_session_dir(cwd, agent_dir)?,
        };
        let files = match read_entries(&dir).await {
            Ok(entries) => jsonl_paths(&dir, &entries),
            Err(_) => Vec::new(),
        };
        Ok(collect(files, on_progress).await)
    }

    /// List the sessions of every project directory under `<agent_dir>/sessions`.
    /// Candidates are counted before any is read; an unreadable root yields an empty list.
    pub async fn list_all(
        agent_dir: &str,
        on_progress: Option<&mut SessionListProgress<'_>>,
    ) -> Vec<SessionInfo> {
        let root = maestro_path::join(&[agent_dir, "sessions"]);
        let Ok(entries) = read_entries(&root).await else {
            return Vec::new();
        };
        let mut files = Vec::new();
        for entry in &entries {
            if !entry.file_type().await.is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let dir = maestro_path::join(&[&root, &entry.file_name().to_string_lossy()]);
            if let Ok(children) = read_entries(&dir).await {
                files.extend(jsonl_paths(&dir, &children));
            }
        }
        collect(files, on_progress).await
    }
}

/// Enumerate a directory in native order, failing on any listing error.
async fn read_entries(dir: &str) -> io::Result<Vec<DirEntry>> {
    let mut reader = tokio::fs::read_dir(dir).await?;
    let mut entries = Vec::new();
    while let Some(entry) = reader.next_entry().await? {
        entries.push(entry);
    }
    Ok(entries)
}

/// Paths of the entries whose names end in exactly `.jsonl`.
fn jsonl_paths(dir: &str, entries: &[DirEntry]) -> Vec<String> {
    entries
        .iter()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.strip_suffix(".jsonl").is_some())
        .map(|name| maestro_path::join(&[dir, &name]))
        .collect()
}

/// Read every candidate concurrently in this task, reporting progress as each completes,
/// then order the healthy results by input position and descending activity.
async fn collect(
    files: Vec<String>,
    mut on_progress: Option<&mut SessionListProgress<'_>>,
) -> Vec<SessionInfo> {
    let total = files.len();
    let mut pending: FuturesUnordered<_> = files
        .iter()
        .enumerate()
        .map(|(index, file)| async move { (index, build_session_info(file).await) })
        .collect();
    let mut results: Vec<Option<SessionInfo>> = (0..total).map(|_| None).collect();
    let mut loaded = 0;
    while let Some((index, info)) = pending.next().await {
        results[index] = info;
        loaded += 1;
        if let Some(progress) = on_progress.as_deref_mut()
            && progress(loaded, total).is_err()
        {
            return Vec::new();
        }
    }
    let mut sessions: Vec<_> = results.into_iter().flatten().collect();
    sessions.sort_by(|a, b| b.modified.total_cmp(&a.modified));
    sessions
}

/// Summarize one session file, or `None` when it is unreadable or not a session.
async fn build_session_info(path: &str) -> Option<SessionInfo> {
    let bytes = tokio::fs::read(path).await.ok()?;
    let entries = parse_session_entries(&String::from_utf8_lossy(&bytes));
    let header = entries.first()?;
    if property(Some(header), "type").ok()??.as_str() != Some("session") {
        return None;
    }
    let metadata = tokio::fs::metadata(path).await.ok()?;
    let scan = scan_entries(&entries).ok()?;
    let mtime = millis_number(i64::try_from(modified_millis(&metadata)?).ok()?);
    Some(summarize(path, header, scan, mtime))
}

/// Selected facts gathered from the entries after the header check.
#[derive(Default)]
struct Scan {
    /// Message records seen.
    message_count: usize,
    /// Latest trimmed name.
    name: Option<String>,
    /// First nonempty user text.
    first_message: String,
    /// Indexed texts in order.
    texts: Vec<String>,
    /// Greatest activity time; zero when none.
    activity: f64,
}

/// Walk every entry once; a reached null record or malformed selected field fails the file.
fn scan_entries(entries: &[Value]) -> Result<Scan, serde_json::Error> {
    let mut scan = Scan::default();
    for entry in entries {
        match property(Some(entry), "type")?.and_then(Value::as_str) {
            Some("session_info") => scan.name = session_name(entry)?,
            Some("message") => {
                scan.message_count += 1;
                scan_message(entry, &mut scan)?;
            }
            _ => {}
        }
    }
    Ok(scan)
}

/// Trim a string name; absent, null or empty clears it and other types fail.
fn session_name(entry: &Value) -> Result<Option<String>, serde_json::Error> {
    match entry.get("name") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(name)) => Ok(Some(name.trim_matches(ecma_whitespace))
            .filter(|name| !name.is_empty())
            .map(str::to_owned)),
        Some(_) => Err(serde::de::Error::custom("name is not a string")),
    }
}

/// Index a user or assistant message that has content and track its activity time.
fn scan_message(entry: &Value, scan: &mut Scan) -> Result<(), serde_json::Error> {
    let message = entry.get("message");
    let Some(role) = property(message, "role")?.and_then(Value::as_str) else {
        return Ok(());
    };
    let Some(content) = message.and_then(|message| message.get("content")) else {
        return Ok(());
    };
    if role != "user" && role != "assistant" {
        return Ok(());
    }
    let activity = match message.and_then(|message| message.get("timestamp")) {
        Some(Value::Number(time)) => time.as_f64(),
        _ => entry
            .get("timestamp")
            .and_then(Value::as_str)
            .map(timestamp_millis)
            .filter(|time| !time.is_nan()),
    };
    scan.activity = scan.activity.max(activity.unwrap_or(0.0));
    let text = message_text(content)?;
    if !text.is_empty() {
        if scan.first_message.is_empty() && role == "user" {
            scan.first_message.clone_from(&text);
        }
        scan.texts.push(text);
    }
    Ok(())
}

/// A string as is, or the space-joined text of the text blocks of an array.
fn message_text(content: &Value) -> Result<String, serde_json::Error> {
    match content {
        Value::String(text) => Ok(text.clone()),
        Value::Array(blocks) => {
            let mut parts = Vec::new();
            for block in blocks {
                if property(Some(block), "type")?.and_then(Value::as_str) == Some("text") {
                    parts.push(
                        block
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    );
                }
            }
            Ok(parts.join(" "))
        }
        _ => Err(serde::de::Error::custom("content is not text")),
    }
}

/// Header creation time: strings parse, numbers truncate, null is the epoch, the rest is invalid.
fn created_millis(timestamp: Option<&Value>) -> f64 {
    match timestamp {
        Some(Value::String(text)) => timestamp_millis(text),
        Some(Value::Number(number)) => number.as_f64().map_or(f64::NAN, f64::trunc),
        Some(Value::Null) => 0.0,
        _ => f64::NAN,
    }
}

/// Assemble the listing record from the header and the scanned facts.
fn summarize(path: &str, header: &Value, scan: Scan, mtime: f64) -> SessionInfo {
    let header_time = header
        .get("timestamp")
        .and_then(Value::as_str)
        .map(timestamp_millis)
        .filter(|time| !time.is_nan());
    let modified = if scan.activity > 0.0 {
        scan.activity.trunc()
    } else {
        header_time.unwrap_or(mtime)
    };
    SessionInfo {
        path: path.to_owned(),
        id: observation(header, "id"),
        cwd: header
            .get("cwd")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        name: scan.name,
        parent_session_path: observation(header, "parentSession"),
        created: created_millis(header.get("timestamp")),
        modified,
        message_count: scan.message_count,
        first_message: if scan.first_message.is_empty() {
            "(no messages)".to_owned()
        } else {
            scan.first_message
        },
        all_messages_text: scan.texts.join(" "),
    }
}
