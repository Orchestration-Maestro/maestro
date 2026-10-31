//! Native session listing with per-candidate progress.
#![doc = include_str!("../../../../docs/discovery.md")]
use super::{
    SessionInfo, SessionManager,
    context::{ecma_whitespace, observation, property},
    files::{get_default_session_dir, jsonl_path, modified_millis},
    records::{Admission, for_each_entry},
};
use crate::messages::{millis_number, timestamp_millis};
use futures_util::{StreamExt, stream};
use serde_json::Value;
use std::{
    borrow::Cow,
    fs::{self, DirEntry},
    future::Future,
    io::{self, BufRead, BufReader},
    ops::ControlFlow,
};

/// The most session files whose metadata loads at the same time.
const MAX_CONCURRENT_LOADS: usize = 10;

/// Progress callback receiving completed and total candidate counts; an error stops the listing.
pub type SessionListProgress<'a> = dyn FnMut(usize, usize) -> io::Result<()> + 'a;

impl SessionManager {
    /// List the sessions of one project directory, most recently active first;
    /// equal times keep the name order of their files. An explicit `session_dir`
    /// is used verbatim; otherwise the default directory for `cwd` is created
    /// beneath `agent_dir`. A failing progress callback stops the listing with an
    /// empty result.
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
            Some(dir) => Cow::Borrowed(dir),
            None => Cow::Owned(get_default_session_dir(cwd, agent_dir)?),
        };
        let files = read_paths(&dir, jsonl_path).await.unwrap_or_default();
        Ok(collect(files, build_session_info, on_progress).await)
    }

    /// List the sessions of every project directory under `<agent_dir>/sessions`,
    /// most recently active first; equal times keep the name order of the project
    /// directories, then of their files. Candidates are counted before any is read;
    /// an unreadable root yields an empty list.
    pub async fn list_all(
        agent_dir: &str,
        on_progress: Option<&mut SessionListProgress<'_>>,
    ) -> Vec<SessionInfo> {
        let root = maestro_path::join(&[agent_dir, "sessions"]);
        let directories = read_paths(&root, |root, entry| {
            let name = entry.file_name();
            let directory = entry.file_type().is_ok_and(|kind| kind.is_dir());
            directory.then(|| maestro_path::join(&[root, &name.to_string_lossy()]))
        });
        let Ok(directories) = directories.await else {
            return Vec::new();
        };
        let mut files = Vec::new();
        for dir in &directories {
            if let Ok(children) = read_paths(dir, jsonl_path).await {
                files.extend(children);
            }
        }
        collect(files, build_session_info, on_progress).await
    }
}

/// List a directory in name order on the blocking pool, keeping the paths `keep`
/// derives; any listing error fails the whole listing.
async fn read_paths(
    dir: &str,
    keep: fn(&str, &DirEntry) -> Option<String>,
) -> io::Result<Vec<String>> {
    let dir = dir.to_owned();
    tokio::task::spawn_blocking(move || {
        let entries = maestro_listing::read_dir(&dir)?;
        Ok(entries
            .iter()
            .filter_map(|entry| keep(&dir, entry))
            .collect())
    })
    .await
    .map_err(io::Error::other)?
}

/// Read the candidates in this task, at most `MAX_CONCURRENT_LOADS` at a time, reporting
/// progress as each completes, then order the healthy results by input position and
/// descending activity.
pub(super) async fn collect<F: Future<Output = Option<SessionInfo>>>(
    files: Vec<String>,
    load: impl Fn(String) -> F,
    mut on_progress: Option<&mut SessionListProgress<'_>>,
) -> Vec<SessionInfo> {
    let total = files.len();
    let mut pending = stream::iter(files.into_iter().enumerate())
        .map(|(index, file)| {
            let loading = load(file);
            async move { (index, loading.await) }
        })
        .buffer_unordered(MAX_CONCURRENT_LOADS);
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

/// Summarize one session file on the blocking pool, or `None` when it is unreadable or not a session.
async fn build_session_info(path: String) -> Option<SessionInfo> {
    tokio::task::spawn_blocking(move || read_session_info(&path))
        .await
        .ok()?
}

/// Summarize one session file, reading it line by line.
fn read_session_info(path: &str) -> Option<SessionInfo> {
    let (header, scan) = scan_reader(BufReader::new(fs::File::open(path).ok()?))?;
    let metadata = fs::metadata(path).ok()?;
    let mtime = millis_number(i64::try_from(modified_millis(&metadata)?).ok()?);
    Some(summarize(path, &header, scan, mtime))
}

/// The session header and the facts of every later record, or `None` when the first record is
/// not a session header or a reached record is malformed; reading stops at that record.
/// Null, false, zero and empty-string records are skipped before either check.
pub(super) fn scan_reader(reader: impl BufRead) -> Option<(Value, Scan)> {
    let mut header: Option<Value> = None;
    let mut scan = Scan::default();
    let mut admitted = true;
    for_each_entry(reader, Admission::Nonempty, |entry| {
        admitted = match &header {
            None => is_session_header(&entry),
            Some(_) => scan_entry(&entry, &mut scan).is_ok(),
        };
        if !admitted {
            return ControlFlow::Break(());
        }
        header.get_or_insert(entry);
        ControlFlow::Continue(())
    })
    .ok()?;
    admitted.then_some(())?;
    Some((header?, scan))
}

/// Whether the record is a session header.
fn is_session_header(entry: &Value) -> bool {
    matches!(property(Some(entry), "type"), Ok(Some(kind)) if kind.as_str() == Some("session"))
}

/// Selected facts gathered from the entries after the header check.
#[derive(Default)]
pub(super) struct Scan {
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

/// Fold one admitted entry into the scan; a malformed selected field fails the file.
fn scan_entry(entry: &Value, scan: &mut Scan) -> Result<(), serde_json::Error> {
    match property(Some(entry), "type")?.and_then(Value::as_str) {
        Some("session_info") => scan.name = session_name(entry)?,
        Some("message") => {
            scan.message_count += 1;
            scan_message(entry, scan)?;
        }
        _ => {}
    }
    Ok(())
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
